mod app;
mod config;
mod generator;
mod initializr;
mod model;
mod ui;

use anyhow::Result;
use app::{App, Mode, ResourceItem};
use clap::{Parser, Subcommand};
use crossterm::{
    event::{Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use model::AppStatus;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{
    io,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;

pub const VERSION: &str = match option_env!("TSB_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

/// Terminal UI for Spring Boot — manage and monitor Spring Boot apps
#[derive(Parser)]
#[command(name = "tsb", version = VERSION, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a new Spring Boot project via Spring Initializr
    New,
}

// ---------------------------------------------------------------------------
// AppEvent — async events sent from spawned tasks back to the main loop
// ---------------------------------------------------------------------------

enum AppEvent {
    /// Splash progress message
    Progress(String),
    /// Health check result for a specific app
    HealthResult {
        app_index: usize,
        status: AppStatus,
        app_name: Option<String>,
    },
    /// All initial health checks done — transition out of splash
    SplashDone,
    /// Actuator data fetched for a resource
    #[allow(dead_code)]
    DataFetched {
        resource: String,
        result: Result<(), String>,
    },
    /// Port scan progress update
    ScanProgress(String),
    /// A Spring Boot app was discovered during port scan
    ScanFound {
        url: String,
        port: u16,
        status: AppStatus,
    },
    /// Port scan is complete
    ScanDone,
    /// Initializr metadata loaded successfully
    MetadataLoaded(Box<crate::model::InitializrMetadata>),
    /// Initializr metadata loading failed
    MetadataFailed(String),
    /// Project generation completed
    GenerateResult(Result<String, String>),
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::New) => {
            return initializr::run().await;
        }
        None => {
            return run_tui().await;
        }
    }
}

// ---------------------------------------------------------------------------
// Platform-aware external tool launchers
// ---------------------------------------------------------------------------

/// Launch VisualVM with `--openfile <path>` to open a dump file.
fn launch_visualvm_file(path: &str) -> Result<(), String> {
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c =
            std::process::Command::new("/Applications/VisualVM.app/Contents/MacOS/visualvm");
        c.arg("--openfile").arg(path);
        c
    } else {
        let mut c = std::process::Command::new("visualvm");
        c.arg("--openfile").arg(path);
        c
    };
    cmd.stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start VisualVM: {}", e))?;
    Ok(())
}

/// Launch VisualVM with `--openpid <pid>` to attach to a live process.
fn launch_visualvm_pid(pid: &str) -> Result<(), String> {
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c =
            std::process::Command::new("/Applications/VisualVM.app/Contents/MacOS/visualvm");
        c.arg("--openpid").arg(pid);
        c
    } else {
        let mut c = std::process::Command::new("visualvm");
        c.arg("--openpid").arg(pid);
        c
    };
    cmd.stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start VisualVM: {}", e))?;
    Ok(())
}

/// Launch Eclipse Memory Analyzer with a heap dump file.
fn launch_eclipse_mat(path: &str) -> Result<(), String> {
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg("-a").arg("MemoryAnalyzer").arg(path);
        c
    } else if cfg!(target_os = "windows") {
        let mut c = std::process::Command::new("MemoryAnalyzer.exe");
        c.arg(path);
        c
    } else {
        let mut c = std::process::Command::new("MemoryAnalyzer");
        c.arg(path);
        c
    };
    cmd.stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start Eclipse MAT: {}", e))?;
    Ok(())
}

/// Run the full-screen TUI (default `tsb` command).
async fn run_tui() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new()?;
    app.scan_saved_dumps();

    // Create channel for async events
    let (tx, mut rx) = mpsc::channel::<AppEvent>(100);

    // Kick off the splash sequence
    {
        // Always show splash first
        app.mode = Mode::Splash;
        app.splash_state.current_step = 0;
        app.splash_state.current_message = if app.apps.is_empty() {
            "Starting up...".into()
        } else {
            "Checking application health...".into()
        };

        let tx_clone = tx.clone();
        let apps_snapshot: Vec<(usize, String)> = app
            .apps
            .iter()
            .enumerate()
            .map(|(i, a)| (i, a.url.clone()))
            .collect();
        let http_client = app.http_client.clone();

        tokio::spawn(async move {
            // Small delay so the user sees the splash
            tokio::time::sleep(Duration::from_millis(300)).await;

            let total = apps_snapshot.len();
            for (i, (idx, url)) in apps_snapshot.iter().enumerate() {
                let _ = tx_clone
                    .send(AppEvent::Progress(format!(
                        "Checking health {}/{}  {}",
                        i + 1,
                        total,
                        url
                    )))
                    .await;

                let status = check_health_static(&http_client, url).await;
                let app_name = fetch_app_name_static(&http_client, url).await;
                let _ = tx_clone
                    .send(AppEvent::HealthResult {
                        app_index: *idx,
                        status,
                        app_name,
                    })
                    .await;

                tokio::time::sleep(Duration::from_millis(200)).await;
            }

            let _ = tx_clone
                .send(AppEvent::Progress("Ready!".to_string()))
                .await;
            tokio::time::sleep(Duration::from_millis(400)).await;
            let _ = tx_clone.send(AppEvent::SplashDone).await;
        });
    }

    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    loop {
        // Draw
        terminal.draw(|f| ui::render(f, &app))?;

        // Poll for keyboard events
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = crossterm::event::read()? {
                // Global quit: Ctrl-C
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    app.should_quit = true;
                }

                match app.mode {
                    // -------------------------------------------------------
                    // NORMAL mode
                    // -------------------------------------------------------
                    Mode::Normal => {
                        if app.filter_active {
                            // Filter input mode
                            match key.code {
                                KeyCode::Enter | KeyCode::Esc => {
                                    if key.code == KeyCode::Esc {
                                        app.filter_text.clear();
                                    }
                                    app.filter_active = false;
                                }
                                KeyCode::Backspace => {
                                    app.filter_text.pop();
                                }
                                KeyCode::Char(c) => {
                                    app.filter_text.push(c);
                                }
                                _ => {}
                            }
                        } else {
                            let mut handled_g = false;
                            match key.code {
                                KeyCode::Esc => {
                                    if !app.filter_text.is_empty() {
                                        // Clear filter first
                                        app.filter_text.clear();
                                    } else if app.active_resource != "apps" {
                                        // Go back to apps view
                                        app.active_resource = "apps".to_string();
                                        app.filter_text.clear();
                                        app.filter_active = false;
                                    }
                                }
                                KeyCode::Char('j') | KeyCode::Down => app.next(),
                                KeyCode::Char('k') | KeyCode::Up => app.previous(),
                                KeyCode::Char('g') => {
                                    if let Some((KeyCode::Char('g'), last_time)) =
                                        app.last_key_press
                                    {
                                        if last_time.elapsed() < Duration::from_millis(250) {
                                            app.go_to_top();
                                            app.last_key_press = None;
                                        } else {
                                            app.last_key_press =
                                                Some((KeyCode::Char('g'), Instant::now()));
                                        }
                                    } else {
                                        app.last_key_press =
                                            Some((KeyCode::Char('g'), Instant::now()));
                                    }
                                    handled_g = true;
                                }
                                KeyCode::Char('G') | KeyCode::End => app.go_to_bottom(),
                                KeyCode::Home => app.go_to_top(),
                                KeyCode::Char(':') => {
                                    app.mode = Mode::Resources;
                                    app.command_text.clear();
                                    app.command_suggestion_selected = 0;
                                    app.update_command_suggestions();
                                }
                                KeyCode::Char('/') => {
                                    app.filter_active = true;
                                    app.filter_text.clear();
                                }
                                KeyCode::Char('a') => {
                                    // Add server (in apps view)
                                    if app.active_resource == "apps" {
                                        app.server_dialog_state = app::ServerDialogState::default();
                                        app.mode = Mode::ServerDialog;
                                    }
                                }
                                KeyCode::Char('d')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    // Delete app (in apps view)
                                    if app.active_resource == "apps" && !app.apps.is_empty() {
                                        let selected_app = &app.apps[app.selected_app_index];
                                        app.modal_title = "Confirm Delete".into();
                                        app.modal_msg = format!(
                                            "Delete '{}' ({})?",
                                            selected_app.name, selected_app.url
                                        );
                                        app.mode = Mode::Confirm;
                                    }
                                }
                                KeyCode::Char('R') => {
                                    // Refresh current resource
                                    let tx_clone = tx.clone();
                                    let resource = app.active_resource.clone();
                                    refresh_resource(&mut app, &resource, tx_clone).await;
                                }
                                KeyCode::Char('e') => {
                                    // Edit logger level (only in loggers view)
                                    if app.active_resource == "loggers" && !app.loggers.is_empty() {
                                        let logger = &app.loggers[app.selected_logger_index];
                                        let current = logger.effective_level.clone();
                                        let level_idx = app::LOG_LEVELS
                                            .iter()
                                            .position(|&l| l == current)
                                            .unwrap_or(2); // default to INFO
                                        app.edit_logger_state = app::EditLoggerState {
                                            logger_name: logger.name.clone(),
                                            current_level: current,
                                            selected_level_index: level_idx,
                                            error: None,
                                        };
                                        app.mode = Mode::EditLogger;
                                    }
                                }
                                KeyCode::Char('t') => {
                                    if app.active_resource == "apps" && !app.apps.is_empty() {
                                        // Thread dump from apps view — use hovered app
                                        let url = app.apps[app.selected_app_index].url.clone();
                                        let prev_active = app.config.active_app_url.clone();
                                        app.config.active_app_url = Some(url);
                                        match app.fetch_and_save_thread_dump().await {
                                            Ok(_path) => {
                                                app.active_resource = "threaddump".to_string();
                                                app.filter_text.clear();
                                                app.filter_active = false;
                                                app.scan_saved_dumps();
                                                app.selected_thread_dump_index = 0;
                                            }
                                            Err(e) => {
                                                app.describe_title = "Thread Dump — Error".into();
                                                app.describe_content = format!("{}", e);
                                                app.describe_scroll = 0;
                                                app.mode = Mode::Describe;
                                            }
                                        }
                                        app.config.active_app_url = prev_active;
                                    } else if app.active_resource == "threaddump" {
                                        // Thread dump from threaddump view — use active app
                                        match app.fetch_and_save_thread_dump().await {
                                            Ok(_path) => {
                                                app.scan_saved_dumps();
                                                app.selected_thread_dump_index = 0;
                                            }
                                            Err(e) => {
                                                app.describe_title = "Thread Dump — Error".into();
                                                app.describe_content = format!("{}", e);
                                                app.describe_scroll = 0;
                                                app.mode = Mode::Describe;
                                            }
                                        }
                                    }
                                }
                                KeyCode::Char('h') => {
                                    if app.active_resource == "apps" && !app.apps.is_empty() {
                                        // Heap dump from apps view — use hovered app
                                        let url = app.apps[app.selected_app_index].url.clone();
                                        let prev_active = app.config.active_app_url.clone();
                                        app.config.active_app_url = Some(url);
                                        match app.download_heap_dump().await {
                                            Ok(_path) => {
                                                app.active_resource = "heapdump".to_string();
                                                app.filter_text.clear();
                                                app.filter_active = false;
                                                app.scan_saved_dumps();
                                                app.selected_heap_dump_index = 0;
                                            }
                                            Err(e) => {
                                                app.describe_title = "Heap Dump — Error".into();
                                                app.describe_content = format!("{}", e);
                                                app.describe_scroll = 0;
                                                app.mode = Mode::Describe;
                                            }
                                        }
                                        app.config.active_app_url = prev_active;
                                    } else if app.active_resource == "heapdump" {
                                        // Heap dump from heapdump view — use active app
                                        match app.download_heap_dump().await {
                                            Ok(_path) => {
                                                app.scan_saved_dumps();
                                                app.selected_heap_dump_index = 0;
                                            }
                                            Err(e) => {
                                                app.describe_title = "Heap Dump — Error".into();
                                                app.describe_content = format!("{}", e);
                                                app.describe_scroll = 0;
                                                app.mode = Mode::Describe;
                                            }
                                        }
                                    }
                                }
                                KeyCode::Enter => {
                                    if app.active_resource == "apps" && !app.apps.is_empty() {
                                        let selected = &app.apps[app.selected_app_index];
                                        if selected.status == AppStatus::Down {
                                            app.show_error(format!(
                                                "Cannot connect to '{}'\n\nThe application is DOWN or unreachable.\nCheck that the server is running and try again.",
                                                selected.name
                                            ));
                                        } else {
                                            // Connect to the selected app and show dashboard
                                            let url = selected.url.clone();
                                            app.config.active_app_url = Some(url);
                                            app.active_resource = "dashboard".to_string();
                                            app.filter_text.clear();
                                            app.filter_active = false;
                                            if let Err(e) = app.fetch_dashboard().await {
                                                app.show_error(format!(
                                                    "Failed to fetch dashboard: {}",
                                                    e
                                                ));
                                            }
                                        }
                                    } else {
                                        handle_describe(&mut app);
                                    }
                                }
                                KeyCode::Char('d') => {
                                    handle_describe(&mut app);
                                }
                                KeyCode::Char('v') => {
                                    if app.active_resource == "heapdump"
                                        && !app.saved_heap_dumps.is_empty()
                                    {
                                        let dump =
                                            &app.saved_heap_dumps[app.selected_heap_dump_index];
                                        if let Err(e) = launch_visualvm_file(&dump.path) {
                                            app.show_error(e);
                                        }
                                    } else if app.active_resource == "threaddump"
                                        && !app.saved_thread_dumps.is_empty()
                                    {
                                        let dump =
                                            &app.saved_thread_dumps[app.selected_thread_dump_index];
                                        let tdump_path = dump
                                            .path
                                            .replace(".json", ".tdump")
                                            .replace(".txt", ".tdump");
                                        if std::path::Path::new(&tdump_path).exists() {
                                            if let Err(e) = launch_visualvm_file(&tdump_path) {
                                                app.show_error(e);
                                            }
                                        } else {
                                            app.show_error(
                                                "No .tdump file found. Take a new thread dump to generate one."
                                            );
                                        }
                                    }
                                }
                                KeyCode::Char('m') => {
                                    if app.active_resource == "heapdump"
                                        && !app.saved_heap_dumps.is_empty()
                                    {
                                        let dump =
                                            &app.saved_heap_dumps[app.selected_heap_dump_index];
                                        if let Err(e) = launch_eclipse_mat(&dump.path) {
                                            app.show_error(e);
                                        }
                                    }
                                }
                                KeyCode::Char('V') => {
                                    if app.active_resource == "dashboard" {
                                        match app.fetch_app_pid().await {
                                            Ok(pid) => {
                                                if let Err(e) = launch_visualvm_pid(&pid) {
                                                    app.show_error(e);
                                                }
                                            }
                                            Err(e) => {
                                                app.show_error(format!("{}", e));
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                            if !handled_g {
                                app.last_key_press = None;
                            }
                        }
                    }

                    // -------------------------------------------------------
                    // DESCRIBE mode
                    // -------------------------------------------------------
                    Mode::Describe => {
                        let mut handled_g = false;
                        match key.code {
                            KeyCode::Esc | KeyCode::Char('q') => {
                                // Return to ThreadViz if we came from there
                                if !app.parsed_threads.is_empty()
                                    && app.active_resource == "threaddump"
                                {
                                    app.mode = Mode::ThreadViz;
                                } else {
                                    app.mode = Mode::Normal;
                                }
                            }
                            KeyCode::Char('j') | KeyCode::Down => {
                                app.describe_scroll = app.describe_scroll.saturating_add(1);
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                app.describe_scroll = app.describe_scroll.saturating_sub(1);
                            }
                            KeyCode::Char('g') => {
                                if let Some((KeyCode::Char('g'), last_time)) = app.last_key_press {
                                    if last_time.elapsed() < Duration::from_millis(250) {
                                        app.describe_scroll = 0;
                                        app.last_key_press = None;
                                    } else {
                                        app.last_key_press =
                                            Some((KeyCode::Char('g'), Instant::now()));
                                    }
                                } else {
                                    app.last_key_press = Some((KeyCode::Char('g'), Instant::now()));
                                }
                                handled_g = true;
                            }
                            KeyCode::Char('G') | KeyCode::End => {
                                // Go to bottom (large value)
                                app.describe_scroll = u16::MAX / 2;
                            }
                            KeyCode::Home => {
                                app.describe_scroll = 0;
                            }
                            _ => {}
                        }
                        if !handled_g {
                            app.last_key_press = None;
                        }
                    }

                    // -------------------------------------------------------
                    // THREAD VIZ mode
                    // -------------------------------------------------------
                    Mode::ThreadViz => {
                        match key.code {
                            KeyCode::Esc | KeyCode::Char('q') => {
                                app.mode = Mode::Normal;
                            }
                            KeyCode::Char('j') | KeyCode::Down => {
                                if app.thread_viz_scroll + 1 < app.parsed_threads.len() {
                                    app.thread_viz_scroll += 1;
                                }
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                app.thread_viz_scroll = app.thread_viz_scroll.saturating_sub(1);
                            }
                            KeyCode::Char('G') | KeyCode::End => {
                                app.thread_viz_scroll = app.parsed_threads.len().saturating_sub(1);
                            }
                            KeyCode::Home => {
                                app.thread_viz_scroll = 0;
                            }
                            KeyCode::Enter | KeyCode::Char('d') => {
                                // Show stack trace of selected thread in describe
                                if let Some(t) = app.parsed_threads.get(app.thread_viz_scroll) {
                                    app.describe_title =
                                        format!("\"{}\" #{} {}", t.name, t.id, t.state);
                                    let mut content = format!(
                                        "Thread: {}\nID:     {}\nState:  {}\nDaemon: {}\n\n",
                                        t.name,
                                        t.id,
                                        t.state,
                                        if t.daemon { "yes" } else { "no" }
                                    );
                                    if t.stack_trace.is_empty() {
                                        content.push_str("(no stack trace)");
                                    } else {
                                        content.push_str("Stack Trace:\n");
                                        for frame in &t.stack_trace {
                                            let location = if frame.native_method {
                                                "Native Method".to_string()
                                            } else if !frame.file_name.is_empty() {
                                                if frame.line_number >= 0 {
                                                    format!(
                                                        "{}:{}",
                                                        frame.file_name, frame.line_number
                                                    )
                                                } else {
                                                    frame.file_name.clone()
                                                }
                                            } else {
                                                "Unknown Source".to_string()
                                            };
                                            content.push_str(&format!(
                                                "  at {}.{}({})\n",
                                                frame.class_name, frame.method_name, location
                                            ));
                                        }
                                    }
                                    app.describe_content = content;
                                    app.describe_scroll = 0;
                                    app.mode = Mode::Describe;
                                }
                            }
                            _ => {}
                        }
                    }

                    // -------------------------------------------------------
                    // CONFIRM mode (modal dialog)
                    // -------------------------------------------------------
                    Mode::Confirm => match key.code {
                        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                            app.mode = Mode::Normal;
                        }
                        KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                            // Execute the confirmed action
                            handle_confirm_action(&mut app);
                            app.mode = Mode::Normal;
                        }
                        _ => {}
                    },

                    // -------------------------------------------------------
                    // ERROR MODAL mode
                    // -------------------------------------------------------
                    Mode::ErrorModal => match key.code {
                        KeyCode::Enter | KeyCode::Esc => {
                            app.mode = app.error_prev_mode.take().unwrap_or(Mode::Normal);
                        }
                        _ => {}
                    },

                    // -------------------------------------------------------
                    // RESOURCES mode (command palette)
                    // -------------------------------------------------------
                    Mode::Resources => match key.code {
                        KeyCode::Esc => {
                            app.mode = Mode::Normal;
                            app.command_text.clear();
                            app.update_command_suggestions();
                        }
                        KeyCode::Backspace => {
                            app.command_text.pop();
                            app.update_command_suggestions();
                        }
                        KeyCode::Down => {
                            if !app.command_suggestions.is_empty() {
                                app.command_suggestion_selected = (app.command_suggestion_selected
                                    + 1)
                                    % app.command_suggestions.len();
                            }
                        }
                        KeyCode::Up => {
                            if !app.command_suggestions.is_empty() {
                                if app.command_suggestion_selected > 0 {
                                    app.command_suggestion_selected -= 1;
                                } else {
                                    app.command_suggestion_selected =
                                        app.command_suggestions.len() - 1;
                                }
                            }
                        }
                        KeyCode::Right | KeyCode::Tab => {
                            // Autocomplete: fill in the selected command text
                            if let Some(selected) = app.get_selected_command() {
                                app.command_text = selected.command.clone();
                                app.update_command_suggestions();
                            }
                        }
                        KeyCode::Char(c) => {
                            app.command_text.push(c);
                            app.update_command_suggestions();
                        }
                        KeyCode::Enter => {
                            if let Some(selected) = app.get_selected_command().cloned() {
                                handle_resource_switch(&mut app, &selected, tx.clone()).await;
                            }
                        }
                        _ => {}
                    },

                    // -------------------------------------------------------
                    // SERVER DIALOG mode
                    // -------------------------------------------------------
                    Mode::ServerDialog => {
                        match app.server_dialog_state.phase {
                            app::ServerDialogPhase::ChooseMethod => match key.code {
                                KeyCode::Esc => {
                                    if app.apps.is_empty() {
                                        app.should_quit = true;
                                    } else {
                                        app.mode = Mode::Normal;
                                    }
                                }
                                KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => {
                                    app.server_dialog_state.method_selected =
                                        (app.server_dialog_state.method_selected + 1) % 2;
                                }
                                KeyCode::Char('k') | KeyCode::Up => {
                                    app.server_dialog_state.method_selected =
                                        if app.server_dialog_state.method_selected == 0 {
                                            1
                                        } else {
                                            0
                                        };
                                }
                                KeyCode::Enter => {
                                    if app.server_dialog_state.method_selected == 0 {
                                        // Manual entry
                                        app.server_dialog_state.phase =
                                            app::ServerDialogPhase::ManualEntry;
                                        app.server_dialog_state.active_field = 0;
                                        app.server_dialog_state.name.clear();
                                        app.server_dialog_state.url.clear();
                                        app.server_dialog_state.error = None;
                                    } else {
                                        // Scan local
                                        app.server_dialog_state.phase =
                                            app::ServerDialogPhase::Scanning;
                                        app.server_dialog_state.discovered_apps.clear();
                                        app.server_dialog_state.scan_done = false;
                                        app.server_dialog_state.scan_progress =
                                            "Starting scan...".into();

                                        let tx_clone = tx.clone();
                                        let http_client = app.http_client.clone();
                                        tokio::spawn(async move {
                                            scan_local_ports(tx_clone, http_client).await;
                                        });
                                    }
                                }
                                _ => {}
                            },
                            app::ServerDialogPhase::ManualEntry => match key.code {
                                KeyCode::Esc => {
                                    // Go back to choose method
                                    app.server_dialog_state.phase =
                                        app::ServerDialogPhase::ChooseMethod;
                                    app.server_dialog_state.error = None;
                                }
                                KeyCode::Tab | KeyCode::Down | KeyCode::Up => {
                                    app.server_dialog_state.active_field =
                                        if app.server_dialog_state.active_field == 0 {
                                            1
                                        } else {
                                            0
                                        };
                                }
                                KeyCode::Backspace => match app.server_dialog_state.active_field {
                                    0 => {
                                        app.server_dialog_state.name.pop();
                                    }
                                    _ => {
                                        app.server_dialog_state.url.pop();
                                    }
                                },
                                KeyCode::Char(c) => match app.server_dialog_state.active_field {
                                    0 => app.server_dialog_state.name.push(c),
                                    _ => app.server_dialog_state.url.push(c),
                                },
                                KeyCode::Enter => {
                                    handle_server_dialog_submit(&mut app, tx.clone());
                                }
                                _ => {}
                            },
                            app::ServerDialogPhase::Scanning => match key.code {
                                KeyCode::Esc => {
                                    // Cancel scan and go back
                                    app.server_dialog_state.phase =
                                        app::ServerDialogPhase::ChooseMethod;
                                    app.server_dialog_state.discovered_apps.clear();
                                }
                                _ => {
                                    // Ignore other keys while scanning
                                }
                            },
                            app::ServerDialogPhase::ScanResults => match key.code {
                                KeyCode::Esc => {
                                    app.server_dialog_state.phase =
                                        app::ServerDialogPhase::ChooseMethod;
                                    app.server_dialog_state.discovered_apps.clear();
                                }
                                KeyCode::Char('j') | KeyCode::Down => {
                                    if !app.server_dialog_state.discovered_apps.is_empty() {
                                        app.server_dialog_state.scan_selected_index =
                                            (app.server_dialog_state.scan_selected_index + 1).min(
                                                app.server_dialog_state.discovered_apps.len() - 1,
                                            );
                                    }
                                }
                                KeyCode::Char('k') | KeyCode::Up => {
                                    app.server_dialog_state.scan_selected_index = app
                                        .server_dialog_state
                                        .scan_selected_index
                                        .saturating_sub(1);
                                }
                                KeyCode::Enter => {
                                    handle_scan_result_select(&mut app, tx.clone());
                                }
                                _ => {}
                            },
                        }
                    }

                    // -------------------------------------------------------
                    // EDIT LOGGER mode
                    // -------------------------------------------------------
                    Mode::EditLogger => match key.code {
                        KeyCode::Esc => {
                            app.mode = Mode::Normal;
                        }
                        KeyCode::Char('j') | KeyCode::Down => {
                            let max = app::LOG_LEVELS.len() - 1;
                            app.edit_logger_state.selected_level_index =
                                (app.edit_logger_state.selected_level_index + 1).min(max);
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            app.edit_logger_state.selected_level_index =
                                app.edit_logger_state.selected_level_index.saturating_sub(1);
                        }
                        KeyCode::Enter => {
                            let level = app::LOG_LEVELS[app.edit_logger_state.selected_level_index];
                            let logger_name = app.edit_logger_state.logger_name.clone();
                            match app.set_logger_level(&logger_name, level).await {
                                Ok(()) => {
                                    app.mode = Mode::Normal;
                                }
                                Err(e) => {
                                    app.edit_logger_state.error = Some(format!("Failed: {}", e));
                                }
                            }
                        }
                        _ => {}
                    },

                    // -------------------------------------------------------
                    // NEW PROJECT wizard mode
                    // -------------------------------------------------------
                    Mode::NewProject => {
                        handle_new_project_key(&mut app, key.code, tx.clone()).await;
                    }

                    // -------------------------------------------------------
                    // SPLASH mode — ignore most keys
                    // -------------------------------------------------------
                    Mode::Splash => {
                        // Only allow Ctrl-C (handled globally above)
                    }
                }
            }
        }

        // Tick
        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            // Advance splash spinner
            if app.mode == Mode::Splash {
                app.splash_state.spinner_frame = app.splash_state.spinner_frame.wrapping_add(1);
            }
            last_tick = Instant::now();
        }

        // Process async events (non-blocking)
        while let Ok(event) = rx.try_recv() {
            match event {
                AppEvent::Progress(msg) => {
                    app.splash_state.current_message = msg;
                    app.splash_state.current_step = app
                        .splash_state
                        .current_step
                        .saturating_add(1)
                        .min(app.splash_state.total_steps);
                }
                AppEvent::HealthResult {
                    app_index,
                    status,
                    app_name,
                } => {
                    if app_index < app.apps.len() {
                        app.apps[app_index].status = status;
                        if let Some(name) = app_name {
                            app.apps[app_index].name = name.clone();
                            // Also update the config so the name persists
                            let url = app.apps[app_index].url.clone();
                            app.config.add_app(name, url);
                            let _ = app.config.save();
                        }
                    }
                }
                AppEvent::SplashDone => {
                    app.splash_state.done = true;
                    app.splash_state.current_step = app.splash_state.total_steps;
                    if app.apps.is_empty() {
                        app.mode = Mode::ServerDialog;
                    } else {
                        app.mode = Mode::Normal;
                    }
                }
                #[allow(dead_code)]
                AppEvent::DataFetched { resource, result } => {
                    if let Err(err_msg) = result {
                        app.show_error(format!("Failed to fetch {}: {}", resource, err_msg));
                    }
                }
                AppEvent::ScanProgress(msg) => {
                    app.server_dialog_state.scan_progress = msg;
                }
                AppEvent::ScanFound { url, port, status } => {
                    app.server_dialog_state
                        .discovered_apps
                        .push(app::DiscoveredApp { url, port, status });
                }
                AppEvent::ScanDone => {
                    app.server_dialog_state.scan_done = true;
                    app.server_dialog_state.scan_progress = "Scan complete.".into();
                    app.server_dialog_state.phase = app::ServerDialogPhase::ScanResults;
                    app.server_dialog_state.scan_selected_index = 0;
                }
                AppEvent::MetadataLoaded(meta) => {
                    app.new_project_state.loading_metadata = false;
                    app.new_project_state.apply_metadata_defaults(&meta);
                    app.new_project_state.metadata = Some(*meta);
                    app.new_project_state.error = None;
                }
                AppEvent::MetadataFailed(err) => {
                    app.new_project_state.loading_metadata = false;
                    app.new_project_state.error = Some(format!("Failed to load metadata: {}", err));
                }
                AppEvent::GenerateResult(result) => match result {
                    Ok(path) => {
                        app.new_project_state.gen_done = true;
                        app.new_project_state.gen_progress =
                            format!("Project created at: {}", path);
                        app.new_project_state.gen_result_path = Some(path);
                    }
                    Err(err) => {
                        app.new_project_state.step = app::WizardStep::Confirm;
                        app.new_project_state.error = Some(format!("Generation failed: {}", err));
                    }
                },
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Helper: static health check (doesn't borrow App)
// ---------------------------------------------------------------------------

async fn check_health_static(client: &reqwest::Client, url: &str) -> AppStatus {
    let endpoint = format!("{}/actuator/health", url.trim_end_matches('/'));
    match client.get(&endpoint).send().await {
        Ok(resp) => {
            if !resp.status().is_success() {
                return AppStatus::Down;
            }
            match resp.json::<serde_json::Value>().await {
                Ok(body) => match body.get("status").and_then(|s| s.as_str()) {
                    Some("UP") => AppStatus::Up,
                    Some("DOWN") => AppStatus::Down,
                    _ => AppStatus::Unknown,
                },
                Err(_) => AppStatus::Unknown,
            }
        }
        Err(_) => AppStatus::Down,
    }
}

/// Fetch `spring.application.name` from actuator. Returns `None` if unavailable.
async fn fetch_app_name_static(client: &reqwest::Client, url: &str) -> Option<String> {
    let endpoint = format!(
        "{}/actuator/env/spring.application.name",
        url.trim_end_matches('/')
    );
    let resp = client.get(&endpoint).send().await.ok()?;
    let body: serde_json::Value = resp.json().await.ok()?;
    let value = body
        .get("property")
        .and_then(|p| p.get("value"))
        .and_then(|v| v.as_str())?;
    if value.is_empty() || value.contains('*') {
        None
    } else {
        Some(value.to_string())
    }
}

// ---------------------------------------------------------------------------
// Helper: handle Describe action based on active resource
// ---------------------------------------------------------------------------

/// Parse thread dump JSON into structured ThreadInfo vec.
fn parse_thread_dump_json(body: &serde_json::Value) -> Vec<crate::model::ThreadInfo> {
    let mut threads = Vec::new();
    if let Some(arr) = body.get("threads").and_then(|t| t.as_array()) {
        for thread in arr {
            let name = thread
                .get("threadName")
                .and_then(|n| n.as_str())
                .unwrap_or("unknown")
                .to_string();
            let id = thread.get("threadId").and_then(|i| i.as_i64()).unwrap_or(0);
            let state = thread
                .get("threadState")
                .and_then(|s| s.as_str())
                .unwrap_or("UNKNOWN")
                .to_string();
            let daemon = thread
                .get("daemon")
                .and_then(|d| d.as_bool())
                .unwrap_or(false);

            let stack_trace = thread
                .get("stackTrace")
                .and_then(|s| s.as_array())
                .map(|frames| {
                    frames
                        .iter()
                        .map(|f| crate::model::StackFrame {
                            class_name: f
                                .get("className")
                                .and_then(|c| c.as_str())
                                .unwrap_or("Unknown")
                                .to_string(),
                            method_name: f
                                .get("methodName")
                                .and_then(|m| m.as_str())
                                .unwrap_or("unknown")
                                .to_string(),
                            file_name: f
                                .get("fileName")
                                .and_then(|f| f.as_str())
                                .unwrap_or("")
                                .to_string(),
                            line_number: f.get("lineNumber").and_then(|l| l.as_i64()).unwrap_or(-1),
                            native_method: f
                                .get("nativeMethod")
                                .and_then(|n| n.as_bool())
                                .unwrap_or(false),
                        })
                        .collect()
                })
                .unwrap_or_default();

            threads.push(crate::model::ThreadInfo {
                name,
                id,
                state,
                daemon,
                stack_trace,
            });
        }
    }
    // Sort: RUNNABLE first, then BLOCKED, TIMED_WAITING, WAITING, rest
    threads.sort_by(|a, b| {
        fn state_order(s: &str) -> u8 {
            match s.to_uppercase().as_str() {
                "RUNNABLE" => 0,
                "BLOCKED" => 1,
                "TIMED_WAITING" => 2,
                "WAITING" => 3,
                _ => 4,
            }
        }
        state_order(&a.state)
            .cmp(&state_order(&b.state))
            .then(a.name.cmp(&b.name))
    });
    threads
}

/// Format a thread dump JSON (from /actuator/threaddump) into readable text.
#[allow(dead_code)]
fn format_thread_dump_json(body: &serde_json::Value) -> String {
    let mut output = String::new();
    if let Some(threads) = body.get("threads").and_then(|t| t.as_array()) {
        output.push_str(&format!("Thread Dump — {} threads\n", threads.len()));
        output.push_str(&"─".repeat(60));
        output.push('\n');

        for thread in threads {
            let name = thread
                .get("threadName")
                .and_then(|n| n.as_str())
                .unwrap_or("unknown");
            let state = thread
                .get("threadState")
                .and_then(|s| s.as_str())
                .unwrap_or("UNKNOWN");
            let id = thread.get("threadId").and_then(|i| i.as_i64()).unwrap_or(0);

            output.push_str(&format!("\n\"{}\" #{} {}\n", name, id, state));

            if let Some(stack) = thread.get("stackTrace").and_then(|s| s.as_array()) {
                for frame in stack {
                    let class = frame
                        .get("className")
                        .and_then(|c| c.as_str())
                        .unwrap_or("?");
                    let method = frame
                        .get("methodName")
                        .and_then(|m| m.as_str())
                        .unwrap_or("?");
                    let line = frame
                        .get("lineNumber")
                        .and_then(|l| l.as_i64())
                        .unwrap_or(-1);
                    output.push_str(&format!("    at {}.{}({})\n", class, method, line));
                }
            }
        }
    } else {
        output = serde_json::to_string_pretty(body)
            .unwrap_or_else(|_| "Could not format thread dump".into());
    }
    output
}

fn handle_describe(app: &mut App) {
    match app.active_resource.as_str() {
        "apps" => {
            if let Some(a) = app.apps.get(app.selected_app_index) {
                app.describe_title = format!("App: {}", a.name);
                app.describe_content = format!(
                    "Name:    {}\nURL:     {}\nStatus:  {}",
                    a.name, a.url, a.status
                );
                app.describe_scroll = 0;
                app.mode = Mode::Describe;
            }
        }
        "endpoints" => {
            if let Some(ep) = app.endpoints.get(app.selected_endpoint_index) {
                app.describe_title = format!("Endpoint: {}", ep.name);
                app.describe_content = format!("Name:  {}\nURL:   {}", ep.name, ep.url);
                app.describe_scroll = 0;
                app.mode = Mode::Describe;
            }
        }
        "beans" => {
            if let Some(bean) = app.beans.get(app.selected_bean_index) {
                app.describe_title = format!("Bean: {}", bean.name);
                app.describe_content = format!(
                    "Name:   {}\nType:   {}\nScope:  {}",
                    bean.name, bean.type_name, bean.scope
                );
                app.describe_scroll = 0;
                app.mode = Mode::Describe;
            }
        }
        "loggers" => {
            if let Some(logger) = app.loggers.get(app.selected_logger_index) {
                let configured = logger.configured_level.as_deref().unwrap_or("(not set)");
                app.describe_title = format!("Logger: {}", logger.name);
                app.describe_content = format!(
                    "Name:              {}\nConfigured Level:  {}\nEffective Level:   {}",
                    logger.name, configured, logger.effective_level
                );
                app.describe_scroll = 0;
                app.mode = Mode::Describe;
            }
        }
        "mappings" => {
            if let Some(mapping) = app.mappings.get(app.selected_mapping_index) {
                app.describe_title = "Mapping".into();
                app.describe_content = format!(
                    "Pattern:  {}\nHandler:  {}",
                    mapping.pattern, mapping.handler
                );
                app.describe_scroll = 0;
                app.mode = Mode::Describe;
            }
        }
        "env" => {
            if let Some(prop) = app.env_props.get(app.selected_env_index) {
                app.describe_title = format!("Env: {}", prop.name);
                app.describe_content = format!(
                    "Name:    {}\nValue:   {}\nSource:  {}",
                    prop.name, prop.value, prop.source
                );
                app.describe_scroll = 0;
                app.mode = Mode::Describe;
            }
        }
        "threaddump" => {
            if let Some(dump) = app.saved_thread_dumps.get(app.selected_thread_dump_index) {
                let path = dump.path.clone();
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        if let Ok(body) = serde_json::from_str::<serde_json::Value>(&content) {
                            app.parsed_threads = parse_thread_dump_json(&body);
                            app.thread_viz_scroll = 0;
                            app.thread_viz_title =
                                format!("Thread Dump: {} — {}", dump.app_name, dump.timestamp);
                            app.mode = Mode::ThreadViz;
                        } else {
                            // Old .txt files — fall back to text describe
                            app.describe_title = format!("Thread Dump: {}", dump.timestamp);
                            app.describe_content = content;
                            app.describe_scroll = 0;
                            app.mode = Mode::Describe;
                        }
                    }
                    Err(e) => {
                        app.describe_title = "Thread Dump — Error".into();
                        app.describe_content = format!("Failed to read {}: {}", path, e);
                        app.describe_scroll = 0;
                        app.mode = Mode::Describe;
                    }
                }
            }
        }
        "heapdump" => {
            if let Some(dump) = app.saved_heap_dumps.get(app.selected_heap_dump_index) {
                app.describe_title = format!("Heap Dump: {}", dump.timestamp);
                app.describe_content = format!(
                    "Path:       {}\nSize:       {:.1} MB\nTimestamp:  {}\nApp:        {}\n\nAnalyze with:\n  VisualVM     [v]\n  Eclipse MAT  [m]",
                    dump.path,
                    dump.size_bytes as f64 / 1_048_576.0,
                    dump.timestamp,
                    if dump.app_name.is_empty() { "—" } else { &dump.app_name },
                );
                app.describe_scroll = 0;
                app.mode = Mode::Describe;
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Helper: handle confirm action (e.g. delete server)
// ---------------------------------------------------------------------------

fn handle_confirm_action(app: &mut App) {
    if app.active_resource == "apps" && !app.apps.is_empty() {
        let url = app.apps[app.selected_app_index].url.clone();
        app.config.remove_app(&url);
        let _ = app.config.save();
        app.apps.remove(app.selected_app_index);
        if app.selected_app_index >= app.apps.len() && app.selected_app_index > 0 {
            app.selected_app_index -= 1;
        }
    }
}

// ---------------------------------------------------------------------------
// Helper: switch to a resource from the command palette
// ---------------------------------------------------------------------------

async fn handle_resource_switch(
    app: &mut App,
    selected: &ResourceItem,
    _tx: mpsc::Sender<AppEvent>,
) {
    // Handle :new command specially — it opens the new project wizard
    if selected.command == ":new" {
        app.mode = Mode::NewProject;
        app.new_project_state = app::NewProjectWizardState::default();
        app.new_project_state.loading_metadata = true;
        app.command_text.clear();
        app.update_command_suggestions();

        // Kick off metadata fetch
        let tx_clone = _tx.clone();
        let http_client = app.http_client.clone();
        tokio::spawn(async move {
            match App::fetch_initializr_metadata(&http_client).await {
                Ok(meta) => {
                    let _ = tx_clone
                        .send(AppEvent::MetadataLoaded(Box::new(meta)))
                        .await;
                }
                Err(e) => {
                    let _ = tx_clone
                        .send(AppEvent::MetadataFailed(format!("{}", e)))
                        .await;
                }
            }
        });
        return;
    }

    let resource_key = match selected.command.as_str() {
        ":apps" => "apps",
        ":dashboard" => "dashboard",
        ":endpoints" => "endpoints",
        ":beans" => "beans",
        ":loggers" => "loggers",
        ":mappings" => "mappings",
        ":env" => "env",
        ":threaddump" => "threaddump",
        ":heapdump" => "heapdump",
        _ => "apps",
    };

    app.active_resource = resource_key.to_string();
    app.mode = Mode::Normal;
    app.command_text.clear();
    app.update_command_suggestions();

    // Trigger fetch based on the selected resource
    match resource_key {
        "dashboard" => {
            if let Err(e) = app.fetch_dashboard().await {
                app.show_error(format!("Failed to fetch dashboard: {}", e));
            }
        }
        "endpoints" => {
            if let Err(e) = app.fetch_endpoints().await {
                app.show_error(format!("Failed to fetch endpoints: {}", e));
            }
        }
        "beans" => {
            if let Err(e) = app.fetch_beans().await {
                app.show_error(format!("Failed to fetch beans: {}", e));
            }
        }
        "loggers" => {
            if let Err(e) = app.fetch_loggers().await {
                app.show_error(format!("Failed to fetch loggers: {}", e));
            }
        }
        "mappings" => {
            if let Err(e) = app.fetch_mappings().await {
                app.show_error(format!("Failed to fetch mappings: {}", e));
            }
        }
        "env" => {
            if let Err(e) = app.fetch_env().await {
                app.show_error(format!("Failed to fetch env: {}", e));
            }
        }
        "threaddump" | "heapdump" => {
            // Rescan the dumps directory to pick up any new files
            app.scan_saved_dumps();
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Helper: refresh the current resource
// ---------------------------------------------------------------------------

async fn refresh_resource(app: &mut App, resource: &str, tx: mpsc::Sender<AppEvent>) {
    match resource {
        "apps" => {
            // Re-run health checks
            let apps_snapshot: Vec<(usize, String)> = app
                .apps
                .iter()
                .enumerate()
                .map(|(i, a)| (i, a.url.clone()))
                .collect();
            let http_client = app.http_client.clone();
            let tx_clone = tx;

            tokio::spawn(async move {
                for (idx, url) in &apps_snapshot {
                    let status = check_health_static(&http_client, url).await;
                    let app_name = fetch_app_name_static(&http_client, url).await;
                    let _ = tx_clone
                        .send(AppEvent::HealthResult {
                            app_index: *idx,
                            status,
                            app_name,
                        })
                        .await;
                }
            });
        }
        "dashboard" => {
            if let Err(e) = app.fetch_dashboard().await {
                app.show_error(format!("Refresh failed: {}", e));
            }
        }
        "endpoints" => {
            if let Err(e) = app.fetch_endpoints().await {
                app.show_error(format!("Refresh failed: {}", e));
            }
        }
        "beans" => {
            if let Err(e) = app.fetch_beans().await {
                app.show_error(format!("Refresh failed: {}", e));
            }
        }
        "loggers" => {
            if let Err(e) = app.fetch_loggers().await {
                app.show_error(format!("Refresh failed: {}", e));
            }
        }
        "mappings" => {
            if let Err(e) = app.fetch_mappings().await {
                app.show_error(format!("Refresh failed: {}", e));
            }
        }
        "env" => {
            if let Err(e) = app.fetch_env().await {
                app.show_error(format!("Refresh failed: {}", e));
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Helper: handle server dialog submit (Enter key)
// ---------------------------------------------------------------------------

fn handle_server_dialog_submit(app: &mut App, tx: mpsc::Sender<AppEvent>) {
    let name = app.server_dialog_state.name.trim().to_string();
    let url = app.server_dialog_state.url.trim().to_string();

    if name.is_empty() || url.is_empty() {
        app.server_dialog_state.error = Some("Please fill in all fields".into());
        return;
    }

    // Validate URL format (basic check)
    if !url.starts_with("http://") && !url.starts_with("https://") {
        app.server_dialog_state.error = Some("URL must start with http:// or https://".into());
        return;
    }

    // Add to config and save
    app.config.add_app(name.clone(), url.clone());
    if let Err(e) = app.config.save() {
        app.server_dialog_state.error = Some(format!("Failed to save: {}", e));
        return;
    }

    // Add to runtime list
    app.apps.push(model::SpringApp {
        name,
        url: url.clone(),
        status: AppStatus::Unknown,
    });

    // Clear dialog state
    app.server_dialog_state = app::ServerDialogState::default();

    // Run a health check on the newly added app
    let app_index = app.apps.len() - 1;
    let http_client = app.http_client.clone();
    let tx_clone = tx;

    tokio::spawn(async move {
        let status = check_health_static(&http_client, &url).await;
        let app_name = fetch_app_name_static(&http_client, &url).await;
        let _ = tx_clone
            .send(AppEvent::HealthResult {
                app_index,
                status,
                app_name,
            })
            .await;
    });

    // Switch to normal mode
    app.mode = Mode::Normal;
}

// ---------------------------------------------------------------------------
// Helper: handle selecting a discovered app from scan results
// ---------------------------------------------------------------------------

fn handle_scan_result_select(app: &mut App, tx: mpsc::Sender<AppEvent>) {
    if app.server_dialog_state.discovered_apps.is_empty() {
        return;
    }

    let discovered =
        &app.server_dialog_state.discovered_apps[app.server_dialog_state.scan_selected_index];
    let url = discovered.url.clone();
    let port = discovered.port;
    let name = format!("localhost:{}", port);

    // Add to config and save
    app.config.add_app(name.clone(), url.clone());
    if let Err(e) = app.config.save() {
        app.server_dialog_state.error = Some(format!("Failed to save: {}", e));
        return;
    }

    // Add to runtime list
    app.apps.push(model::SpringApp {
        name,
        url: url.clone(),
        status: discovered.status.clone(),
    });

    // Clear dialog state
    app.server_dialog_state = app::ServerDialogState::default();

    // Run a health check on the newly added app
    let app_index = app.apps.len() - 1;
    let http_client = app.http_client.clone();

    tokio::spawn(async move {
        let status = check_health_static(&http_client, &url).await;
        let app_name = fetch_app_name_static(&http_client, &url).await;
        let _ = tx
            .send(AppEvent::HealthResult {
                app_index,
                status,
                app_name,
            })
            .await;
    });

    app.mode = Mode::Normal;
}

// ---------------------------------------------------------------------------
// New Project wizard key handling
// ---------------------------------------------------------------------------

async fn handle_new_project_key(app: &mut App, key: KeyCode, tx: mpsc::Sender<AppEvent>) {
    use app::WizardStep;

    match app.new_project_state.step {
        WizardStep::ProjectInfo => {
            if app.new_project_state.loading_metadata {
                // Only allow Esc while loading
                if key == KeyCode::Esc {
                    app.mode = Mode::Normal;
                    app.new_project_state = app::NewProjectWizardState::default();
                }
                return;
            }

            let meta = match &app.new_project_state.metadata {
                Some(m) => m.clone(),
                None => {
                    if key == KeyCode::Esc {
                        app.mode = Mode::Normal;
                    }
                    return;
                }
            };

            let field = app.new_project_state.active_field;
            let is_select = field <= 4; // fields 0-4 are select fields

            match key {
                KeyCode::Esc => {
                    app.mode = Mode::Normal;
                    app.new_project_state = app::NewProjectWizardState::default();
                }
                KeyCode::Tab | KeyCode::Down => {
                    app.new_project_state.active_field =
                        (app.new_project_state.active_field + 1) % 10;
                }
                KeyCode::BackTab | KeyCode::Up => {
                    app.new_project_state.active_field = if app.new_project_state.active_field == 0
                    {
                        9
                    } else {
                        app.new_project_state.active_field - 1
                    };
                }
                KeyCode::Left if is_select => {
                    // Cycle select field backward
                    match field {
                        0 => {
                            let max = meta.boot_versions.len().max(1) - 1;
                            app.new_project_state.boot_version_idx =
                                if app.new_project_state.boot_version_idx == 0 {
                                    max
                                } else {
                                    app.new_project_state.boot_version_idx - 1
                                };
                        }
                        1 => {
                            let max = meta.languages.len().max(1) - 1;
                            app.new_project_state.language_idx =
                                if app.new_project_state.language_idx == 0 {
                                    max
                                } else {
                                    app.new_project_state.language_idx - 1
                                };
                        }
                        2 => {
                            let max = meta.packagings.len().max(1) - 1;
                            app.new_project_state.packaging_idx =
                                if app.new_project_state.packaging_idx == 0 {
                                    max
                                } else {
                                    app.new_project_state.packaging_idx - 1
                                };
                        }
                        3 => {
                            let max = meta.java_versions.len().max(1) - 1;
                            app.new_project_state.java_version_idx =
                                if app.new_project_state.java_version_idx == 0 {
                                    max
                                } else {
                                    app.new_project_state.java_version_idx - 1
                                };
                        }
                        4 => {
                            let max = meta.project_types.len().max(1) - 1;
                            app.new_project_state.project_type_idx =
                                if app.new_project_state.project_type_idx == 0 {
                                    max
                                } else {
                                    app.new_project_state.project_type_idx - 1
                                };
                        }
                        _ => {}
                    }
                }
                KeyCode::Right if is_select => {
                    // Cycle select field forward
                    match field {
                        0 => {
                            app.new_project_state.boot_version_idx =
                                (app.new_project_state.boot_version_idx + 1)
                                    % meta.boot_versions.len().max(1);
                        }
                        1 => {
                            app.new_project_state.language_idx =
                                (app.new_project_state.language_idx + 1)
                                    % meta.languages.len().max(1);
                        }
                        2 => {
                            app.new_project_state.packaging_idx =
                                (app.new_project_state.packaging_idx + 1)
                                    % meta.packagings.len().max(1);
                        }
                        3 => {
                            app.new_project_state.java_version_idx =
                                (app.new_project_state.java_version_idx + 1)
                                    % meta.java_versions.len().max(1);
                        }
                        4 => {
                            app.new_project_state.project_type_idx =
                                (app.new_project_state.project_type_idx + 1)
                                    % meta.project_types.len().max(1);
                        }
                        _ => {}
                    }
                }
                KeyCode::Backspace if !is_select => {
                    // Delete from text field
                    match field {
                        5 => {
                            app.new_project_state.group_id.pop();
                        }
                        6 => {
                            app.new_project_state.artifact_id.pop();
                        }
                        7 => {
                            app.new_project_state.name.pop();
                        }
                        8 => {
                            app.new_project_state.description.pop();
                        }
                        9 => {
                            app.new_project_state.package_name.pop();
                        }
                        _ => {}
                    }
                }
                KeyCode::Char(c) if !is_select => {
                    // Type into text field
                    match field {
                        5 => app.new_project_state.group_id.push(c),
                        6 => app.new_project_state.artifact_id.push(c),
                        7 => app.new_project_state.name.push(c),
                        8 => app.new_project_state.description.push(c),
                        9 => app.new_project_state.package_name.push(c),
                        _ => {}
                    }
                }
                KeyCode::Enter => {
                    // Move to Dependencies step
                    app.new_project_state.step = WizardStep::Dependencies;
                    app.new_project_state.dep_group_idx = 0;
                    app.new_project_state.dep_item_idx = 0;
                }
                _ => {}
            }
        }
        WizardStep::Dependencies => {
            let meta = match &app.new_project_state.metadata {
                Some(m) => m.clone(),
                None => return,
            };

            if app.new_project_state.dep_filter_active {
                // Filter input mode
                match key {
                    KeyCode::Esc => {
                        app.new_project_state.dep_filter_active = false;
                        app.new_project_state.dep_filter.clear();
                    }
                    KeyCode::Enter => {
                        app.new_project_state.dep_filter_active = false;
                    }
                    KeyCode::Backspace => {
                        app.new_project_state.dep_filter.pop();
                    }
                    KeyCode::Char(c) => {
                        app.new_project_state.dep_filter.push(c);
                    }
                    _ => {}
                }
                // After filter text changes, jump to first selectable item
                let first_selectable = {
                    let count = crate::ui::new_project::flat_dep_count(
                        &meta,
                        &app.new_project_state.dep_filter,
                    );
                    (0..count)
                        .find(|&i| {
                            crate::ui::new_project::flat_dep_is_selectable(
                                &meta,
                                &app.new_project_state.dep_filter,
                                i,
                            )
                        })
                        .unwrap_or(0)
                };
                app.new_project_state.dep_item_idx = first_selectable;
                return;
            }

            let total =
                crate::ui::new_project::flat_dep_count(&meta, &app.new_project_state.dep_filter);

            match key {
                KeyCode::Esc => {
                    app.new_project_state.step = WizardStep::ProjectInfo;
                }
                KeyCode::Char('/') => {
                    app.new_project_state.dep_filter_active = true;
                    app.new_project_state.dep_filter.clear();
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    if total > 0 {
                        let mut next = app.new_project_state.dep_item_idx + 1;
                        // Skip group headers
                        while next < total
                            && !crate::ui::new_project::flat_dep_is_selectable(
                                &meta,
                                &app.new_project_state.dep_filter,
                                next,
                            )
                        {
                            next += 1;
                        }
                        if next < total {
                            app.new_project_state.dep_item_idx = next;
                        }
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if app.new_project_state.dep_item_idx > 0 {
                        let mut prev = app.new_project_state.dep_item_idx - 1;
                        // Skip group headers
                        while prev > 0
                            && !crate::ui::new_project::flat_dep_is_selectable(
                                &meta,
                                &app.new_project_state.dep_filter,
                                prev,
                            )
                        {
                            prev -= 1;
                        }
                        if crate::ui::new_project::flat_dep_is_selectable(
                            &meta,
                            &app.new_project_state.dep_filter,
                            prev,
                        ) {
                            app.new_project_state.dep_item_idx = prev;
                        }
                    }
                }
                KeyCode::Char(' ') => {
                    if let Some(id) = crate::ui::new_project::flat_dep_id_at(
                        &meta,
                        &app.new_project_state.dep_filter,
                        app.new_project_state.dep_item_idx,
                    ) {
                        if let Some(pos) = app
                            .new_project_state
                            .selected_deps
                            .iter()
                            .position(|d| d == &id)
                        {
                            app.new_project_state.selected_deps.remove(pos);
                        } else {
                            app.new_project_state.selected_deps.push(id);
                        }
                    }
                }
                KeyCode::Enter => {
                    app.new_project_state.step = WizardStep::Confirm;
                }
                _ => {}
            }
        }
        WizardStep::Confirm => {
            match key {
                KeyCode::Esc => {
                    // Go back to Dependencies
                    app.new_project_state.step = WizardStep::Dependencies;
                }
                KeyCode::Char(c) => {
                    app.new_project_state.output_dir.push(c);
                }
                KeyCode::Backspace => {
                    app.new_project_state.output_dir.pop();
                }
                KeyCode::Enter => {
                    // Trigger generation
                    let meta = match &app.new_project_state.metadata {
                        Some(m) => m,
                        None => return,
                    };
                    let ws = &app.new_project_state;

                    let params = crate::model::NewProjectParams {
                        boot_version: meta
                            .boot_versions
                            .get(ws.boot_version_idx)
                            .map(|v| v.id.clone())
                            .unwrap_or_default(),
                        language: meta
                            .languages
                            .get(ws.language_idx)
                            .map(|v| v.id.clone())
                            .unwrap_or_default(),
                        packaging: meta
                            .packagings
                            .get(ws.packaging_idx)
                            .map(|v| v.id.clone())
                            .unwrap_or_default(),
                        java_version: meta
                            .java_versions
                            .get(ws.java_version_idx)
                            .map(|v| v.id.clone())
                            .unwrap_or_default(),
                        project_type: meta
                            .project_types
                            .get(ws.project_type_idx)
                            .map(|v| v.id.clone())
                            .unwrap_or_default(),
                        group_id: ws.group_id.clone(),
                        artifact_id: ws.artifact_id.clone(),
                        version: "0.0.1-SNAPSHOT".into(),
                        name: ws.name.clone(),
                        description: ws.description.clone(),
                        package_name: ws.package_name.clone(),
                        dependencies: ws.selected_deps.clone(),
                        output_dir: ws.output_dir.clone(),
                    };

                    app.new_project_state.step = WizardStep::Generating;
                    app.new_project_state.gen_progress = "Generating project...".into();
                    app.new_project_state.gen_done = false;

                    let tx_clone = tx;
                    tokio::spawn(async move {
                        let result = App::generate_project(&params);
                        let _ = tx_clone
                            .send(AppEvent::GenerateResult(
                                result.map_err(|e| format!("{}", e)),
                            ))
                            .await;
                    });
                }
                _ => {}
            }
        }
        WizardStep::Generating => {
            match key {
                KeyCode::Esc | KeyCode::Enter => {
                    if app.new_project_state.gen_done {
                        // Done — go back to normal mode
                        app.mode = Mode::Normal;
                        app.new_project_state = app::NewProjectWizardState::default();
                    }
                    // Can't cancel while generating
                }
                _ => {}
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Port scanning: scan common Spring Boot ports on localhost
// ---------------------------------------------------------------------------

/// Known ports commonly used by Spring Boot applications.
const SCAN_PORTS: &[u16] = &[
    8080, 8081, 8082, 8083, 8084, 8085, 8086, 8087, 8088, 8089, 8090, 8443, 9090, 9091, 3000, 5000,
];

async fn scan_local_ports(tx: mpsc::Sender<AppEvent>, http_client: reqwest::Client) {
    let total = SCAN_PORTS.len();

    // Use a short-timeout client for scanning
    let scan_client = reqwest::Client::builder()
        .timeout(Duration::from_millis(800))
        .build()
        .unwrap_or(http_client);

    for (i, &port) in SCAN_PORTS.iter().enumerate() {
        let _ = tx
            .send(AppEvent::ScanProgress(format!(
                "Scanning port {}/{} (:{})...",
                i + 1,
                total,
                port
            )))
            .await;

        let url = format!("http://localhost:{}", port);
        let health_url = format!("{}/actuator/health", url);

        match scan_client.get(&health_url).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    let status = match resp.json::<serde_json::Value>().await {
                        Ok(body) => match body.get("status").and_then(|s| s.as_str()) {
                            Some("UP") => AppStatus::Up,
                            Some("DOWN") => AppStatus::Down,
                            _ => AppStatus::Unknown,
                        },
                        Err(_) => AppStatus::Unknown,
                    };
                    let _ = tx
                        .send(AppEvent::ScanFound {
                            url: url.clone(),
                            port,
                            status,
                        })
                        .await;
                }
                // Non-success status (e.g. 404) means it's not a Spring Boot actuator
            }
            Err(_) => {
                // Connection refused / timeout — port not open or not Spring Boot
            }
        }
    }

    let _ = tx.send(AppEvent::ScanDone).await;
}
