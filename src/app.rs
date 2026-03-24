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
}

// ---------------------------------------------------------------------------
// ResourceItem (command-palette entry)
// ---------------------------------------------------------------------------

/// A single entry in the command palette / resource list.
#[derive(Debug, Clone)]
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
    pub fn apply_metadata_defaults(&mut self, meta: &crate::model::InitializrMetadata) {
        self.group_id = meta.group_id_default.clone();
        self.artifact_id = meta.artifact_id_default.clone();
        self.name = meta.name_default.clone();
        self.description = meta.description_default.clone();
        self.package_name = meta.package_name_default.clone();

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
    pub error_message: Option<String>,
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

            error_message: None,
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
                        let file = frame
                            .get("fileName")
                            .and_then(|f| f.as_str());
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

    /// Called on every tick of the main event loop. Advances the spinner.
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
