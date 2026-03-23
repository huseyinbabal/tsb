//! Standalone interactive flow for `tsb new` — step-by-step inline prompts.
//!
//! Uses raw crossterm for arrow-key selection + text input without a full
//! ratatui TUI.

use std::io::{self, Write};
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
    terminal::{self, ClearType},
};

use crate::app::App;
use crate::model::{InitializrDependencyGroup, InitializrOption, NewProjectParams};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run the interactive `tsb new` project generation flow.
pub async fn run() -> Result<()> {
    let mut stdout = io::stdout();

    // Banner
    print_banner(&mut stdout)?;

    // Load metadata from local cache
    println!();
    print_step(&mut stdout, "Loading metadata...")?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .context("failed to build HTTP client")?;

    let meta = App::fetch_initializr_metadata(&client).await?;
    print_success(&mut stdout, "Metadata loaded")?;
    println!();

    // -- Step 1: Select fields --
    // select_option returns (id, name) — id goes to the API, name is for display.
    let (boot_version_id, boot_version_name) = select_option(
        &mut stdout,
        "Spring Boot",
        &meta.boot_versions,
        &meta.boot_version_default,
    )?;
    let (language_id, language_name) = select_option(
        &mut stdout,
        "Language",
        &meta.languages,
        &meta.language_default,
    )?;
    let (packaging_id, packaging_name) = select_option(
        &mut stdout,
        "Packaging",
        &meta.packagings,
        &meta.packaging_default,
    )?;
    let (java_version_id, java_version_name) = select_option(
        &mut stdout,
        "Java Version",
        &meta.java_versions,
        &meta.java_version_default,
    )?;
    let (project_type_id, project_type_name) = select_option(
        &mut stdout,
        "Project Type",
        &meta.project_types,
        &meta.project_type_default,
    )?;

    // -- Step 2: Text fields --
    println!();
    let group_id = text_prompt(&mut stdout, "Group", &meta.group_id_default)?;
    let artifact_id = text_prompt(&mut stdout, "Artifact", &meta.artifact_id_default)?;
    let name = text_prompt(&mut stdout, "Name", &meta.name_default)?;
    let description = text_prompt(&mut stdout, "Description", &meta.description_default)?;
    let package_name = text_prompt(&mut stdout, "Package Name", &meta.package_name_default)?;

    // -- Step 3: Dependencies --
    println!();
    let selected_deps = select_dependencies(&mut stdout, &meta.dependency_groups)?;

    // -- Step 4: Output directory --
    println!();
    let output_dir = text_prompt(&mut stdout, "Output Directory", ".")?;

    // -- Summary (show display names, not raw API ids) --
    println!();
    print_summary(
        &mut stdout,
        &boot_version_name,
        &language_name,
        &packaging_name,
        &java_version_name,
        &project_type_name,
        &group_id,
        &artifact_id,
        &name,
        &description,
        &package_name,
        &selected_deps,
        &output_dir,
    )?;

    // -- Confirm --
    println!();
    if !confirm_prompt(&mut stdout, "Generate project?")? {
        println!("  Cancelled.");
        return Ok(());
    }

    // -- Generate project locally --
    println!();
    print_step(&mut stdout, "Generating project...")?;

    let params = NewProjectParams {
        boot_version: boot_version_id,
        language: language_id,
        packaging: packaging_id,
        java_version: java_version_id,
        project_type: project_type_id,
        group_id,
        artifact_id,
        version: "0.0.1-SNAPSHOT".into(),
        name,
        description,
        package_name,
        dependencies: selected_deps,
        output_dir,
    };

    let path = App::generate_project(&params)?;
    println!();
    print_success(&mut stdout, &format!("Project created at: {}", path))?;
    println!();

    Ok(())
}

// ---------------------------------------------------------------------------
// Banner
// ---------------------------------------------------------------------------

fn print_banner(out: &mut impl Write) -> Result<()> {
    let green = Color::Rgb {
        r: 80,
        g: 200,
        b: 50,
    };
    execute!(
        out,
        SetForegroundColor(green),
        Print("  ████████  ██████  ██████▄\n"),
        Print("     ██     ██      ██   ██\n"),
        Print("     ██     ██████  ██████▀\n"),
        Print("     ██         ██  ██   ██\n"),
        Print("     ██     ██████  ██████▀\n"),
        ResetColor,
        Print("\n"),
        SetForegroundColor(Color::Grey),
        Print("  Spring Boot Project Generator\n"),
        ResetColor,
    )?;
    Ok(())
}

fn print_step(out: &mut impl Write, msg: &str) -> Result<()> {
    execute!(
        out,
        SetForegroundColor(Color::Cyan),
        Print(format!("  ▸ {}", msg)),
        ResetColor,
        Print("\n"),
    )?;
    out.flush()?;
    Ok(())
}

fn print_success(out: &mut impl Write, msg: &str) -> Result<()> {
    execute!(
        out,
        SetForegroundColor(Color::Green),
        Print(format!("  ✓ {}", msg)),
        ResetColor,
        Print("\n"),
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Select prompt (single-select with arrow keys)
// ---------------------------------------------------------------------------

/// Returns `(id, name)` — the API id and the human-readable display name.
fn select_option(
    out: &mut impl Write,
    label: &str,
    options: &[InitializrOption],
    default: &str,
) -> Result<(String, String)> {
    if options.is_empty() {
        return Ok((default.to_string(), default.to_string()));
    }

    let mut selected = options.iter().position(|o| o.id == default).unwrap_or(0);

    terminal::enable_raw_mode()?;

    // Print label + initial value
    render_select_line(out, label, options, selected)?;

    loop {
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Left | KeyCode::Char('h') => {
                        selected = if selected == 0 {
                            options.len() - 1
                        } else {
                            selected - 1
                        };
                        render_select_line(out, label, options, selected)?;
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        selected = (selected + 1) % options.len();
                        render_select_line(out, label, options, selected)?;
                    }
                    KeyCode::Enter => break,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        terminal::disable_raw_mode()?;
                        anyhow::bail!("interrupted");
                    }
                    _ => {}
                }
            }
        }
    }

    terminal::disable_raw_mode()?;

    // Finalize line
    execute!(
        out,
        cursor::MoveToColumn(0),
        terminal::Clear(ClearType::CurrentLine),
        SetForegroundColor(Color::Green),
        Print("  ✓ "),
        ResetColor,
        SetAttribute(Attribute::Bold),
        Print(format!("{:<16}", label)),
        SetAttribute(Attribute::Reset),
        SetForegroundColor(Color::White),
        Print(&options[selected].name),
        ResetColor,
        Print("\n"),
    )?;
    out.flush()?;

    Ok((options[selected].id.clone(), options[selected].name.clone()))
}

fn render_select_line(
    out: &mut impl Write,
    label: &str,
    options: &[InitializrOption],
    selected: usize,
) -> Result<()> {
    execute!(
        out,
        cursor::MoveToColumn(0),
        terminal::Clear(ClearType::CurrentLine),
        SetForegroundColor(Color::Cyan),
        Print("  ▸ "),
        ResetColor,
        SetAttribute(Attribute::Bold),
        Print(format!("{:<16}", label)),
        SetAttribute(Attribute::Reset),
        SetForegroundColor(Color::DarkGrey),
        Print("◂ "),
        SetForegroundColor(Color::Rgb {
            r: 80,
            g: 200,
            b: 50
        }),
        SetAttribute(Attribute::Bold),
        Print(&options[selected].name),
        SetAttribute(Attribute::Reset),
        SetForegroundColor(Color::DarkGrey),
        Print(" ▸"),
        ResetColor,
    )?;
    out.flush()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Text prompt
// ---------------------------------------------------------------------------

fn text_prompt(out: &mut impl Write, label: &str, default: &str) -> Result<String> {
    let mut value = default.to_string();

    terminal::enable_raw_mode()?;
    execute!(out, cursor::Hide)?;
    render_text_line(out, label, &value)?;

    loop {
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Enter => break,
                    KeyCode::Backspace => {
                        value.pop();
                        render_text_line(out, label, &value)?;
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        execute!(out, cursor::Show)?;
                        terminal::disable_raw_mode()?;
                        anyhow::bail!("interrupted");
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        value.clear();
                        render_text_line(out, label, &value)?;
                    }
                    KeyCode::Char(c) => {
                        value.push(c);
                        render_text_line(out, label, &value)?;
                    }
                    _ => {}
                }
            }
        }
    }

    execute!(out, cursor::Show)?;
    terminal::disable_raw_mode()?;

    // Finalize line
    execute!(
        out,
        cursor::MoveToColumn(0),
        terminal::Clear(ClearType::CurrentLine),
        SetForegroundColor(Color::Green),
        Print("  ✓ "),
        ResetColor,
        SetAttribute(Attribute::Bold),
        Print(format!("{:<16}", label)),
        SetAttribute(Attribute::Reset),
        SetForegroundColor(Color::White),
        Print(&value),
        ResetColor,
        Print("\n"),
    )?;
    out.flush()?;

    Ok(value)
}

fn render_text_line(out: &mut impl Write, label: &str, value: &str) -> Result<()> {
    execute!(
        out,
        cursor::MoveToColumn(0),
        terminal::Clear(ClearType::CurrentLine),
        SetForegroundColor(Color::Cyan),
        Print("  ▸ "),
        ResetColor,
        SetAttribute(Attribute::Bold),
        Print(format!("{:<16}", label)),
        SetAttribute(Attribute::Reset),
        SetForegroundColor(Color::White),
        Print(value),
        SetForegroundColor(Color::DarkGrey),
        Print("█"),
        ResetColor,
    )?;
    out.flush()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Multi-select dependencies (grouped, with search)
// ---------------------------------------------------------------------------

fn select_dependencies(
    out: &mut impl Write,
    groups: &[InitializrDependencyGroup],
) -> Result<Vec<String>> {
    if groups.is_empty() {
        return Ok(Vec::new());
    }

    // Flatten all deps with group info for display
    let mut all_deps: Vec<(&str, &str, &str, &str)> = Vec::new(); // (group, id, name, desc)
    for g in groups {
        for d in &g.values {
            all_deps.push((&g.name, &d.id, &d.name, &d.description));
        }
    }

    let mut selected: Vec<bool> = vec![false; all_deps.len()];
    let mut cursor_pos: usize = 0;
    let mut filter = String::new();
    let mut filter_active = false;

    terminal::enable_raw_mode()?;

    // Hide cursor
    execute!(out, cursor::Hide)?;

    // How many lines we can show
    let page_size: usize = 12;

    loop {
        // Build filtered indices
        let filtered_indices: Vec<usize> = if filter.is_empty() {
            (0..all_deps.len()).collect()
        } else {
            let f = filter.to_lowercase();
            all_deps
                .iter()
                .enumerate()
                .filter(|(_, (group, id, name, desc))| {
                    name.to_lowercase().contains(&f)
                        || id.to_lowercase().contains(&f)
                        || desc.to_lowercase().contains(&f)
                        || group.to_lowercase().contains(&f)
                })
                .map(|(i, _)| i)
                .collect()
        };

        // Clamp cursor
        if !filtered_indices.is_empty() {
            cursor_pos = cursor_pos.min(filtered_indices.len() - 1);
        }

        // Determine visible window
        let start = if filtered_indices.is_empty() {
            0
        } else {
            cursor_pos
                .saturating_sub(page_size / 2)
                .min(filtered_indices.len().saturating_sub(page_size))
        };
        let end = (start + page_size).min(filtered_indices.len());

        // Count selected
        let selected_count = selected.iter().filter(|&&s| s).count();

        // Render header line
        render_dep_header(out, &filter, filter_active, selected_count)?;

        // Render visible deps
        #[allow(clippy::needless_range_loop)]
        for vi in start..end {
            let di = filtered_indices[vi];
            let (group, _id, name, desc) = all_deps[di];
            let is_current = vi == cursor_pos;
            let is_selected = selected[di];
            render_dep_line(out, group, name, desc, is_current, is_selected)?;
        }

        // Padding
        for _ in end..(start + page_size) {
            execute!(out, Print("\r\n"), terminal::Clear(ClearType::CurrentLine),)?;
        }

        // Hint line
        let hint = if filter_active {
            "  Type to filter │ Enter to confirm filter │ Esc to clear"
        } else {
            "  ↑/↓ navigate │ Space toggle │ / filter │ Enter done"
        };
        execute!(
            out,
            Print("\r\n"),
            terminal::Clear(ClearType::CurrentLine),
            SetForegroundColor(Color::DarkGrey),
            Print(hint),
            ResetColor,
        )?;
        out.flush()?;

        // Wait for input
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    // Move below rendered area, restore cursor, exit
                    execute!(out, cursor::Show)?;
                    terminal::disable_raw_mode()?;
                    anyhow::bail!("interrupted");
                }

                if filter_active {
                    match key.code {
                        KeyCode::Esc => {
                            filter.clear();
                            filter_active = false;
                            cursor_pos = 0;
                        }
                        KeyCode::Enter => {
                            filter_active = false;
                        }
                        KeyCode::Backspace => {
                            filter.pop();
                            cursor_pos = 0;
                        }
                        KeyCode::Char(c) => {
                            filter.push(c);
                            cursor_pos = 0;
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('j') | KeyCode::Down => {
                            if !filtered_indices.is_empty()
                                && cursor_pos + 1 < filtered_indices.len()
                            {
                                cursor_pos += 1;
                            }
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            cursor_pos = cursor_pos.saturating_sub(1);
                        }
                        KeyCode::Char(' ') => {
                            if !filtered_indices.is_empty() {
                                let di = filtered_indices[cursor_pos];
                                selected[di] = !selected[di];
                            }
                        }
                        KeyCode::Char('/') => {
                            filter_active = true;
                            filter.clear();
                        }
                        KeyCode::Enter => break,
                        KeyCode::Esc => break,
                        _ => {}
                    }
                }
            }
        }

        // Move cursor back up to redraw (header + page_size + hint = page_size + 2)
        let lines_drawn = (end - start) + (page_size - (end - start)) + 2;
        execute!(out, cursor::MoveUp(lines_drawn as u16), Print("\r"))?;
    }

    // Clear rendered area
    let total_lines = page_size + 2;
    for _ in 0..total_lines {
        execute!(out, terminal::Clear(ClearType::CurrentLine), Print("\r\n"),)?;
    }
    // Move back up
    execute!(out, cursor::MoveUp(total_lines as u16), Print("\r"))?;

    execute!(out, cursor::Show)?;
    terminal::disable_raw_mode()?;

    // Collect results
    let result: Vec<String> = all_deps
        .iter()
        .enumerate()
        .filter(|(i, _)| selected[*i])
        .map(|(_, (_, id, _, _))| id.to_string())
        .collect();

    // Print summary
    if result.is_empty() {
        execute!(
            out,
            SetForegroundColor(Color::Green),
            Print("  ✓ "),
            ResetColor,
            SetAttribute(Attribute::Bold),
            Print("Dependencies    "),
            SetAttribute(Attribute::Reset),
            SetForegroundColor(Color::DarkGrey),
            Print("(none)"),
            ResetColor,
            Print("\n"),
        )?;
    } else {
        let names: Vec<String> = all_deps
            .iter()
            .enumerate()
            .filter(|(i, _)| selected[*i])
            .map(|(_, (_, _, name, _))| name.to_string())
            .collect();
        let display = if names.len() <= 5 {
            names.join(", ")
        } else {
            format!("{}, +{} more", names[..5].join(", "), names.len() - 5)
        };
        execute!(
            out,
            SetForegroundColor(Color::Green),
            Print("  ✓ "),
            ResetColor,
            SetAttribute(Attribute::Bold),
            Print("Dependencies    "),
            SetAttribute(Attribute::Reset),
            SetForegroundColor(Color::Cyan),
            Print(&display),
            ResetColor,
            Print("\n"),
        )?;
    }
    out.flush()?;

    Ok(result)
}

fn render_dep_header(
    out: &mut impl Write,
    filter: &str,
    filter_active: bool,
    selected_count: usize,
) -> Result<()> {
    execute!(out, Print("\r\n"), terminal::Clear(ClearType::CurrentLine))?;

    if filter_active {
        execute!(
            out,
            SetForegroundColor(Color::Yellow),
            SetAttribute(Attribute::Bold),
            Print("  Filter: "),
            SetAttribute(Attribute::Reset),
            SetForegroundColor(Color::White),
            Print(filter),
            SetForegroundColor(Color::DarkGrey),
            Print("█"),
            ResetColor,
        )?;
    } else if !filter.is_empty() {
        execute!(
            out,
            SetForegroundColor(Color::Yellow),
            Print(format!("  Filter: {} ", filter)),
            ResetColor,
            SetForegroundColor(Color::DarkGrey),
            Print(format!("│ {} selected", selected_count)),
            ResetColor,
        )?;
    } else {
        execute!(
            out,
            SetForegroundColor(Color::Cyan),
            SetAttribute(Attribute::Bold),
            Print("  Dependencies"),
            SetAttribute(Attribute::Reset),
            SetForegroundColor(Color::DarkGrey),
            Print(format!("  ({} selected)", selected_count)),
            ResetColor,
        )?;
    }

    Ok(())
}

fn render_dep_line(
    out: &mut impl Write,
    group: &str,
    name: &str,
    desc: &str,
    is_current: bool,
    is_selected: bool,
) -> Result<()> {
    execute!(out, Print("\r\n"), terminal::Clear(ClearType::CurrentLine))?;

    let pointer = if is_current { "▸" } else { " " };
    let checkbox = if is_selected { "✓" } else { " " };

    let pointer_color = if is_current {
        Color::Rgb {
            r: 80,
            g: 200,
            b: 50,
        }
    } else {
        Color::DarkGrey
    };
    let check_color = if is_selected {
        Color::Rgb {
            r: 80,
            g: 200,
            b: 50,
        }
    } else {
        Color::DarkGrey
    };
    let name_color = if is_current {
        Color::White
    } else {
        Color::Grey
    };

    // Truncate description
    let short_desc: String = if desc.len() > 45 {
        format!("{}…", &desc[..44])
    } else {
        desc.to_string()
    };

    // Truncate group to 14 chars
    let short_group: String = if group.len() > 14 {
        format!("{}…", &group[..13])
    } else {
        group.to_string()
    };

    execute!(
        out,
        Print("  "),
        SetForegroundColor(pointer_color),
        Print(pointer),
        Print(" "),
        SetForegroundColor(check_color),
        Print(format!("[{}] ", checkbox)),
        SetForegroundColor(name_color),
        SetAttribute(if is_current {
            Attribute::Bold
        } else {
            Attribute::Reset
        }),
        Print(format!("{:<30}", name)),
        SetAttribute(Attribute::Reset),
        SetForegroundColor(Color::DarkGrey),
        Print(format!("{:<16}", short_group)),
        SetForegroundColor(Color::DarkGrey),
        Print(short_desc),
        ResetColor,
    )?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Confirm (y/n)
// ---------------------------------------------------------------------------

fn confirm_prompt(out: &mut impl Write, message: &str) -> Result<bool> {
    terminal::enable_raw_mode()?;

    execute!(
        out,
        SetForegroundColor(Color::Cyan),
        Print(format!("  ▸ {} ", message)),
        SetForegroundColor(Color::DarkGrey),
        Print("[Y/n] "),
        ResetColor,
    )?;
    out.flush()?;

    let result = loop {
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => break true,
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => break false,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        terminal::disable_raw_mode()?;
                        anyhow::bail!("interrupted");
                    }
                    _ => {}
                }
            }
        }
    };

    terminal::disable_raw_mode()?;
    execute!(out, Print("\n"))?;
    out.flush()?;

    Ok(result)
}

// ---------------------------------------------------------------------------
// Summary
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn print_summary(
    out: &mut impl Write,
    boot_version: &str,
    language: &str,
    packaging: &str,
    java_version: &str,
    project_type: &str,
    group_id: &str,
    artifact_id: &str,
    name: &str,
    description: &str,
    package_name: &str,
    selected_deps: &[String],
    output_dir: &str,
) -> Result<()> {
    execute!(
        out,
        SetForegroundColor(Color::Cyan),
        SetAttribute(Attribute::Bold),
        Print("  ─── Summary ───────────────────────────────────\n"),
        SetAttribute(Attribute::Reset),
        ResetColor,
    )?;

    let fields: Vec<(&str, &str)> = vec![
        ("Boot Version", boot_version),
        ("Language", language),
        ("Packaging", packaging),
        ("Java Version", java_version),
        ("Project Type", project_type),
        ("Group", group_id),
        ("Artifact", artifact_id),
        ("Name", name),
        ("Description", description),
        ("Package", package_name),
        ("Output Dir", output_dir),
    ];

    for (label, value) in &fields {
        execute!(
            out,
            SetForegroundColor(Color::Grey),
            Print(format!("  {:<16}", label)),
            ResetColor,
            SetForegroundColor(Color::White),
            Print(value),
            ResetColor,
            Print("\n"),
        )?;
    }

    // Dependencies
    let dep_display = if selected_deps.is_empty() {
        "(none)".to_string()
    } else {
        selected_deps.join(", ")
    };
    execute!(
        out,
        SetForegroundColor(Color::Grey),
        Print("  Dependencies  "),
        ResetColor,
        SetForegroundColor(Color::Cyan),
        Print(&dep_display),
        ResetColor,
        Print("\n"),
    )?;

    execute!(
        out,
        SetForegroundColor(Color::Cyan),
        Print("  ────────────────────────────────────────────────\n"),
        ResetColor,
    )?;
    out.flush()?;

    Ok(())
}
