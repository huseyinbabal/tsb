use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::event::KeyCode;
use serde_json::Value;

use crate::config::TsbConfig;
use crate::model::*;
use crate::ui::splash::SplashState;

// ---------------------------------------------------------------------------
// Mode
// ---------------------------------------------------------------------------

/// Active UI mode — drives which panel/overlay is shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Splash,
    Normal,
    Describe,
    Confirm,
    Resources,
    ServerDialog,
    EditLogger,
    NewProject,
    ThreadViz,
    ErrorModal,
}

// ---------------------------------------------------------------------------
// ResourceItem (command-palette entry)
// ---------------------------------------------------------------------------

/// A single entry in the command palette / resource list.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceItem {
    pub name: String,
    pub command: String,
    pub description: String,
}

// ---------------------------------------------------------------------------
// ServerDialogState
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// EditLoggerState
// ---------------------------------------------------------------------------

/// Available log levels for editing.
pub const LOG_LEVELS: &[&str] = &["TRACE", "DEBUG", "INFO", "WARN", "ERROR", "OFF"];

/// State for the logger level editor overlay.
#[derive(Default)]
pub struct EditLoggerState {
    pub logger_name: String,
    pub current_level: String,
    pub selected_level_index: usize,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// NewProjectWizard state
// ---------------------------------------------------------------------------

/// Which step of the new-project wizard the user is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WizardStep {
    /// Editing project metadata fields (group, artifact, name, etc.) and
    /// selecting boot version / language / packaging / java version.
    ProjectInfo,
    /// Selecting dependencies from the categorised list.
    Dependencies,
    /// Review summary and choose output directory, then generate.
    Confirm,
    /// Generating (downloading + extracting).
    Generating,
}

/// State for the New Project wizard.
pub struct NewProjectWizardState {
    pub step: WizardStep,
    pub metadata: Option<crate::model::InitializrMetadata>,
    pub loading_metadata: bool,
    pub error: Option<String>,

    // -- Project info fields (step 1) --
    /// 0=bootVersion, 1=language, 2=packaging, 3=javaVersion, 4=projectType,
    /// 5=groupId, 6=artifactId, 7=name, 8=description, 9=packageName
    pub active_field: usize,
    pub boot_version_idx: usize,
    pub language_idx: usize,
    pub packaging_idx: usize,
    pub java_version_idx: usize,
    pub project_type_idx: usize,
    pub group_id: String,
    pub artifact_id: String,
    pub name: String,
    pub description: String,
    pub package_name: String,

    // -- Dependencies (step 2) --
    /// Index of the currently browsed dependency group.
    pub dep_group_idx: usize,
    /// Index within the current group's dependencies.
    pub dep_item_idx: usize,
    /// IDs of selected dependencies.
    pub selected_deps: Vec<String>,
    /// Filter text for dependency search.
    pub dep_filter: String,
    pub dep_filter_active: bool,

    // -- Confirm (step 3) --
    pub output_dir: String,

    // -- Generating --
    pub gen_progress: String,
    pub gen_done: bool,
    pub gen_result_path: Option<String>,
}

impl Default for NewProjectWizardState {
    fn default() -> Self {
        Self {
            step: WizardStep::ProjectInfo,
            metadata: None,
            loading_metadata: false,
            error: None,
            active_field: 0,
            boot_version_idx: 0,
            language_idx: 0,
            packaging_idx: 0,
            java_version_idx: 0,
            project_type_idx: 0,
            group_id: "com.example".into(),
            artifact_id: "demo".into(),
            name: "demo".into(),
            description: "Demo project for Spring Boot".into(),
            package_name: "com.example.demo".into(),
            dep_group_idx: 0,
            dep_item_idx: 1, // skip first group header
            selected_deps: Vec::new(),
            dep_filter: String::new(),
            dep_filter_active: false,
            output_dir: ".".into(),
            gen_progress: String::new(),
            gen_done: false,
            gen_result_path: None,
        }
    }
}

impl NewProjectWizardState {
    /// Populate defaults from loaded metadata.
    /// Only overwrite wizard defaults when the metadata provides a non-empty value.
    pub fn apply_metadata_defaults(&mut self, meta: &crate::model::InitializrMetadata) {
        if !meta.group_id_default.is_empty() {
            self.group_id = meta.group_id_default.clone();
        }
        if !meta.artifact_id_default.is_empty() {
            self.artifact_id = meta.artifact_id_default.clone();
        }
        if !meta.name_default.is_empty() {
            self.name = meta.name_default.clone();
        }
        if !meta.description_default.is_empty() {
            self.description = meta.description_default.clone();
        }
        if !meta.package_name_default.is_empty() {
            self.package_name = meta.package_name_default.clone();
        }

        self.boot_version_idx = meta
            .boot_versions
            .iter()
            .position(|v| v.id == meta.boot_version_default)
            .unwrap_or(0);
        self.language_idx = meta
            .languages
            .iter()
            .position(|v| v.id == meta.language_default)
            .unwrap_or(0);
        self.packaging_idx = meta
            .packagings
            .iter()
            .position(|v| v.id == meta.packaging_default)
            .unwrap_or(0);
        self.java_version_idx = meta
            .java_versions
            .iter()
            .position(|v| v.id == meta.java_version_default)
            .unwrap_or(0);
        self.project_type_idx = meta
            .project_types
            .iter()
            .position(|v| v.id == meta.project_type_default)
            .unwrap_or(0);
    }
}

/// Which phase of the "add server" dialog we're in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerDialogPhase {
    /// Choosing between manual entry and local scan.
    ChooseMethod,
    /// Manual form: name + url fields.
    ManualEntry,
    /// Scanning local ports.
    Scanning,
    /// Displaying scan results for the user to select from.
    ScanResults,
}

/// A Spring Boot app discovered during local port scan.
#[derive(Debug, Clone)]
pub struct DiscoveredApp {
    pub url: String,
    pub port: u16,
    pub status: AppStatus,
}

/// State for the "add/edit server" dialog.
pub struct ServerDialogState {
    pub phase: ServerDialogPhase,
    /// Which option is highlighted in the ChooseMethod phase (0=Manual, 1=Scan).
    pub method_selected: usize,

    // -- Manual entry fields --
    pub name: String,
    pub url: String,
    pub active_field: usize,
    pub error: Option<String>,

    // -- Scan state --
    pub scan_progress: String,
    pub scan_done: bool,
    pub discovered_apps: Vec<DiscoveredApp>,
    pub scan_selected_index: usize,
}

impl Default for ServerDialogState {
    fn default() -> Self {
        Self {
            phase: ServerDialogPhase::ChooseMethod,
            method_selected: 0,

            name: String::new(),
            url: String::new(),
            active_field: 0,
            error: None,

            scan_progress: String::new(),
            scan_done: false,
            discovered_apps: Vec::new(),
            scan_selected_index: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

/// Monolithic application state (mirrors the tredis pattern).
pub struct App {
    // -- mode / lifecycle ---------------------------------------------------
    pub mode: Mode,
    pub should_quit: bool,

    // -- active resource ----------------------------------------------------
    pub active_resource: String,

    // -- data collections ---------------------------------------------------
    pub apps: Vec<SpringApp>,
    pub endpoints: Vec<Endpoint>,
    pub beans: Vec<Bean>,
    pub loggers: Vec<Logger>,
    pub mappings: Vec<Mapping>,
    pub env_props: Vec<EnvProperty>,
    #[allow(dead_code)]
    pub server_info: Option<ServerInfo>,
    pub dashboard: crate::model::DashboardData,

    // -- selection indices ---------------------------------------------------
    pub selected_app_index: usize,
    pub selected_endpoint_index: usize,
    pub selected_bean_index: usize,
    pub selected_logger_index: usize,
    pub selected_mapping_index: usize,
    pub selected_env_index: usize,

    // -- describe panel -----------------------------------------------------
    pub describe_scroll: u16,
    pub describe_content: String,
    pub describe_title: String,

    // -- filter bar ----------------------------------------------------------
    pub filter_text: String,
    pub filter_active: bool,

    // -- command palette (resources) -----------------------------------------
    pub resources: Vec<ResourceItem>,
    pub command_text: String,
    pub command_suggestions: Vec<usize>,
    pub command_suggestion_selected: usize,

    // -- splash & dialogs ---------------------------------------------------
    pub splash_state: SplashState,
    pub server_dialog_state: ServerDialogState,
    pub edit_logger_state: EditLoggerState,
    pub new_project_state: NewProjectWizardState,

    // -- config -------------------------------------------------------------
    pub config: TsbConfig,

    // -- transient UI state -------------------------------------------------
    pub error_prev_mode: Option<Mode>,
    pub modal_title: String,
    pub modal_msg: String,
    #[allow(dead_code)]
    pub width: u16,
    #[allow(dead_code)]
    pub height: u16,
    pub spinner_frame: usize,

    // -- vim navigation -------------------------------------------------------
    pub last_key_press: Option<(KeyCode, Instant)>,

    // -- saved dumps -----------------------------------------------------------
    pub saved_thread_dumps: Vec<crate::model::SavedDump>,
    pub saved_heap_dumps: Vec<crate::model::SavedDump>,
    pub selected_thread_dump_index: usize,
    pub selected_heap_dump_index: usize,

    // -- thread visualization -------------------------------------------------
    pub parsed_threads: Vec<crate::model::ThreadInfo>,
    pub thread_viz_scroll: usize,
    pub thread_viz_title: String,

    // -- HTTP ----------------------------------------------------------------
    pub http_client: reqwest::Client,
}

const EMBEDDED_METADATA: &str = include_str!("../resources/initializr-metadata.json");
const EMBEDDED_DEPENDENCIES: &str = include_str!("../resources/dependencies.json");

const GITHUB_METADATA_URL: &str =
    "https://raw.githubusercontent.com/huseyinbabal/tsb/main/resources/initializr-metadata.json";
const GITHUB_DEPENDENCIES_URL: &str =
    "https://raw.githubusercontent.com/huseyinbabal/tsb/main/resources/dependencies.json";
impl App {
    // -----------------------------------------------------------------------
    // Constructor
    // -----------------------------------------------------------------------

    /// Create a new `App`, loading configuration from disk.
    pub fn new() -> Result<Self> {
        let config = TsbConfig::load().unwrap_or_default();

        let resources = vec![
            ResourceItem {
                name: "Apps".into(),
                command: ":apps".into(),
                description: "List connected Spring Boot applications".into(),
            },
            ResourceItem {
                name: "Dashboard".into(),
                command: ":dashboard".into(),
                description: "Application health, metrics and JVM overview".into(),
            },
            ResourceItem {
                name: "Endpoints".into(),
                command: ":endpoints".into(),
                description: "Browse actuator endpoints".into(),
            },
            ResourceItem {
                name: "Beans".into(),
                command: ":beans".into(),
                description: "View registered Spring beans".into(),
            },
            ResourceItem {
                name: "Loggers".into(),
                command: ":loggers".into(),
                description: "View and manage logger levels".into(),
            },
            ResourceItem {
                name: "Mappings".into(),
                command: ":mappings".into(),
                description: "List HTTP request mappings".into(),
            },
            ResourceItem {
                name: "Env".into(),
                command: ":env".into(),
                description: "Inspect environment properties".into(),
            },
            ResourceItem {
                name: "ThreadDumps".into(),
                command: ":threaddump".into(),
                description: "List saved thread dumps".into(),
            },
            ResourceItem {
                name: "HeapDumps".into(),
                command: ":heapdump".into(),
                description: "List saved heap dumps".into(),
            },
            ResourceItem {
                name: "New Project".into(),
                command: ":new".into(),
                description: "Generate a new Spring Boot project via Initializr".into(),
            },
        ];

        // Build initial apps list from saved config entries.
        let apps: Vec<SpringApp> = config
            .apps
            .iter()
            .map(|saved| SpringApp {
                name: saved.name.clone(),
                url: saved.url.clone(),
                status: AppStatus::Unknown,
            })
            .collect();

        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .context("failed to build HTTP client")?;

        Ok(Self {
            mode: Mode::Splash,
            should_quit: false,

            active_resource: "apps".into(),

            apps,
            endpoints: Vec::new(),
            beans: Vec::new(),
            loggers: Vec::new(),
            mappings: Vec::new(),
            env_props: Vec::new(),
            server_info: None,
            dashboard: crate::model::DashboardData::default(),

            selected_app_index: 0,
            selected_endpoint_index: 0,
            selected_bean_index: 0,
            selected_logger_index: 0,
            selected_mapping_index: 0,
            selected_env_index: 0,

            describe_scroll: 0,
            describe_content: String::new(),
            describe_title: String::new(),

            filter_text: String::new(),
            filter_active: false,

            command_suggestions: (0..resources.len()).collect(),
            resources,
            command_text: String::new(),
            command_suggestion_selected: 0,

            splash_state: SplashState::default(),
            server_dialog_state: ServerDialogState::default(),
            edit_logger_state: EditLoggerState::default(),
            new_project_state: NewProjectWizardState::default(),

            config,

            error_prev_mode: None,
            modal_title: String::new(),
            modal_msg: String::new(),
            width: 0,
            height: 0,
            spinner_frame: 0,

            last_key_press: None,

            saved_thread_dumps: Vec::new(),
            saved_heap_dumps: Vec::new(),
            selected_thread_dump_index: 0,
            selected_heap_dump_index: 0,

            parsed_threads: Vec::new(),
            thread_viz_scroll: 0,
            thread_viz_title: String::new(),

            http_client,
        })
    }

    // -----------------------------------------------------------------------
    // Navigation helpers
    // -----------------------------------------------------------------------

    /// Returns the indices of items matching the current filter for the active
    /// resource. When filter_text is empty, returns all indices.
    fn filtered_indices(&self) -> Vec<usize> {
        let filter = self.filter_text.to_lowercase();
        match self.active_resource.as_str() {
            "apps" => self
                .apps
                .iter()
                .enumerate()
                .filter(|(_, a)| {
                    filter.is_empty()
                        || a.name.to_lowercase().contains(&filter)
                        || a.url.to_lowercase().contains(&filter)
                        || a.status.to_string().to_lowercase().contains(&filter)
                })
                .map(|(i, _)| i)
                .collect(),
            "endpoints" => self
                .endpoints
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    filter.is_empty()
                        || e.name.to_lowercase().contains(&filter)
                        || e.url.to_lowercase().contains(&filter)
                })
                .map(|(i, _)| i)
                .collect(),
            "beans" => self
                .beans
                .iter()
                .enumerate()
                .filter(|(_, b)| {
                    filter.is_empty()
                        || b.name.to_lowercase().contains(&filter)
                        || b.scope.to_lowercase().contains(&filter)
                        || b.type_name.to_lowercase().contains(&filter)
                })
                .map(|(i, _)| i)
                .collect(),
            "loggers" => self
                .loggers
                .iter()
                .enumerate()
                .filter(|(_, l)| {
                    filter.is_empty()
                        || l.name.to_lowercase().contains(&filter)
                        || l.effective_level.to_lowercase().contains(&filter)
                        || l.configured_level
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&filter)
                })
                .map(|(i, _)| i)
                .collect(),
            "mappings" => self
                .mappings
                .iter()
                .enumerate()
                .filter(|(_, m)| {
                    filter.is_empty()
                        || m.pattern.to_lowercase().contains(&filter)
                        || m.handler.to_lowercase().contains(&filter)
                })
                .map(|(i, _)| i)
                .collect(),
            "env" => self
                .env_props
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    filter.is_empty()
                        || e.name.to_lowercase().contains(&filter)
                        || e.value.to_lowercase().contains(&filter)
                        || e.source.to_lowercase().contains(&filter)
                })
                .map(|(i, _)| i)
                .collect(),
            "threaddump" => self
                .saved_thread_dumps
                .iter()
                .enumerate()
                .filter(|(_, d)| {
                    filter.is_empty()
                        || d.path.to_lowercase().contains(&filter)
                        || d.timestamp.to_lowercase().contains(&filter)
                        || d.app_name.to_lowercase().contains(&filter)
                })
                .map(|(i, _)| i)
                .collect(),
            "heapdump" => self
                .saved_heap_dumps
                .iter()
                .enumerate()
                .filter(|(_, d)| {
                    filter.is_empty()
                        || d.path.to_lowercase().contains(&filter)
                        || d.timestamp.to_lowercase().contains(&filter)
                        || d.app_name.to_lowercase().contains(&filter)
                })
                .map(|(i, _)| i)
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Returns the current selection index for the active resource.
    fn active_index(&self) -> usize {
        match self.active_resource.as_str() {
            "apps" => self.selected_app_index,
            "endpoints" => self.selected_endpoint_index,
            "beans" => self.selected_bean_index,
            "loggers" => self.selected_logger_index,
            "mappings" => self.selected_mapping_index,
            "env" => self.selected_env_index,
            "threaddump" => self.selected_thread_dump_index,
            "heapdump" => self.selected_heap_dump_index,
            _ => 0,
        }
    }

    /// Returns a mutable reference to the selection index for the active
    /// resource.
    fn active_index_mut(&mut self) -> &mut usize {
        match self.active_resource.as_str() {
            "apps" => &mut self.selected_app_index,
            "endpoints" => &mut self.selected_endpoint_index,
            "beans" => &mut self.selected_bean_index,
            "loggers" => &mut self.selected_logger_index,
            "mappings" => &mut self.selected_mapping_index,
            "env" => &mut self.selected_env_index,
            "threaddump" => &mut self.selected_thread_dump_index,
            "heapdump" => &mut self.selected_heap_dump_index,
            _ => &mut self.selected_app_index,
        }
    }

    /// Move the cursor down by one in the (filtered) active list.
    pub fn next(&mut self) {
        let indices = self.filtered_indices();
        if indices.is_empty() {
            return;
        }
        let current = self.active_index();
        // Find the position of the current index in the filtered list
        let pos = indices.iter().position(|&i| i == current);
        let next_pos = match pos {
            Some(p) => (p + 1).min(indices.len() - 1),
            None => 0, // current index not in filtered list, jump to first
        };
        let idx = self.active_index_mut();
        *idx = indices[next_pos];
    }

    /// Move the cursor up by one in the (filtered) active list.
    pub fn previous(&mut self) {
        let indices = self.filtered_indices();
        if indices.is_empty() {
            return;
        }
        let current = self.active_index();
        let pos = indices.iter().position(|&i| i == current);
        let prev_pos = match pos {
            Some(p) => p.saturating_sub(1),
            None => 0,
        };
        let idx = self.active_index_mut();
        *idx = indices[prev_pos];
    }

    /// Jump to the first item in the (filtered) list.
    pub fn go_to_top(&mut self) {
        let indices = self.filtered_indices();
        if indices.is_empty() {
            return;
        }
        let idx = self.active_index_mut();
        *idx = indices[0];
    }

    /// Jump to the last item in the (filtered) list.
    pub fn go_to_bottom(&mut self) {
        let indices = self.filtered_indices();
        if indices.is_empty() {
            return;
        }
        let idx = self.active_index_mut();
        *idx = *indices.last().unwrap();
    }

    // -----------------------------------------------------------------------
    // Active-app helpers
    // -----------------------------------------------------------------------

    /// Returns `~/.config/tsb/dumps/`, creating it if needed.
    pub fn dumps_dir() -> Result<PathBuf> {
        let home = dirs::home_dir().context("could not determine home directory")?;
        let dir = home.join(".config").join("tsb").join("dumps");
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create dumps dir {}", dir.display()))?;
        Ok(dir)
    }

    /// Returns `~/.config/tsb/dumps/<app_name>/`, creating it if needed.
    /// Sanitizes the app name for use as a directory name.
    pub fn app_dumps_dir(app_name: &str) -> Result<PathBuf> {
        let sanitized: String = app_name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let name = if sanitized.is_empty() {
            "unknown".to_string()
        } else {
            sanitized
        };
        let dir = Self::dumps_dir()?.join(name);
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create app dumps dir {}", dir.display()))?;
        Ok(dir)
    }

    /// Scan the dumps directory and populate `saved_thread_dumps` and
    /// `saved_heap_dumps` from existing files on disk.
    /// Only loads dumps for the currently active app.
    pub fn scan_saved_dumps(&mut self) {
        self.saved_thread_dumps.clear();
        self.saved_heap_dumps.clear();

        let app_name = self.current_server_name();
        let dir = match Self::app_dumps_dir(&app_name) {
            Ok(d) => d,
            Err(_) => return,
        };

        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let filename = match path.file_name().and_then(|f| f.to_str()) {
                Some(f) => f.to_string(),
                None => continue,
            };
            let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let path_str = path.to_string_lossy().to_string();

            if filename.starts_with("threaddump_")
                && (filename.ends_with(".json") || filename.ends_with(".txt"))
            {
                let timestamp = filename
                    .strip_prefix("threaddump_")
                    .and_then(|s| s.strip_suffix(".json").or_else(|| s.strip_suffix(".txt")))
                    .unwrap_or("")
                    .to_string();
                self.saved_thread_dumps.push(crate::model::SavedDump {
                    app_url: self.active_app_url().unwrap_or_default(),
                    app_name: app_name.clone(),
                    path: path_str,
                    timestamp,
                    size_bytes,
                });
            } else if filename.starts_with("heapdump_") && filename.ends_with(".hprof") {
                let timestamp = filename
                    .strip_prefix("heapdump_")
                    .and_then(|s| s.strip_suffix(".hprof"))
                    .unwrap_or("")
                    .to_string();
                self.saved_heap_dumps.push(crate::model::SavedDump {
                    app_url: self.active_app_url().unwrap_or_default(),
                    app_name: app_name.clone(),
                    path: path_str,
                    timestamp,
                    size_bytes,
                });
            }
        }

        // Sort by timestamp descending (newest first)
        self.saved_thread_dumps
            .sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        self.saved_heap_dumps
            .sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    }

    /// URL of the currently selected / active Spring Boot app.
    pub fn active_app_url(&self) -> Option<String> {
        // Prefer the config-level active_app_url, otherwise fall back to the
        // app at the current selection index.
        if let Some(ref url) = self.config.active_app_url {
            return Some(url.clone());
        }
        self.apps
            .get(self.selected_app_index)
            .map(|a| a.url.clone())
    }

    /// Display name of the currently active app.
    pub fn current_server_name(&self) -> String {
        if let Some(ref active_url) = self.config.active_app_url {
            if let Some(app) = self.apps.iter().find(|a| &a.url == active_url) {
                return app.name.clone();
            }
        }
        self.apps
            .get(self.selected_app_index)
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "No server".into())
    }

    /// Show an error in a modal dialog. Saves the current mode so it can
    /// be restored when the user dismisses the dialog.
    pub fn show_error(&mut self, msg: impl Into<String>) {
        self.modal_title = "Error".into();
        self.modal_msg = msg.into();
        self.error_prev_mode = Some(self.mode.clone());
        self.mode = Mode::ErrorModal;
    }

    // -----------------------------------------------------------------------
    // Data fetching (async, reqwest)
    // -----------------------------------------------------------------------

    /// Check the health of a Spring Boot app via `/actuator/health`.
    #[allow(dead_code)]
    pub async fn check_health(&self, url: &str) -> Result<AppStatus> {
        let endpoint = format!("{}/actuator/health", url.trim_end_matches('/'));
        let resp = self
            .http_client
            .get(&endpoint)
            .send()
            .await
            .context("health check request failed")?;

        if !resp.status().is_success() {
            return Ok(AppStatus::Down);
        }

        let body: Value = resp
            .json()
            .await
            .context("failed to parse health response")?;
        match body.get("status").and_then(|s| s.as_str()) {
            Some("UP") => Ok(AppStatus::Up),
            Some("DOWN") => Ok(AppStatus::Down),
            _ => Ok(AppStatus::Unknown),
        }
    }

    /// Set the level of a logger via POST to `/actuator/loggers/{name}`.
    pub async fn set_logger_level(&mut self, logger_name: &str, level: &str) -> Result<()> {
        let base_url = self.active_app_url().context("no active app selected")?;
        let endpoint = format!(
            "{}/actuator/loggers/{}",
            base_url.trim_end_matches('/'),
            logger_name
        );
        let body = serde_json::json!({
            "configuredLevel": if level == "OFF" { Value::Null } else { Value::String(level.to_string()) }
        });

        let resp = self
            .http_client
            .post(&endpoint)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("failed to set logger level")?;

        if !resp.status().is_success() {
            anyhow::bail!("set logger level failed with status {}", resp.status());
        }

        // Update local state
        if let Some(logger) = self.loggers.iter_mut().find(|l| l.name == logger_name) {
            if level == "OFF" {
                logger.configured_level = None;
            } else {
                logger.configured_level = Some(level.to_string());
            }
            logger.effective_level = level.to_string();
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Dashboard
    // -----------------------------------------------------------------------

    /// Helper: fetch a single metric value from `/actuator/metrics/{name}`.
    async fn fetch_metric(&self, base_url: &str, name: &str) -> Option<f64> {
        let url = format!(
            "{}/actuator/metrics/{}",
            base_url.trim_end_matches('/'),
            name
        );
        let resp = self.http_client.get(&url).send().await.ok()?;
        let body: Value = resp.json().await.ok()?;
        body.get("measurements")
            .and_then(|m| m.as_array())
            .and_then(|arr| arr.first())
            .and_then(|m| m.get("value"))
            .and_then(|v| v.as_f64())
    }

    /// Fetch dashboard data from multiple actuator endpoints.
    pub async fn fetch_dashboard(&mut self) -> Result<()> {
        let base_url = self.active_app_url().context("no active app selected")?;
        let mut data = crate::model::DashboardData::default();

        // -- Health -----------------------------------------------------------
        if let Ok(resp) = self
            .http_client
            .get(format!(
                "{}/actuator/health",
                base_url.trim_end_matches('/')
            ))
            .send()
            .await
        {
            if let Ok(body) = resp.json::<Value>().await {
                data.app_status = body
                    .get("status")
                    .and_then(|s| s.as_str())
                    .unwrap_or("UNKNOWN")
                    .to_string();
                if let Some(components) = body.get("components").and_then(|c| c.as_object()) {
                    for (name, comp) in components {
                        let status = comp
                            .get("status")
                            .and_then(|s| s.as_str())
                            .unwrap_or("UNKNOWN")
                            .to_string();
                        let details = comp
                            .get("details")
                            .map(|d| {
                                d.as_object()
                                    .map(|obj| {
                                        obj.iter()
                                            .map(|(k, v)| {
                                                format!(
                                                    "{}={}",
                                                    k,
                                                    v.as_str()
                                                        .map(|s| s.to_string())
                                                        .unwrap_or_else(|| v.to_string())
                                                )
                                            })
                                            .collect::<Vec<_>>()
                                            .join(", ")
                                    })
                                    .unwrap_or_default()
                            })
                            .unwrap_or_default();
                        data.health_components.push(crate::model::HealthComponent {
                            name: name.clone(),
                            status,
                            details,
                        });
                    }
                }
            }
        }

        // -- JVM Memory -------------------------------------------------------
        if let Some(v) = self.fetch_metric(&base_url, "jvm.memory.used").await {
            // This is total, we'll also try heap specifically
            data.nonheap_used_mb = v / 1_048_576.0;
        }
        if let Some(v) = self.fetch_metric(&base_url, "jvm.memory.used").await {
            // Try to get heap-specific values via tags
            let heap_url = format!(
                "{}/actuator/metrics/jvm.memory.used?tag=area:heap",
                base_url.trim_end_matches('/')
            );
            if let Ok(resp) = self.http_client.get(&heap_url).send().await {
                if let Ok(body) = resp.json::<Value>().await {
                    if let Some(val) = body
                        .get("measurements")
                        .and_then(|m| m.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|m| m.get("value"))
                        .and_then(|v| v.as_f64())
                    {
                        data.heap_used_mb = val / 1_048_576.0;
                        data.nonheap_used_mb = (v / 1_048_576.0) - data.heap_used_mb;
                    }
                }
            }
        }
        if self
            .fetch_metric(&base_url, "jvm.memory.max")
            .await
            .is_some()
        {
            // Try heap-specific max
            let heap_url = format!(
                "{}/actuator/metrics/jvm.memory.max?tag=area:heap",
                base_url.trim_end_matches('/')
            );
            if let Ok(resp) = self.http_client.get(&heap_url).send().await {
                if let Ok(body) = resp.json::<Value>().await {
                    if let Some(val) = body
                        .get("measurements")
                        .and_then(|m| m.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|m| m.get("value"))
                        .and_then(|v| v.as_f64())
                    {
                        data.heap_max_mb = val / 1_048_576.0;
                    }
                }
            }
        }

        // -- Threads ----------------------------------------------------------
        if let Some(v) = self.fetch_metric(&base_url, "jvm.threads.live").await {
            data.threads_live = v as u64;
        }
        if let Some(v) = self.fetch_metric(&base_url, "jvm.threads.peak").await {
            data.threads_peak = v as u64;
        }
        if let Some(v) = self.fetch_metric(&base_url, "jvm.threads.daemon").await {
            data.threads_daemon = v as u64;
        }

        // -- CPU --------------------------------------------------------------
        if let Some(v) = self.fetch_metric(&base_url, "system.cpu.usage").await {
            data.cpu_system = v * 100.0;
        }
        if let Some(v) = self.fetch_metric(&base_url, "process.cpu.usage").await {
            data.cpu_process = v * 100.0;
        }

        // -- GC ---------------------------------------------------------------
        if let Some(v) = self.fetch_metric(&base_url, "jvm.gc.pause").await {
            // COUNT measurement
            let gc_url = format!(
                "{}/actuator/metrics/jvm.gc.pause",
                base_url.trim_end_matches('/')
            );
            if let Ok(resp) = self.http_client.get(&gc_url).send().await {
                if let Ok(body) = resp.json::<Value>().await {
                    if let Some(measurements) = body.get("measurements").and_then(|m| m.as_array())
                    {
                        for m in measurements {
                            let stat = m.get("statistic").and_then(|s| s.as_str()).unwrap_or("");
                            let val = m.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
                            match stat {
                                "COUNT" => data.gc_pause_count = val as u64,
                                "TOTAL_TIME" => data.gc_pause_total_ms = val * 1000.0,
                                _ => {}
                            }
                        }
                    }
                }
            }
            let _ = v; // suppress unused warning
        }

        // -- HTTP requests ----------------------------------------------------
        let http_url = format!(
            "{}/actuator/metrics/http.server.requests",
            base_url.trim_end_matches('/')
        );
        if let Ok(resp) = self.http_client.get(&http_url).send().await {
            if let Ok(body) = resp.json::<Value>().await {
                if let Some(measurements) = body.get("measurements").and_then(|m| m.as_array()) {
                    for m in measurements {
                        let stat = m.get("statistic").and_then(|s| s.as_str()).unwrap_or("");
                        let val = m.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        match stat {
                            "COUNT" => data.http_total_count = val as u64,
                            "TOTAL_TIME" => data.http_total_time_s = val,
                            _ => {}
                        }
                    }
                }
            }
        }
        // HTTP 5xx errors
        let err_url = format!(
            "{}/actuator/metrics/http.server.requests?tag=outcome:SERVER_ERROR",
            base_url.trim_end_matches('/')
        );
        if let Ok(resp) = self.http_client.get(&err_url).send().await {
            if let Ok(body) = resp.json::<Value>().await {
                if let Some(measurements) = body.get("measurements").and_then(|m| m.as_array()) {
                    for m in measurements {
                        let stat = m.get("statistic").and_then(|s| s.as_str()).unwrap_or("");
                        if stat == "COUNT" {
                            data.http_error_count =
                                m.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0) as u64;
                        }
                    }
                }
            }
        }

        // -- Uptime -----------------------------------------------------------
        if let Some(v) = self.fetch_metric(&base_url, "process.uptime").await {
            data.uptime_seconds = v;
        }

        // -- Info -------------------------------------------------------------
        let info_url = format!("{}/actuator/info", base_url.trim_end_matches('/'));
        if let Ok(resp) = self.http_client.get(&info_url).send().await {
            if let Ok(body) = resp.json::<Value>().await {
                if let Some(java) = body
                    .pointer("/java/version")
                    .or_else(|| body.pointer("/java/runtime/version"))
                    .and_then(|v| v.as_str())
                {
                    data.java_version = java.to_string();
                }
                if let Some(sb) = body.pointer("/build/version").and_then(|v| v.as_str()) {
                    data.spring_boot_version = sb.to_string();
                }
            }
        }

        // -- Env: active profiles ---------------------------------------------
        let env_url = format!("{}/actuator/env", base_url.trim_end_matches('/'));
        if let Ok(resp) = self.http_client.get(&env_url).send().await {
            if let Ok(body) = resp.json::<Value>().await {
                if let Some(profiles) = body.get("activeProfiles").and_then(|p| p.as_array()) {
                    data.active_profiles = profiles
                        .iter()
                        .filter_map(|p| p.as_str().map(String::from))
                        .collect();
                }
            }
        }

        // -- Disk -------------------------------------------------------------
        if let Some(v) = self.fetch_metric(&base_url, "disk.free").await {
            data.disk_free_gb = v / 1_073_741_824.0;
        }
        if let Some(v) = self.fetch_metric(&base_url, "disk.total").await {
            data.disk_total_gb = v / 1_073_741_824.0;
        }

        self.dashboard = data;
        Ok(())
    }

    /// Fetch the PID of the connected Spring Boot application from actuator.
    /// Requires `management.endpoint.env.show-values=ALWAYS` in the app config.
    pub async fn fetch_app_pid(&self) -> Result<String> {
        let base_url = self.active_app_url().context("no active app selected")?;

        let url = format!("{}/actuator/env/PID", base_url.trim_end_matches('/'));
        if let Ok(resp) = self.http_client.get(&url).send().await {
            if let Ok(body) = resp.json::<Value>().await {
                if let Some(val) = body.get("property").and_then(|p| p.get("value")) {
                    let pid = val
                        .as_str()
                        .map(String::from)
                        .unwrap_or_else(|| val.to_string());
                    if !pid.is_empty() && pid != "null" && !pid.contains('*') {
                        return Ok(pid);
                    }
                }
            }
        }

        anyhow::bail!(
            "Could not determine PID. The value may be masked.\n\
             Add this to your application.properties:\n\
             management.endpoint.env.show-values=ALWAYS"
        )
    }

    /// Fetch the list of actuator endpoints from `/actuator`.
    pub async fn fetch_endpoints(&mut self) -> Result<()> {
        let base_url = self.active_app_url().context("no active app selected")?;
        let endpoint = format!("{}/actuator", base_url.trim_end_matches('/'));
        let resp = self
            .http_client
            .get(&endpoint)
            .send()
            .await
            .context("failed to fetch actuator index")?;

        let body: Value = resp
            .json()
            .await
            .context("failed to parse actuator index")?;

        let mut endpoints = Vec::new();
        if let Some(links) = body.get("_links").and_then(|l| l.as_object()) {
            for (name, link) in links {
                let url = link
                    .get("href")
                    .and_then(|h| h.as_str())
                    .unwrap_or("")
                    .to_string();
                endpoints.push(Endpoint {
                    name: name.clone(),
                    url,
                });
            }
        }

        self.endpoints = endpoints;
        self.selected_endpoint_index = 0;
        Ok(())
    }

    /// Fetch Spring beans from `/actuator/beans`.
    pub async fn fetch_beans(&mut self) -> Result<()> {
        let base_url = self.active_app_url().context("no active app selected")?;
        let endpoint = format!("{}/actuator/beans", base_url.trim_end_matches('/'));
        let resp = self
            .http_client
            .get(&endpoint)
            .send()
            .await
            .context("failed to fetch beans")?;

        let body: Value = resp
            .json()
            .await
            .context("failed to parse beans response")?;

        let mut beans = Vec::new();
        if let Some(contexts) = body.get("contexts").and_then(|c| c.as_object()) {
            for (_ctx_name, ctx_val) in contexts {
                if let Some(bean_map) = ctx_val.get("beans").and_then(|b| b.as_object()) {
                    for (bean_name, bean_info) in bean_map {
                        let scope = bean_info
                            .get("scope")
                            .and_then(|s| s.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let type_name = bean_info
                            .get("type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        beans.push(Bean {
                            name: bean_name.clone(),
                            scope,
                            type_name,
                        });
                    }
                }
            }
        }

        self.beans = beans;
        self.selected_bean_index = 0;
        Ok(())
    }

    /// Fetch loggers from `/actuator/loggers`.
    pub async fn fetch_loggers(&mut self) -> Result<()> {
        let base_url = self.active_app_url().context("no active app selected")?;
        let endpoint = format!("{}/actuator/loggers", base_url.trim_end_matches('/'));
        let resp = self
            .http_client
            .get(&endpoint)
            .send()
            .await
            .context("failed to fetch loggers")?;

        let body: Value = resp
            .json()
            .await
            .context("failed to parse loggers response")?;

        let mut loggers = Vec::new();
        if let Some(logger_map) = body.get("loggers").and_then(|l| l.as_object()) {
            for (name, info) in logger_map {
                let configured_level = info
                    .get("configuredLevel")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let effective_level = info
                    .get("effectiveLevel")
                    .and_then(|v| v.as_str())
                    .unwrap_or("OFF")
                    .to_string();
                loggers.push(Logger {
                    name: name.clone(),
                    configured_level,
                    effective_level,
                });
            }
        }

        self.loggers = loggers;
        self.selected_logger_index = 0;
        Ok(())
    }

    /// Fetch HTTP request mappings from `/actuator/mappings`.
    pub async fn fetch_mappings(&mut self) -> Result<()> {
        let base_url = self.active_app_url().context("no active app selected")?;
        let endpoint = format!("{}/actuator/mappings", base_url.trim_end_matches('/'));
        let resp = self
            .http_client
            .get(&endpoint)
            .send()
            .await
            .context("failed to fetch mappings")?;

        let body: Value = resp
            .json()
            .await
            .context("failed to parse mappings response")?;

        let mut mappings = Vec::new();
        // Structure: contexts -> {ctx} -> mappings -> {category} -> {name} -> [array]
        // e.g. contexts.application.mappings.dispatcherServlets.dispatcherServlet.[...]
        // Also handle: contexts.application.mappings.servletFilters.[...]
        // Also handle: contexts.application.mappings.servlets.[...]
        if let Some(contexts) = body.get("contexts").and_then(|c| c.as_object()) {
            for (_ctx_name, ctx_val) in contexts {
                if let Some(mapping_categories) =
                    ctx_val.get("mappings").and_then(|m| m.as_object())
                {
                    for (_category_name, category_val) in mapping_categories {
                        // category_val may be:
                        //  - an object with named arrays (dispatcherServlets -> {dispatcherServlet: [...]})
                        //  - a direct array (servletFilters -> [...])
                        let arrays_to_scan: Vec<&Value> =
                            if let Some(obj) = category_val.as_object() {
                                obj.values().collect()
                            } else if category_val.is_array() {
                                vec![category_val]
                            } else {
                                continue;
                            };

                        for arr_val in arrays_to_scan {
                            if let Some(arr) = arr_val.as_array() {
                                for entry in arr {
                                    let pattern = entry
                                        .get("predicate")
                                        .and_then(|p| p.as_str())
                                        .unwrap_or_else(|| {
                                            // Some entries use "details.requestMappingConditions.patterns"
                                            entry
                                                .get("details")
                                                .and_then(|d| d.get("requestMappingConditions"))
                                                .and_then(|r| r.get("patterns"))
                                                .and_then(|p| p.as_array())
                                                .and_then(|a| a.first())
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("unknown")
                                        })
                                        .to_string();
                                    let handler = entry
                                        .get("handler")
                                        .and_then(|h| h.as_str())
                                        .unwrap_or_else(|| {
                                            // servletFilters use "servletNameMappings" or "urlPatternMappings"
                                            entry
                                                .get("name")
                                                .and_then(|n| n.as_str())
                                                .or_else(|| {
                                                    entry.get("className").and_then(|c| c.as_str())
                                                })
                                                .unwrap_or("unknown")
                                        })
                                        .to_string();
                                    mappings.push(Mapping { pattern, handler });
                                }
                            }
                        }
                    }
                }
            }
        }

        self.mappings = mappings;
        self.selected_mapping_index = 0;
        Ok(())
    }

    /// Fetch environment properties from `/actuator/env`.
    pub async fn fetch_env(&mut self) -> Result<()> {
        let base_url = self.active_app_url().context("no active app selected")?;
        let endpoint = format!("{}/actuator/env", base_url.trim_end_matches('/'));
        let resp = self
            .http_client
            .get(&endpoint)
            .send()
            .await
            .context("failed to fetch env")?;

        let body: Value = resp.json().await.context("failed to parse env response")?;

        let mut env_props = Vec::new();
        if let Some(property_sources) = body.get("propertySources").and_then(|p| p.as_array()) {
            for source in property_sources {
                let source_name = source
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                if let Some(props) = source.get("properties").and_then(|p| p.as_object()) {
                    for (prop_name, prop_val) in props {
                        let value = prop_val
                            .get("value")
                            .map(|v| match v {
                                Value::String(s) => s.clone(),
                                other => other.to_string(),
                            })
                            .unwrap_or_default();
                        env_props.push(EnvProperty {
                            name: prop_name.clone(),
                            value,
                            source: source_name.clone(),
                        });
                    }
                }
            }
        }

        self.env_props = env_props;
        self.selected_env_index = 0;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Thread dump
    // -----------------------------------------------------------------------

    /// Fetch a thread dump from `/actuator/threaddump`, save to file, and
    /// track it in `saved_thread_dumps`. Returns the saved file path.
    pub async fn fetch_and_save_thread_dump(&mut self) -> Result<String> {
        let base_url = self.active_app_url().context("no active app selected")?;
        let app_name = self.current_server_name();
        let endpoint = format!("{}/actuator/threaddump", base_url.trim_end_matches('/'));
        let resp = self
            .http_client
            .get(&endpoint)
            .send()
            .await
            .context("failed to connect to the application")?;

        if !resp.status().is_success() {
            let status = resp.status();
            if status.as_u16() == 404 {
                anyhow::bail!(
                    "Thread dump endpoint not found.\n\n\
                     The /actuator/threaddump endpoint is not available.\n\
                     Make sure your application has:\n\
                     1. spring-boot-starter-actuator dependency\n\
                     2. management.endpoints.web.exposure.include=threaddump\n   \
                        (or include=* to expose all endpoints)"
                );
            }
            anyhow::bail!(
                "Thread dump request failed with status {}.\n\
                 The endpoint may not be enabled or accessible.",
                status
            );
        }

        let body: Value = resp
            .json()
            .await
            .context("failed to parse thread dump response")?;

        // Save raw JSON to per-app directory
        let raw_json = serde_json::to_string_pretty(&body).unwrap_or_else(|_| body.to_string());
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
        let filename = format!("threaddump_{}.json", timestamp);
        let dir = Self::app_dumps_dir(&app_name)?;
        let path = dir.join(&filename);
        std::fs::write(&path, &raw_json)
            .with_context(|| format!("failed to write thread dump to {}", path.display()))?;

        // Also write a .tdump in standard JVM text format for VisualVM
        let tdump_filename = format!("threaddump_{}.tdump", timestamp);
        let tdump_path = dir.join(&tdump_filename);
        let tdump_text = Self::thread_dump_to_jvm_text(&body);
        let _ = std::fs::write(&tdump_path, &tdump_text);

        let size_bytes = raw_json.len() as u64;
        let path_str = path.to_string_lossy().to_string();

        self.saved_thread_dumps.push(crate::model::SavedDump {
            app_url: base_url,
            app_name,
            path: path_str.clone(),
            timestamp,
            size_bytes,
        });

        Ok(path_str)
    }

    /// Convert Spring Boot actuator thread dump JSON into standard JVM text
    /// format that VisualVM can open as a `.tdump` file.
    fn thread_dump_to_jvm_text(body: &Value) -> String {
        let mut out = String::new();
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        out.push_str(&format!("{}\nFull thread dump:\n\n", now));

        if let Some(threads) = body.get("threads").and_then(|t| t.as_array()) {
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
                let daemon = thread
                    .get("daemon")
                    .and_then(|d| d.as_bool())
                    .unwrap_or(false);
                let daemon_str = if daemon { " daemon" } else { "" };

                out.push_str(&format!(
                    "\"{}\" #{}{} java.lang.Thread.State: {}\n",
                    name, id, daemon_str, state
                ));

                if let Some(stack) = thread.get("stackTrace").and_then(|s| s.as_array()) {
                    for frame in stack {
                        let class = frame
                            .get("className")
                            .and_then(|c| c.as_str())
                            .unwrap_or("Unknown");
                        let method = frame
                            .get("methodName")
                            .and_then(|m| m.as_str())
                            .unwrap_or("unknown");
                        let file = frame.get("fileName").and_then(|f| f.as_str());
                        let line = frame
                            .get("lineNumber")
                            .and_then(|l| l.as_i64())
                            .unwrap_or(-1);
                        let native = frame
                            .get("nativeMethod")
                            .and_then(|n| n.as_bool())
                            .unwrap_or(false);

                        let location = if native {
                            "Native Method".to_string()
                        } else if let Some(f) = file {
                            if line >= 0 {
                                format!("{}:{}", f, line)
                            } else {
                                f.to_string()
                            }
                        } else {
                            "Unknown Source".to_string()
                        };

                        out.push_str(&format!("\tat {}.{}({})\n", class, method, location));
                    }
                }
                out.push('\n');
            }
        }
        out
    }

    // -----------------------------------------------------------------------
    // Heap dump
    // -----------------------------------------------------------------------

    /// Trigger a heap dump download from `/actuator/heapdump`, save to
    /// a local file, and track it in `saved_heap_dumps`. Returns the path.
    pub async fn download_heap_dump(&mut self) -> Result<String> {
        let base_url = self.active_app_url().context("no active app selected")?;
        let app_name = self.current_server_name();
        let endpoint = format!("{}/actuator/heapdump", base_url.trim_end_matches('/'));

        let resp = self
            .http_client
            .get(&endpoint)
            .send()
            .await
            .context("failed to connect to the application")?;

        if !resp.status().is_success() {
            let status = resp.status();
            if status.as_u16() == 404 {
                anyhow::bail!(
                    "Heap dump endpoint not found.\n\n\
                     The /actuator/heapdump endpoint is not available.\n\
                     Make sure your application has:\n\
                     1. spring-boot-starter-actuator dependency\n\
                     2. management.endpoints.web.exposure.include=heapdump\n   \
                        (or include=* to expose all endpoints)"
                );
            }
            anyhow::bail!(
                "Heap dump request failed with status {}.\n\
                 The endpoint may not be enabled or accessible.",
                status
            );
        }

        let bytes = resp
            .bytes()
            .await
            .context("failed to read heap dump response body")?;

        // Write to per-app dumps directory
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
        let filename = format!("heapdump_{}.hprof", timestamp);
        let dir = Self::app_dumps_dir(&app_name)?;
        let path = dir.join(&filename);
        let size_bytes = bytes.len() as u64;
        std::fs::write(&path, &bytes)
            .with_context(|| format!("failed to write heap dump to {}", path.display()))?;

        let path_str = path.to_string_lossy().to_string();

        self.saved_heap_dumps.push(crate::model::SavedDump {
            app_url: base_url,
            app_name,
            path: path_str.clone(),
            timestamp,
            size_bytes,
        });

        Ok(path_str)
    }

    // -----------------------------------------------------------------------
    // Command palette
    // -----------------------------------------------------------------------

    /// Recompute `command_suggestions` based on the current `command_text`.
    pub fn update_command_suggestions(&mut self) {
        let query = self.command_text.to_lowercase();
        self.command_suggestions = self
            .resources
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                if query.is_empty() {
                    return true;
                }
                r.name.to_lowercase().contains(&query)
                    || r.command.to_lowercase().contains(&query)
                    || r.description.to_lowercase().contains(&query)
            })
            .map(|(i, _)| i)
            .collect();

        // Reset selection when the suggestion list changes.
        self.command_suggestion_selected = 0;
    }

    /// Returns the currently highlighted command-palette resource, if any.
    pub fn get_selected_command(&self) -> Option<&ResourceItem> {
        self.command_suggestions
            .get(self.command_suggestion_selected)
            .and_then(|&idx| self.resources.get(idx))
    }

    // -----------------------------------------------------------------------
    // Tick
    // -----------------------------------------------------------------------

    /// Called on every tick of the main event loop. Advances the spinner
    /// frame counter used for animated loading indicators.
    pub fn on_tick(&mut self) {
        self.spinner_frame = self.spinner_frame.wrapping_add(1);
    }

    // -----------------------------------------------------------------------
    // -----------------------------------------------------------------------
    // Local Metadata management
    // -----------------------------------------------------------------------

    fn initializr_metadata_path() -> PathBuf {
        crate::config::TsbConfig::config_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("initializr-metadata.json")
    }

    fn initializr_dependencies_path() -> PathBuf {
        crate::config::TsbConfig::config_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("dependencies.json")
    }

    pub fn ensure_local_metadata() -> Result<()> {
        let meta_path = Self::initializr_metadata_path();
        let deps_path = Self::initializr_dependencies_path();

        if let Some(parent) = meta_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        if !meta_path.exists() {
            std::fs::write(&meta_path, EMBEDDED_METADATA)?;
        }
        if !deps_path.exists() {
            std::fs::write(&deps_path, EMBEDDED_DEPENDENCIES)?;
        }
        Ok(())
    }

    pub fn sync_metadata_from_github(client: reqwest::Client) {
        tokio::spawn(async move {
            let meta_path = Self::initializr_metadata_path();
            let deps_path = Self::initializr_dependencies_path();

            if let Ok(resp) = client.get(GITHUB_METADATA_URL).send().await {
                if resp.status().is_success() {
                    if let Ok(text) = resp.text().await {
                        let _ = std::fs::write(&meta_path, text);
                    }
                }
            }

            if let Ok(resp) = client.get(GITHUB_DEPENDENCIES_URL).send().await {
                if resp.status().is_success() {
                    if let Ok(text) = resp.text().await {
                        let _ = std::fs::write(&deps_path, text);
                    }
                }
            }
        });
    }

    pub async fn fetch_initializr_metadata(client: &reqwest::Client) -> Result<InitializrMetadata> {
        // Ensure local defaults exist before reading
        let _ = Self::ensure_local_metadata();

        // Trigger an async background sync every time we need metadata
        // (This way we gradually stay updated)
        Self::sync_metadata_from_github(client.clone());

        let meta_path = Self::initializr_metadata_path();
        let data =
            std::fs::read_to_string(&meta_path).context("failed to read local metadata file")?;
        let body: Value =
            serde_json::from_str(&data).context("failed to parse local metadata JSON")?;

        let parsed = Self::parse_initializr_metadata(&body)?;
        Ok(parsed)
    }

    /// Parse the raw JSON from the initializr metadata into our
    /// `InitializrMetadata` struct.
    fn parse_initializr_metadata(body: &Value) -> Result<InitializrMetadata> {
        fn extract_options(body: &Value, key: &str) -> (Vec<InitializrOption>, String) {
            let section = &body[key];
            let default = section["default"].as_str().unwrap_or("").to_string();
            let values = section["values"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .map(|v| InitializrOption {
                            id: v["id"].as_str().unwrap_or("").to_string(),
                            name: v["name"].as_str().unwrap_or("").to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            (values, default)
        }

        fn extract_text_default(body: &Value, key: &str) -> String {
            body[key]["default"].as_str().unwrap_or("").to_string()
        }

        let (boot_versions, boot_version_default) = extract_options(body, "bootVersion");
        let (languages, language_default) = extract_options(body, "language");
        let (packagings, packaging_default) = extract_options(body, "packaging");
        let (java_versions, java_version_default) = extract_options(body, "javaVersion");
        let (project_types, project_type_default) = extract_options(body, "type");

        // Dependencies are nested: groups → values
        let dependency_groups = body["dependencies"]["values"]
            .as_array()
            .map(|groups| {
                groups
                    .iter()
                    .map(|g| {
                        let name = g["name"].as_str().unwrap_or("").to_string();
                        let values = g["values"]
                            .as_array()
                            .map(|deps| {
                                deps.iter()
                                    .map(|d| InitializrDependency {
                                        id: d["id"].as_str().unwrap_or("").to_string(),
                                        name: d["name"].as_str().unwrap_or("").to_string(),
                                        description: d["description"]
                                            .as_str()
                                            .unwrap_or("")
                                            .to_string(),
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        InitializrDependencyGroup { name, values }
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(InitializrMetadata {
            boot_versions,
            boot_version_default,
            languages,
            language_default,
            packagings,
            packaging_default,
            java_versions,
            java_version_default,
            project_types,
            project_type_default,
            dependency_groups,
            group_id_default: extract_text_default(body, "groupId"),
            artifact_id_default: extract_text_default(body, "artifactId"),
            version_default: extract_text_default(body, "version"),
            name_default: extract_text_default(body, "name"),
            description_default: extract_text_default(body, "description"),
            package_name_default: extract_text_default(body, "packageName"),
        })
    }

    // -----------------------------------------------------------------------
    // Spring Initializr — project generation (download + extract zip)
    // -----------------------------------------------------------------------

    /// Download a project zip from Spring Initializr and extract it to the
    /// specified output directory. Returns the path of the extracted project.
    pub fn generate_project(params: &NewProjectParams) -> Result<String> {
        crate::generator::generate_project(params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Create a minimal App for testing. Avoids the full App::new() which
    /// touches the filesystem for config.
    fn test_app() -> App {
        let resources = vec![
            ResourceItem {
                name: "Apps".into(),
                command: ":apps".into(),
                description: "List connected Spring Boot applications".into(),
            },
            ResourceItem {
                name: "Dashboard".into(),
                command: ":dashboard".into(),
                description: "Application health, metrics and JVM overview".into(),
            },
            ResourceItem {
                name: "Beans".into(),
                command: ":beans".into(),
                description: "View registered Spring beans".into(),
            },
            ResourceItem {
                name: "Loggers".into(),
                command: ":loggers".into(),
                description: "View and manage logger levels".into(),
            },
        ];
        App {
            mode: Mode::Normal,
            should_quit: false,
            active_resource: "apps".into(),
            apps: vec![
                SpringApp {
                    name: "app-one".into(),
                    url: "http://localhost:8080".into(),
                    status: AppStatus::Up,
                },
                SpringApp {
                    name: "app-two".into(),
                    url: "http://localhost:9090".into(),
                    status: AppStatus::Down,
                },
                SpringApp {
                    name: "backend-service".into(),
                    url: "http://localhost:7070".into(),
                    status: AppStatus::Up,
                },
            ],
            endpoints: vec![
                Endpoint {
                    name: "health".into(),
                    url: "/actuator/health".into(),
                },
                Endpoint {
                    name: "info".into(),
                    url: "/actuator/info".into(),
                },
            ],
            beans: vec![
                Bean {
                    name: "myBean".into(),
                    scope: "singleton".into(),
                    type_name: "com.example.MyBean".into(),
                },
                Bean {
                    name: "dataSource".into(),
                    scope: "singleton".into(),
                    type_name: "javax.sql.DataSource".into(),
                },
            ],
            loggers: vec![
                Logger {
                    name: "com.example".into(),
                    configured_level: Some("DEBUG".into()),
                    effective_level: "DEBUG".into(),
                },
                Logger {
                    name: "org.springframework".into(),
                    configured_level: None,
                    effective_level: "INFO".into(),
                },
            ],
            mappings: vec![Mapping {
                pattern: "/api/users".into(),
                handler: "UserController#list".into(),
            }],
            env_props: vec![EnvProperty {
                name: "server.port".into(),
                value: "8080".into(),
                source: "application.properties".into(),
            }],
            server_info: None,
            dashboard: DashboardData::default(),
            selected_app_index: 0,
            selected_endpoint_index: 0,
            selected_bean_index: 0,
            selected_logger_index: 0,
            selected_mapping_index: 0,
            selected_env_index: 0,
            describe_scroll: 0,
            describe_content: String::new(),
            describe_title: String::new(),
            filter_text: String::new(),
            filter_active: false,
            command_suggestions: (0..resources.len()).collect(),
            command_text: String::new(),
            command_suggestion_selected: 0,
            resources,
            splash_state: SplashState::default(),
            server_dialog_state: ServerDialogState::default(),
            edit_logger_state: EditLoggerState::default(),
            new_project_state: NewProjectWizardState::default(),
            config: crate::config::TsbConfig::default(),
            error_prev_mode: None,
            modal_title: String::new(),
            modal_msg: String::new(),
            width: 0,
            height: 0,
            spinner_frame: 0,
            last_key_press: None,
            saved_thread_dumps: Vec::new(),
            saved_heap_dumps: Vec::new(),
            selected_thread_dump_index: 0,
            selected_heap_dump_index: 0,
            parsed_threads: Vec::new(),
            thread_viz_scroll: 0,
            thread_viz_title: String::new(),
            http_client: reqwest::Client::new(),
        }
    }

    // =====================================================================
    // filtered_indices
    // =====================================================================

    #[test]
    fn filtered_indices_no_filter_returns_all() {
        let app = test_app();
        let indices = app.filtered_indices();
        assert_eq!(indices, vec![0, 1, 2]);
    }

    #[test]
    fn filtered_indices_by_name() {
        let mut app = test_app();
        app.filter_text = "backend".into();
        let indices = app.filtered_indices();
        assert_eq!(indices, vec![2]);
    }

    #[test]
    fn filtered_indices_by_url() {
        let mut app = test_app();
        app.filter_text = "9090".into();
        let indices = app.filtered_indices();
        assert_eq!(indices, vec![1]);
    }

    #[test]
    fn filtered_indices_by_status() {
        let mut app = test_app();
        app.filter_text = "down".into();
        let indices = app.filtered_indices();
        assert_eq!(indices, vec![1]);
    }

    #[test]
    fn filtered_indices_case_insensitive() {
        let mut app = test_app();
        app.filter_text = "APP-ONE".into();
        let indices = app.filtered_indices();
        assert_eq!(indices, vec![0]);
    }

    #[test]
    fn filtered_indices_no_match() {
        let mut app = test_app();
        app.filter_text = "nonexistent".into();
        let indices = app.filtered_indices();
        assert!(indices.is_empty());
    }

    #[test]
    fn filtered_indices_endpoints() {
        let mut app = test_app();
        app.active_resource = "endpoints".into();
        app.filter_text = "health".into();
        let indices = app.filtered_indices();
        assert_eq!(indices, vec![0]);
    }

    #[test]
    fn filtered_indices_beans() {
        let mut app = test_app();
        app.active_resource = "beans".into();
        app.filter_text = "DataSource".into();
        let indices = app.filtered_indices();
        assert_eq!(indices, vec![1]);
    }

    #[test]
    fn filtered_indices_loggers_by_effective_level() {
        let mut app = test_app();
        app.active_resource = "loggers".into();
        app.filter_text = "info".into();
        let indices = app.filtered_indices();
        assert_eq!(indices, vec![1]);
    }

    #[test]
    fn filtered_indices_unknown_resource() {
        let mut app = test_app();
        app.active_resource = "unknown".into();
        let indices = app.filtered_indices();
        assert!(indices.is_empty());
    }

    // =====================================================================
    // Navigation: next / previous / go_to_top / go_to_bottom
    // =====================================================================

    #[test]
    fn next_moves_forward() {
        let mut app = test_app();
        assert_eq!(app.selected_app_index, 0);
        app.next();
        assert_eq!(app.selected_app_index, 1);
        app.next();
        assert_eq!(app.selected_app_index, 2);
    }

    #[test]
    fn next_stays_at_last() {
        let mut app = test_app();
        app.selected_app_index = 2;
        app.next();
        assert_eq!(app.selected_app_index, 2);
    }

    #[test]
    fn previous_moves_back() {
        let mut app = test_app();
        app.selected_app_index = 2;
        app.previous();
        assert_eq!(app.selected_app_index, 1);
    }

    #[test]
    fn previous_stays_at_first() {
        let mut app = test_app();
        app.previous();
        assert_eq!(app.selected_app_index, 0);
    }

    #[test]
    fn next_on_empty_list_is_noop() {
        let mut app = test_app();
        app.apps.clear();
        app.next();
        assert_eq!(app.selected_app_index, 0);
    }

    #[test]
    fn go_to_top() {
        let mut app = test_app();
        app.selected_app_index = 2;
        app.go_to_top();
        assert_eq!(app.selected_app_index, 0);
    }

    #[test]
    fn go_to_bottom() {
        let mut app = test_app();
        app.go_to_bottom();
        assert_eq!(app.selected_app_index, 2);
    }

    #[test]
    fn navigation_respects_filter() {
        let mut app = test_app();
        // Filter to only "app-one" (index 0) and "app-two" (index 1)
        app.filter_text = "app-".into();
        app.selected_app_index = 0;
        app.next();
        assert_eq!(app.selected_app_index, 1); // next filtered item
        app.next();
        assert_eq!(app.selected_app_index, 1); // stays at last filtered
    }

    // =====================================================================
    // active_index / active_index_mut
    // =====================================================================

    #[test]
    fn active_index_dispatches_correctly() {
        let mut app = test_app();
        app.active_resource = "beans".into();
        app.selected_bean_index = 1;
        assert_eq!(app.active_index(), 1);

        app.active_resource = "loggers".into();
        app.selected_logger_index = 0;
        assert_eq!(app.active_index(), 0);
    }

    // =====================================================================
    // active_app_url / current_server_name
    // =====================================================================

    #[test]
    fn active_app_url_from_config() {
        let mut app = test_app();
        app.config.active_app_url = Some("http://override:1234".into());
        assert_eq!(app.active_app_url(), Some("http://override:1234".into()));
    }

    #[test]
    fn active_app_url_fallback_to_selected() {
        let app = test_app();
        assert_eq!(app.active_app_url(), Some("http://localhost:8080".into()));
    }

    #[test]
    fn active_app_url_empty_apps_and_no_config() {
        let mut app = test_app();
        app.apps.clear();
        assert_eq!(app.active_app_url(), None);
    }

    #[test]
    fn current_server_name_from_active_url() {
        let mut app = test_app();
        app.config.active_app_url = Some("http://localhost:9090".into());
        assert_eq!(app.current_server_name(), "app-two");
    }

    #[test]
    fn current_server_name_no_server() {
        let mut app = test_app();
        app.apps.clear();
        assert_eq!(app.current_server_name(), "No server");
    }

    // =====================================================================
    // show_error
    // =====================================================================

    #[test]
    fn show_error_sets_modal_and_mode() {
        let mut app = test_app();
        app.mode = Mode::Normal;
        app.show_error("something went wrong");
        assert_eq!(app.mode, Mode::ErrorModal);
        assert_eq!(app.modal_title, "Error");
        assert_eq!(app.modal_msg, "something went wrong");
        assert_eq!(app.error_prev_mode, Some(Mode::Normal));
    }

    #[test]
    fn show_error_preserves_previous_mode() {
        let mut app = test_app();
        app.mode = Mode::Describe;
        app.show_error("test");
        assert_eq!(app.error_prev_mode, Some(Mode::Describe));
    }

    // =====================================================================
    // update_command_suggestions / get_selected_command
    // =====================================================================

    #[test]
    fn command_suggestions_empty_query_returns_all() {
        let mut app = test_app();
        app.command_text = "".into();
        app.update_command_suggestions();
        assert_eq!(app.command_suggestions.len(), app.resources.len());
    }

    #[test]
    fn command_suggestions_filter_by_name() {
        let mut app = test_app();
        app.command_text = "bean".into();
        app.update_command_suggestions();
        assert_eq!(app.command_suggestions.len(), 1);
        let cmd = app.get_selected_command().unwrap();
        assert_eq!(cmd.name, "Beans");
    }

    #[test]
    fn command_suggestions_filter_by_command() {
        let mut app = test_app();
        app.command_text = ":dashboard".into();
        app.update_command_suggestions();
        assert_eq!(app.command_suggestions.len(), 1);
        let cmd = app.get_selected_command().unwrap();
        assert_eq!(cmd.command, ":dashboard");
    }

    #[test]
    fn command_suggestions_filter_by_description() {
        let mut app = test_app();
        app.command_text = "Spring".into();
        app.update_command_suggestions();
        // "List connected Spring Boot applications" in Apps description
        assert!(!app.command_suggestions.is_empty());
    }

    #[test]
    fn command_suggestions_no_match() {
        let mut app = test_app();
        app.command_text = "zzzzz".into();
        app.update_command_suggestions();
        assert!(app.command_suggestions.is_empty());
        assert_eq!(app.get_selected_command(), None);
    }

    #[test]
    fn command_suggestions_resets_selection() {
        let mut app = test_app();
        app.command_suggestion_selected = 5;
        app.command_text = "bean".into();
        app.update_command_suggestions();
        assert_eq!(app.command_suggestion_selected, 0);
    }

    // =====================================================================
    // on_tick
    // =====================================================================

    #[test]
    fn on_tick_advances_spinner() {
        let mut app = test_app();
        assert_eq!(app.spinner_frame, 0);
        app.on_tick();
        assert_eq!(app.spinner_frame, 1);
    }

    // =====================================================================
    // parse_initializr_metadata
    // =====================================================================

    #[test]
    fn parse_metadata_full() {
        let body = json!({
            "bootVersion": {
                "default": "3.4.0",
                "values": [
                    {"id": "3.4.0", "name": "3.4.0"},
                    {"id": "3.3.0", "name": "3.3.0"}
                ]
            },
            "language": {
                "default": "java",
                "values": [
                    {"id": "java", "name": "Java"},
                    {"id": "kotlin", "name": "Kotlin"}
                ]
            },
            "packaging": {
                "default": "jar",
                "values": [{"id": "jar", "name": "Jar"}]
            },
            "javaVersion": {
                "default": "21",
                "values": [{"id": "21", "name": "21"}]
            },
            "type": {
                "default": "maven-project",
                "values": [{"id": "maven-project", "name": "Maven"}]
            },
            "dependencies": {
                "values": [{
                    "name": "Web",
                    "values": [
                        {"id": "web", "name": "Spring Web", "description": "Build web apps"}
                    ]
                }]
            },
            "groupId": {"default": "com.example"},
            "artifactId": {"default": "demo"},
            "name": {"default": "demo"},
            "description": {"default": "Demo project"},
            "version": {"default": "0.0.1-SNAPSHOT"},
            "packageName": {"default": "com.example.demo"}
        });

        let meta = App::parse_initializr_metadata(&body).unwrap();
        assert_eq!(meta.boot_versions.len(), 2);
        assert_eq!(meta.boot_version_default, "3.4.0");
        assert_eq!(meta.languages.len(), 2);
        assert_eq!(meta.language_default, "java");
        assert_eq!(meta.group_id_default, "com.example");
        assert_eq!(meta.name_default, "demo");
        assert_eq!(meta.dependency_groups.len(), 1);
        assert_eq!(meta.dependency_groups[0].values[0].id, "web");
    }

    #[test]
    fn parse_metadata_empty_json() {
        let body = json!({});
        let meta = App::parse_initializr_metadata(&body).unwrap();
        assert!(meta.boot_versions.is_empty());
        assert!(meta.dependency_groups.is_empty());
        assert_eq!(meta.group_id_default, "");
    }

    #[test]
    fn parse_metadata_missing_name_default() {
        let body = json!({
            "name": {"type": "text"},
            "groupId": {"default": "org.test"}
        });
        let meta = App::parse_initializr_metadata(&body).unwrap();
        assert_eq!(meta.name_default, ""); // no default key
        assert_eq!(meta.group_id_default, "org.test");
    }

    // =====================================================================
    // thread_dump_to_jvm_text
    // =====================================================================

    #[test]
    fn thread_dump_to_text_basic() {
        let body = json!({
            "threads": [{
                "threadName": "main",
                "threadId": 1,
                "threadState": "RUNNABLE",
                "daemon": false,
                "stackTrace": [{
                    "className": "com.example.Main",
                    "methodName": "run",
                    "fileName": "Main.java",
                    "lineNumber": 42,
                    "nativeMethod": false
                }]
            }]
        });

        let text = App::thread_dump_to_jvm_text(&body);
        assert!(text.contains("\"main\" #1 java.lang.Thread.State: RUNNABLE"));
        assert!(text.contains("at com.example.Main.run(Main.java:42)"));
    }

    #[test]
    fn thread_dump_to_text_daemon() {
        let body = json!({
            "threads": [{
                "threadName": "gc",
                "threadId": 2,
                "threadState": "WAITING",
                "daemon": true,
                "stackTrace": []
            }]
        });

        let text = App::thread_dump_to_jvm_text(&body);
        assert!(text.contains("\"gc\" #2 daemon java.lang.Thread.State: WAITING"));
    }

    #[test]
    fn thread_dump_to_text_native_method() {
        let body = json!({
            "threads": [{
                "threadName": "t1",
                "threadId": 3,
                "threadState": "RUNNABLE",
                "daemon": false,
                "stackTrace": [{
                    "className": "java.net.SocketInputStream",
                    "methodName": "read0",
                    "fileName": null,
                    "lineNumber": -2,
                    "nativeMethod": true
                }]
            }]
        });

        let text = App::thread_dump_to_jvm_text(&body);
        assert!(text.contains("at java.net.SocketInputStream.read0(Native Method)"));
    }

    #[test]
    fn thread_dump_to_text_empty_threads() {
        let body = json!({"threads": []});
        let text = App::thread_dump_to_jvm_text(&body);
        assert!(text.contains("Full thread dump"));
        // No thread entries
        assert!(!text.contains("java.lang.Thread.State"));
    }

    #[test]
    fn thread_dump_to_text_missing_threads_key() {
        let body = json!({});
        let text = App::thread_dump_to_jvm_text(&body);
        assert!(text.contains("Full thread dump"));
    }

    // =====================================================================
    // NewProjectWizardState::apply_metadata_defaults
    // =====================================================================

    #[test]
    fn apply_metadata_defaults_populates_fields() {
        let mut ws = NewProjectWizardState::default();
        let meta = InitializrMetadata {
            boot_versions: vec![
                InitializrOption {
                    id: "3.3.0".into(),
                    name: "3.3.0".into(),
                },
                InitializrOption {
                    id: "3.4.0".into(),
                    name: "3.4.0".into(),
                },
            ],
            boot_version_default: "3.4.0".into(),
            languages: vec![
                InitializrOption {
                    id: "java".into(),
                    name: "Java".into(),
                },
                InitializrOption {
                    id: "kotlin".into(),
                    name: "Kotlin".into(),
                },
            ],
            language_default: "kotlin".into(),
            packagings: vec![InitializrOption {
                id: "jar".into(),
                name: "Jar".into(),
            }],
            packaging_default: "jar".into(),
            java_versions: vec![
                InitializrOption {
                    id: "17".into(),
                    name: "17".into(),
                },
                InitializrOption {
                    id: "21".into(),
                    name: "21".into(),
                },
            ],
            java_version_default: "21".into(),
            project_types: vec![InitializrOption {
                id: "maven-project".into(),
                name: "Maven".into(),
            }],
            project_type_default: "maven-project".into(),
            dependency_groups: vec![],
            group_id_default: "org.test".into(),
            artifact_id_default: "myproject".into(),
            version_default: "1.0.0".into(),
            name_default: "myproject".into(),
            description_default: "My desc".into(),
            package_name_default: "org.test.myproject".into(),
        };

        ws.apply_metadata_defaults(&meta);

        assert_eq!(ws.group_id, "org.test");
        assert_eq!(ws.artifact_id, "myproject");
        assert_eq!(ws.name, "myproject");
        assert_eq!(ws.description, "My desc");
        assert_eq!(ws.package_name, "org.test.myproject");
        assert_eq!(ws.boot_version_idx, 1); // "3.4.0" is at index 1
        assert_eq!(ws.language_idx, 1); // "kotlin" is at index 1
        assert_eq!(ws.java_version_idx, 1); // "21" is at index 1
    }

    #[test]
    fn apply_metadata_defaults_empty_values_preserve_wizard_defaults() {
        let mut ws = NewProjectWizardState::default();
        let original_name = ws.name.clone();
        let original_group = ws.group_id.clone();

        let meta = InitializrMetadata {
            boot_versions: vec![],
            boot_version_default: "".into(),
            languages: vec![],
            language_default: "".into(),
            packagings: vec![],
            packaging_default: "".into(),
            java_versions: vec![],
            java_version_default: "".into(),
            project_types: vec![],
            project_type_default: "".into(),
            dependency_groups: vec![],
            group_id_default: "".into(),
            artifact_id_default: "".into(),
            version_default: "".into(),
            name_default: "".into(), // empty -> should NOT overwrite
            description_default: "".into(),
            package_name_default: "".into(),
        };

        ws.apply_metadata_defaults(&meta);

        assert_eq!(ws.name, original_name); // "demo" preserved
        assert_eq!(ws.group_id, original_group); // "com.example" preserved
    }

    #[test]
    fn apply_metadata_default_not_found_falls_back_to_zero() {
        let mut ws = NewProjectWizardState::default();
        let meta = InitializrMetadata {
            boot_versions: vec![InitializrOption {
                id: "3.3.0".into(),
                name: "3.3.0".into(),
            }],
            boot_version_default: "nonexistent".into(),
            languages: vec![],
            language_default: "".into(),
            packagings: vec![],
            packaging_default: "".into(),
            java_versions: vec![],
            java_version_default: "".into(),
            project_types: vec![],
            project_type_default: "".into(),
            dependency_groups: vec![],
            group_id_default: "".into(),
            artifact_id_default: "".into(),
            version_default: "".into(),
            name_default: "".into(),
            description_default: "".into(),
            package_name_default: "".into(),
        };

        ws.apply_metadata_defaults(&meta);
        assert_eq!(ws.boot_version_idx, 0); // default not found -> 0
    }

    // =====================================================================
    // Async HTTP tests (wiremock)
    // =====================================================================

    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Create a test App pointed at a mock server URL.
    fn test_app_with_url(base_url: &str) -> App {
        let mut app = test_app();
        app.apps = vec![crate::model::SpringApp {
            name: "mock-app".into(),
            url: base_url.to_string(),
            status: crate::model::AppStatus::Up,
        }];
        app.selected_app_index = 0;
        app.config.active_app_url = Some(base_url.to_string());
        app
    }

    // -- check_health --

    #[tokio::test]
    async fn check_health_up() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/actuator/health"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"status": "UP"})),
            )
            .mount(&server)
            .await;

        let app = test_app_with_url(&server.uri());
        let status = app.check_health(&server.uri()).await.unwrap();
        assert_eq!(status, crate::model::AppStatus::Up);
    }

    #[tokio::test]
    async fn check_health_down_status() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/actuator/health"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"status": "DOWN"})),
            )
            .mount(&server)
            .await;

        let app = test_app_with_url(&server.uri());
        let status = app.check_health(&server.uri()).await.unwrap();
        assert_eq!(status, crate::model::AppStatus::Down);
    }

    #[tokio::test]
    async fn check_health_http_error_returns_down() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/actuator/health"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let app = test_app_with_url(&server.uri());
        let status = app.check_health(&server.uri()).await.unwrap();
        assert_eq!(status, crate::model::AppStatus::Down);
    }

    #[tokio::test]
    async fn check_health_unknown_status() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/actuator/health"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"status": "OUT_OF_SERVICE"})),
            )
            .mount(&server)
            .await;

        let app = test_app_with_url(&server.uri());
        let status = app.check_health(&server.uri()).await.unwrap();
        assert_eq!(status, crate::model::AppStatus::Unknown);
    }

    // -- fetch_endpoints --

    #[tokio::test]
    async fn fetch_endpoints_parses_links() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/actuator"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "_links": {
                    "health": {"href": "/actuator/health"},
                    "info": {"href": "/actuator/info"},
                    "beans": {"href": "/actuator/beans"}
                }
            })))
            .mount(&server)
            .await;

        let mut app = test_app_with_url(&server.uri());
        app.fetch_endpoints().await.unwrap();
        assert_eq!(app.endpoints.len(), 3);
        assert!(app.endpoints.iter().any(|e| e.name == "health"));
        assert!(app.endpoints.iter().any(|e| e.name == "beans"));
    }

    #[tokio::test]
    async fn fetch_endpoints_empty_links() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/actuator"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"_links": {}})),
            )
            .mount(&server)
            .await;

        let mut app = test_app_with_url(&server.uri());
        app.fetch_endpoints().await.unwrap();
        assert!(app.endpoints.is_empty());
    }

    // -- fetch_beans --

    #[tokio::test]
    async fn fetch_beans_parses_contexts() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/actuator/beans"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "contexts": {
                    "application": {
                        "beans": {
                            "myService": {
                                "scope": "singleton",
                                "type": "com.example.MyService",
                                "dependencies": []
                            },
                            "dataSource": {
                                "scope": "singleton",
                                "type": "javax.sql.DataSource",
                                "dependencies": []
                            }
                        }
                    }
                }
            })))
            .mount(&server)
            .await;

        let mut app = test_app_with_url(&server.uri());
        app.fetch_beans().await.unwrap();
        assert_eq!(app.beans.len(), 2);
        assert!(app.beans.iter().any(|b| b.name == "myService"));
        assert!(app
            .beans
            .iter()
            .any(|b| b.type_name == "javax.sql.DataSource"));
    }

    // -- fetch_loggers --

    #[tokio::test]
    async fn fetch_loggers_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/actuator/loggers"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "loggers": {
                    "com.example": {
                        "configuredLevel": "DEBUG",
                        "effectiveLevel": "DEBUG"
                    },
                    "org.springframework": {
                        "configuredLevel": null,
                        "effectiveLevel": "INFO"
                    }
                }
            })))
            .mount(&server)
            .await;

        let mut app = test_app_with_url(&server.uri());
        app.fetch_loggers().await.unwrap();
        assert_eq!(app.loggers.len(), 2);

        let spring = app
            .loggers
            .iter()
            .find(|l| l.name == "org.springframework")
            .unwrap();
        assert_eq!(spring.effective_level, "INFO");
        assert!(spring.configured_level.is_none());
    }

    // -- set_logger_level --

    #[tokio::test]
    async fn set_logger_level_updates_local_state() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/actuator/loggers/com.example"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let mut app = test_app_with_url(&server.uri());
        // Pre-populate a logger
        app.loggers = vec![crate::model::Logger {
            name: "com.example".into(),
            configured_level: Some("INFO".into()),
            effective_level: "INFO".into(),
        }];

        app.set_logger_level("com.example", "DEBUG").await.unwrap();
        assert_eq!(app.loggers[0].configured_level, Some("DEBUG".into()));
        assert_eq!(app.loggers[0].effective_level, "DEBUG");
    }

    #[tokio::test]
    async fn set_logger_level_off_clears_configured() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/actuator/loggers/com.example"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let mut app = test_app_with_url(&server.uri());
        app.loggers = vec![crate::model::Logger {
            name: "com.example".into(),
            configured_level: Some("DEBUG".into()),
            effective_level: "DEBUG".into(),
        }];

        app.set_logger_level("com.example", "OFF").await.unwrap();
        assert_eq!(app.loggers[0].configured_level, None);
    }

    // -- fetch_mappings --

    #[tokio::test]
    async fn fetch_mappings_parses_dispatcher_servlets() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/actuator/mappings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "contexts": {
                    "application": {
                        "mappings": {
                            "dispatcherServlets": {
                                "dispatcherServlet": [
                                    {
                                        "predicate": "{GET /api/users}",
                                        "handler": "UserController#list()"
                                    },
                                    {
                                        "predicate": "{POST /api/users}",
                                        "handler": "UserController#create()"
                                    }
                                ]
                            }
                        }
                    }
                }
            })))
            .mount(&server)
            .await;

        let mut app = test_app_with_url(&server.uri());
        app.fetch_mappings().await.unwrap();
        assert_eq!(app.mappings.len(), 2);
        assert!(app.mappings.iter().any(|m| m.handler.contains("list")));
    }

    // -- fetch_env --

    #[tokio::test]
    async fn fetch_env_parses_property_sources() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/actuator/env"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "propertySources": [{
                    "name": "application.properties",
                    "properties": {
                        "server.port": {"value": "8080"},
                        "spring.application.name": {"value": "demo"}
                    }
                }]
            })))
            .mount(&server)
            .await;

        let mut app = test_app_with_url(&server.uri());
        app.fetch_env().await.unwrap();
        assert_eq!(app.env_props.len(), 2);
        assert!(app
            .env_props
            .iter()
            .any(|p| p.name == "server.port" && p.value == "8080"));
        assert!(app
            .env_props
            .iter()
            .any(|p| p.source == "application.properties"));
    }

    // -- fetch_app_pid --

    #[tokio::test]
    async fn fetch_app_pid_returns_pid() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/actuator/env/PID"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "property": {"value": "12345"}
            })))
            .mount(&server)
            .await;

        let app = test_app_with_url(&server.uri());
        let pid = app.fetch_app_pid().await.unwrap();
        assert_eq!(pid, "12345");
    }

    #[tokio::test]
    async fn fetch_app_pid_masked_returns_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/actuator/env/PID"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "property": {"value": "******"}
            })))
            .mount(&server)
            .await;

        let app = test_app_with_url(&server.uri());
        let result = app.fetch_app_pid().await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("management.endpoint.env.show-values"));
    }

    // -- fetch_dashboard (partial — test that it doesn't panic on partial data) --

    #[tokio::test]
    async fn fetch_dashboard_with_health_and_metrics() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/actuator/health"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "UP",
                "components": {
                    "diskSpace": {"status": "UP", "details": {"free": 50000000000_i64}},
                    "db": {"status": "UP"}
                }
            })))
            .mount(&server)
            .await;

        // Metrics — just a few key ones
        for (metric, value) in [
            ("jvm.threads.live", 42.0),
            ("jvm.threads.peak", 58.0),
            ("jvm.threads.daemon", 38.0),
            ("system.cpu.usage", 0.23),
            ("process.cpu.usage", 0.08),
            ("process.uptime", 86400.0),
        ] {
            Mock::given(method("GET"))
                .and(path(format!("/actuator/metrics/{}", metric)))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "measurements": [{"statistic": "VALUE", "value": value}]
                })))
                .mount(&server)
                .await;
        }

        // Return 404 for metrics we don't mock (dashboard should handle gracefully)
        Mock::given(method("GET"))
            .and(path("/actuator/info"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/actuator/env"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "activeProfiles": ["dev", "local"]
            })))
            .mount(&server)
            .await;

        let mut app = test_app_with_url(&server.uri());
        app.fetch_dashboard().await.unwrap();

        assert_eq!(app.dashboard.app_status, "UP");
        assert_eq!(app.dashboard.health_components.len(), 2);
        assert_eq!(app.dashboard.threads_live, 42);
        assert_eq!(app.dashboard.threads_peak, 58);
        assert!((app.dashboard.cpu_system - 23.0).abs() < 0.1);
        assert!((app.dashboard.uptime_seconds - 86400.0).abs() < 0.1);
        assert_eq!(app.dashboard.active_profiles, vec!["dev", "local"]);
    }

    // -- fetch_dashboard: info with java/build versions --

    #[tokio::test]
    async fn fetch_dashboard_info_java_and_build_version() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/actuator/health"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "UP"
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/actuator/info"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "java": {"version": "21.0.2"},
                "build": {"version": "3.4.0"}
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/actuator/env"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        let mut app = test_app_with_url(&server.uri());
        app.fetch_dashboard().await.unwrap();

        assert_eq!(app.dashboard.java_version, "21.0.2");
        assert_eq!(app.dashboard.spring_boot_version, "3.4.0");
    }

    // -- fetch_dashboard: GC pause parsing --

    #[tokio::test]
    async fn fetch_dashboard_gc_pause() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/actuator/health"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"status":"UP"})),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/actuator/info"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/actuator/env"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/actuator/metrics/jvm.gc.pause"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "measurements": [
                    {"statistic": "COUNT", "value": 15.0},
                    {"statistic": "TOTAL_TIME", "value": 0.5},
                    {"statistic": "MAX", "value": 0.05}
                ]
            })))
            .mount(&server)
            .await;

        let mut app = test_app_with_url(&server.uri());
        app.fetch_dashboard().await.unwrap();

        assert_eq!(app.dashboard.gc_pause_count, 15);
        assert!((app.dashboard.gc_pause_total_ms - 500.0).abs() < 0.1);
    }

    // -- fetch_dashboard: disk metrics --

    #[tokio::test]
    async fn fetch_dashboard_disk_metrics() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/actuator/health"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"status":"UP"})),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/actuator/info"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/actuator/env"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/actuator/metrics/disk.free"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "measurements": [{"statistic": "VALUE", "value": 53687091200.0}]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/actuator/metrics/disk.total"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "measurements": [{"statistic": "VALUE", "value": 107374182400.0}]
            })))
            .mount(&server)
            .await;

        let mut app = test_app_with_url(&server.uri());
        app.fetch_dashboard().await.unwrap();

        assert!((app.dashboard.disk_free_gb - 50.0).abs() < 0.5);
        assert!((app.dashboard.disk_total_gb - 100.0).abs() < 0.5);
    }

    // -- fetch_dashboard: health component details --

    #[tokio::test]
    async fn fetch_dashboard_health_component_details() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/actuator/health"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "UP",
                "components": {
                    "diskSpace": {
                        "status": "UP",
                        "details": {"free": 50000000000_i64, "total": 100000000000_i64}
                    },
                    "db": {"status": "DOWN"}
                }
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/actuator/info"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/actuator/env"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        let mut app = test_app_with_url(&server.uri());
        app.fetch_dashboard().await.unwrap();

        assert_eq!(app.dashboard.health_components.len(), 2);
        let disk = app
            .dashboard
            .health_components
            .iter()
            .find(|c| c.name == "diskSpace")
            .unwrap();
        assert_eq!(disk.status, "UP");
        assert!(disk.details.contains("free"));
        let db = app
            .dashboard
            .health_components
            .iter()
            .find(|c| c.name == "db")
            .unwrap();
        assert_eq!(db.status, "DOWN");
    }

    // -- fetch_dashboard with no active app --

    #[tokio::test]
    async fn fetch_dashboard_no_active_app() {
        let mut app = test_app();
        app.apps.clear();
        app.config.active_app_url = None;
        let result = app.fetch_dashboard().await;
        assert!(result.is_err());
    }

    // -- set_logger_level failure --

    #[tokio::test]
    async fn set_logger_level_http_failure() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/actuator/loggers/com.example"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let mut app = test_app_with_url(&server.uri());
        app.loggers = vec![crate::model::Logger {
            name: "com.example".into(),
            configured_level: Some("INFO".into()),
            effective_level: "INFO".into(),
        }];

        let result = app.set_logger_level("com.example", "DEBUG").await;
        assert!(result.is_err());
        // Local state should NOT be updated on failure
        assert_eq!(app.loggers[0].configured_level, Some("INFO".into()));
    }

    // -- fetch endpoints/beans/loggers/mappings/env: no active app --

    #[tokio::test]
    async fn fetch_endpoints_no_active_app() {
        let mut app = test_app();
        app.apps.clear();
        app.config.active_app_url = None;
        assert!(app.fetch_endpoints().await.is_err());
    }

    #[tokio::test]
    async fn fetch_beans_no_active_app() {
        let mut app = test_app();
        app.apps.clear();
        app.config.active_app_url = None;
        assert!(app.fetch_beans().await.is_err());
    }

    #[tokio::test]
    async fn fetch_loggers_no_active_app() {
        let mut app = test_app();
        app.apps.clear();
        app.config.active_app_url = None;
        assert!(app.fetch_loggers().await.is_err());
    }

    #[tokio::test]
    async fn fetch_mappings_no_active_app() {
        let mut app = test_app();
        app.apps.clear();
        app.config.active_app_url = None;
        assert!(app.fetch_mappings().await.is_err());
    }

    #[tokio::test]
    async fn fetch_env_no_active_app() {
        let mut app = test_app();
        app.apps.clear();
        app.config.active_app_url = None;
        assert!(app.fetch_env().await.is_err());
    }

    #[tokio::test]
    async fn fetch_app_pid_no_active_app() {
        let mut app = test_app();
        app.apps.clear();
        app.config.active_app_url = None;
        assert!(app.fetch_app_pid().await.is_err());
    }

    // -- fetch_and_save_thread_dump error cases --

    #[tokio::test]
    async fn fetch_thread_dump_404() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/actuator/threaddump"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let mut app = test_app_with_url(&server.uri());
        let result = app.fetch_and_save_thread_dump().await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found") || err.contains("not available"));
    }

    #[tokio::test]
    async fn fetch_thread_dump_500() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/actuator/threaddump"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let mut app = test_app_with_url(&server.uri());
        let result = app.fetch_and_save_thread_dump().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn fetch_thread_dump_no_active_app() {
        let mut app = test_app();
        app.apps.clear();
        app.config.active_app_url = None;
        assert!(app.fetch_and_save_thread_dump().await.is_err());
    }

    // -- download_heap_dump error cases --

    #[tokio::test]
    async fn download_heap_dump_404() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/actuator/heapdump"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let mut app = test_app_with_url(&server.uri());
        let result = app.download_heap_dump().await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found") || err.contains("not available"));
    }

    #[tokio::test]
    async fn download_heap_dump_500() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/actuator/heapdump"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let mut app = test_app_with_url(&server.uri());
        let result = app.download_heap_dump().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn download_heap_dump_no_active_app() {
        let mut app = test_app();
        app.apps.clear();
        app.config.active_app_url = None;
        assert!(app.download_heap_dump().await.is_err());
    }

    // -- filtered_indices: remaining resources --

    #[test]
    fn filtered_indices_mappings() {
        let mut app = test_app();
        app.active_resource = "mappings".into();
        app.filter_text = "users".into();
        let indices = app.filtered_indices();
        assert_eq!(indices, vec![0]);
    }

    #[test]
    fn filtered_indices_env() {
        let mut app = test_app();
        app.active_resource = "env".into();
        app.filter_text = "port".into();
        let indices = app.filtered_indices();
        assert_eq!(indices, vec![0]);
    }

    #[test]
    fn filtered_indices_threaddump() {
        let mut app = test_app();
        app.active_resource = "threaddump".into();
        app.saved_thread_dumps = vec![crate::model::SavedDump {
            app_url: "http://localhost:8080".into(),
            app_name: "my-app".into(),
            path: "/tmp/threaddump_20240101.json".into(),
            timestamp: "20240101_120000".into(),
            size_bytes: 1024,
        }];
        app.filter_text = "my-app".into();
        let indices = app.filtered_indices();
        assert_eq!(indices, vec![0]);

        app.filter_text = "nonexistent".into();
        let indices = app.filtered_indices();
        assert!(indices.is_empty());
    }

    #[test]
    fn filtered_indices_heapdump() {
        let mut app = test_app();
        app.active_resource = "heapdump".into();
        app.saved_heap_dumps = vec![crate::model::SavedDump {
            app_url: "http://localhost:8080".into(),
            app_name: "my-app".into(),
            path: "/tmp/heapdump_20240101.hprof".into(),
            timestamp: "20240101_120000".into(),
            size_bytes: 1048576,
        }];
        app.filter_text = "hprof".into();
        let indices = app.filtered_indices();
        assert_eq!(indices, vec![0]);
    }

    // -- thread_dump_to_jvm_text: file without line number --

    #[test]
    fn thread_dump_to_text_file_no_line_number() {
        let body = serde_json::json!({
            "threads": [{
                "threadName": "t1",
                "threadId": 1,
                "threadState": "RUNNABLE",
                "daemon": false,
                "stackTrace": [{
                    "className": "com.example.Foo",
                    "methodName": "bar",
                    "fileName": "Foo.java",
                    "lineNumber": -1,
                    "nativeMethod": false
                }]
            }]
        });
        let text = App::thread_dump_to_jvm_text(&body);
        assert!(text.contains("at com.example.Foo.bar(Foo.java)"));
    }

    // -- fetch_mappings with servlet filters (direct array format) --

    #[tokio::test]
    async fn fetch_mappings_with_servlet_filters() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/actuator/mappings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "contexts": {
                    "application": {
                        "mappings": {
                            "servletFilters": [
                                {
                                    "name": "characterEncodingFilter",
                                    "className": "org.springframework.web.filter.CharacterEncodingFilter"
                                }
                            ]
                        }
                    }
                }
            })))
            .mount(&server)
            .await;

        let mut app = test_app_with_url(&server.uri());
        app.fetch_mappings().await.unwrap();
        assert_eq!(app.mappings.len(), 1);
        assert!(app.mappings[0].handler.contains("characterEncodingFilter"));
    }

    // -- fetch_beans empty contexts --

    #[tokio::test]
    async fn fetch_beans_empty_contexts() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/actuator/beans"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "contexts": {}
            })))
            .mount(&server)
            .await;

        let mut app = test_app_with_url(&server.uri());
        app.fetch_beans().await.unwrap();
        assert!(app.beans.is_empty());
    }

    // -- fetch_loggers with configured level --

    #[tokio::test]
    async fn fetch_loggers_with_configured_level() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/actuator/loggers"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "loggers": {
                    "ROOT": {
                        "configuredLevel": "WARN",
                        "effectiveLevel": "WARN"
                    }
                }
            })))
            .mount(&server)
            .await;

        let mut app = test_app_with_url(&server.uri());
        app.fetch_loggers().await.unwrap();
        assert_eq!(app.loggers.len(), 1);
        assert_eq!(app.loggers[0].name, "ROOT");
        assert_eq!(app.loggers[0].configured_level, Some("WARN".into()));
    }

    // -- fetch_and_save_thread_dump success --

    #[tokio::test]
    async fn fetch_thread_dump_success_saves_files() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/actuator/threaddump"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "threads": [{
                    "threadName": "main",
                    "threadId": 1,
                    "threadState": "RUNNABLE",
                    "daemon": false,
                    "stackTrace": [{
                        "className": "com.example.Main",
                        "methodName": "run",
                        "fileName": "Main.java",
                        "lineNumber": 10,
                        "nativeMethod": false
                    }]
                }]
            })))
            .mount(&server)
            .await;

        let mut app = test_app_with_url(&server.uri());
        let result = app.fetch_and_save_thread_dump().await;
        assert!(result.is_ok());

        let path_str = result.unwrap();
        assert!(path_str.ends_with(".json"));
        assert!(std::path::Path::new(&path_str).exists());

        // Check .tdump companion file exists
        let tdump_path = path_str.replace(".json", ".tdump");
        assert!(std::path::Path::new(&tdump_path).exists());

        // Verify saved_thread_dumps was updated
        assert_eq!(app.saved_thread_dumps.len(), 1);
        assert_eq!(app.saved_thread_dumps[0].app_name, "mock-app");

        // Cleanup
        let _ = std::fs::remove_file(&path_str);
        let _ = std::fs::remove_file(&tdump_path);
    }

    // -- download_heap_dump success --

    #[tokio::test]
    async fn download_heap_dump_success_saves_file() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/actuator/heapdump"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![0xCA, 0xFE, 0xBA, 0xBE]))
            .mount(&server)
            .await;

        let mut app = test_app_with_url(&server.uri());
        let result = app.download_heap_dump().await;
        assert!(result.is_ok());

        let path_str = result.unwrap();
        assert!(path_str.ends_with(".hprof"));
        assert!(std::path::Path::new(&path_str).exists());

        // Verify saved_heap_dumps was updated
        assert_eq!(app.saved_heap_dumps.len(), 1);
        assert_eq!(app.saved_heap_dumps[0].size_bytes, 4);

        // Cleanup
        let _ = std::fs::remove_file(&path_str);
    }

    // -- scan_saved_dumps --

    #[test]
    fn scan_saved_dumps_finds_files() {
        let mut app = test_app();
        app.config.active_app_url = Some("http://localhost:8080".into());
        app.apps = vec![crate::model::SpringApp {
            name: "test-scan-app".into(),
            url: "http://localhost:8080".into(),
            status: crate::model::AppStatus::Up,
        }];

        // Create temp dump files
        let dir = App::app_dumps_dir("test-scan-app").unwrap();
        let td_path = dir.join("threaddump_20240101_120000.json");
        let hd_path = dir.join("heapdump_20240101_120000.hprof");
        std::fs::write(&td_path, "{}").unwrap();
        std::fs::write(&hd_path, &[0u8; 64]).unwrap();

        app.scan_saved_dumps();

        assert_eq!(app.saved_thread_dumps.len(), 1);
        assert_eq!(app.saved_thread_dumps[0].timestamp, "20240101_120000");
        assert_eq!(app.saved_heap_dumps.len(), 1);
        assert_eq!(app.saved_heap_dumps[0].timestamp, "20240101_120000");

        // Cleanup
        let _ = std::fs::remove_file(&td_path);
        let _ = std::fs::remove_file(&hd_path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn scan_saved_dumps_sorts_newest_first() {
        let mut app = test_app();
        app.config.active_app_url = Some("http://localhost:8080".into());
        app.apps = vec![crate::model::SpringApp {
            name: "test-sort-app".into(),
            url: "http://localhost:8080".into(),
            status: crate::model::AppStatus::Up,
        }];

        let dir = App::app_dumps_dir("test-sort-app").unwrap();
        let old = dir.join("threaddump_20240101_100000.json");
        let new = dir.join("threaddump_20240202_120000.json");
        std::fs::write(&old, "{}").unwrap();
        std::fs::write(&new, "{}").unwrap();

        app.scan_saved_dumps();

        assert_eq!(app.saved_thread_dumps.len(), 2);
        assert_eq!(app.saved_thread_dumps[0].timestamp, "20240202_120000"); // newest first

        let _ = std::fs::remove_file(&old);
        let _ = std::fs::remove_file(&new);
        let _ = std::fs::remove_dir(&dir);
    }

    // -- HTTP request metrics in dashboard --

    #[tokio::test]
    async fn fetch_dashboard_http_request_metrics() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/actuator/health"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"status":"UP"})),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/actuator/info"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/actuator/env"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/actuator/metrics/http.server.requests"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "measurements": [
                    {"statistic": "COUNT", "value": 5000.0},
                    {"statistic": "TOTAL_TIME", "value": 120.0}
                ]
            })))
            .mount(&server)
            .await;

        let mut app = test_app_with_url(&server.uri());
        app.fetch_dashboard().await.unwrap();

        assert_eq!(app.dashboard.http_total_count, 5000);
        assert!((app.dashboard.http_total_time_s - 120.0).abs() < 0.1);
    }

    // -- set_logger_level no active app --

    #[tokio::test]
    async fn set_logger_level_no_active_app() {
        let mut app = test_app();
        app.apps.clear();
        app.config.active_app_url = None;
        assert!(app.set_logger_level("com.example", "DEBUG").await.is_err());
    }
}
