use serde::{Deserialize, Serialize};
use std::fmt;

/// Health status of a Spring Boot application.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum AppStatus {
    Up,
    Down,
    #[default]
    Unknown,
}

impl fmt::Display for AppStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppStatus::Up => write!(f, "UP"),
            AppStatus::Down => write!(f, "DOWN"),
            AppStatus::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

/// A Spring Boot application connected via Actuator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpringApp {
    pub name: String,
    pub url: String,
    pub status: AppStatus,
}

/// An Actuator endpoint (e.g. /actuator/health, /actuator/beans).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    pub name: String,
    pub url: String,
}

/// A Spring bean registered in the application context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bean {
    pub name: String,
    pub scope: String,
    #[serde(rename = "type")]
    pub type_name: String,
}

/// A logger with its configured and effective levels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Logger {
    pub name: String,
    pub configured_level: Option<String>,
    pub effective_level: String,
}

/// An HTTP request mapping from Spring MVC/WebFlux.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapping {
    pub pattern: String,
    pub handler: String,
}

/// An environment property with its source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvProperty {
    pub name: String,
    pub value: String,
    pub source: String,
}

/// Basic server/runtime information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub spring_boot_version: Option<String>,
    pub java_version: Option<String>,
}

/// A locally saved dump file (thread dump or heap dump).
#[derive(Debug, Clone)]
pub struct SavedDump {
    pub app_url: String,
    pub app_name: String,
    pub path: String,
    pub timestamp: String,
    pub size_bytes: u64,
}

// ===========================================================================
// Thread visualization
// ===========================================================================

/// A single thread parsed from a thread dump JSON.
#[derive(Debug, Clone)]
pub struct ThreadInfo {
    pub name: String,
    pub id: i64,
    pub state: String,
    pub daemon: bool,
    pub stack_trace: Vec<StackFrame>,
}

/// A single stack frame.
#[derive(Debug, Clone)]
pub struct StackFrame {
    pub class_name: String,
    pub method_name: String,
    pub file_name: String,
    pub line_number: i64,
    pub native_method: bool,
}

// ===========================================================================
// Dashboard data
// ===========================================================================

/// Health component status (e.g. db, diskSpace, redis).
#[derive(Debug, Clone, Default)]
pub struct HealthComponent {
    pub name: String,
    pub status: String,
    pub details: String,
}

/// Dashboard metrics collected from multiple actuator endpoints.
#[derive(Debug, Clone, Default)]
pub struct DashboardData {
    // -- health --
    pub app_status: String,
    pub health_components: Vec<HealthComponent>,

    // -- JVM memory --
    pub heap_used_mb: f64,
    pub heap_max_mb: f64,
    pub nonheap_used_mb: f64,

    // -- threads --
    pub threads_live: u64,
    pub threads_peak: u64,
    pub threads_daemon: u64,

    // -- CPU --
    pub cpu_system: f64,
    pub cpu_process: f64,

    // -- GC --
    pub gc_pause_count: u64,
    pub gc_pause_total_ms: f64,

    // -- HTTP requests --
    pub http_total_count: u64,
    pub http_total_time_s: f64,
    pub http_error_count: u64,

    // -- info --
    pub uptime_seconds: f64,
    pub java_version: String,
    pub spring_boot_version: String,
    pub active_profiles: Vec<String>,

    // -- disk --
    pub disk_free_gb: f64,
    pub disk_total_gb: f64,
}

// ===========================================================================
// Spring Initializr metadata models
// ===========================================================================

/// A selectable option (used for bootVersion, language, packaging, javaVersion, type).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializrOption {
    pub id: String,
    pub name: String,
}

/// A single dependency within a dependency group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializrDependency {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
}

/// A group of related dependencies (e.g. "Web", "SQL", "Security").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializrDependencyGroup {
    pub name: String,
    pub values: Vec<InitializrDependency>,
}

/// A text field with a default value (groupId, artifactId, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct InitializrTextField {
    pub default: String,
}

/// Parsed metadata from the Spring Initializr API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializrMetadata {
    pub boot_versions: Vec<InitializrOption>,
    pub boot_version_default: String,
    pub languages: Vec<InitializrOption>,
    pub language_default: String,
    pub packagings: Vec<InitializrOption>,
    pub packaging_default: String,
    pub java_versions: Vec<InitializrOption>,
    pub java_version_default: String,
    pub project_types: Vec<InitializrOption>,
    pub project_type_default: String,
    pub dependency_groups: Vec<InitializrDependencyGroup>,
    pub group_id_default: String,
    pub artifact_id_default: String,
    pub version_default: String,
    pub name_default: String,
    pub description_default: String,
    pub package_name_default: String,
}

/// Parameters for generating a new Spring Boot project.
#[derive(Debug, Clone)]
pub struct NewProjectParams {
    pub boot_version: String,
    pub language: String,
    #[allow(dead_code)]
    pub packaging: String,
    pub java_version: String,
    pub project_type: String,
    pub group_id: String,
    pub artifact_id: String,
    #[allow(dead_code)]
    pub version: String,
    pub name: String,
    pub description: String,
    pub package_name: String,
    pub dependencies: Vec<String>,
    pub output_dir: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- AppStatus --

    #[test]
    fn app_status_display() {
        assert_eq!(AppStatus::Up.to_string(), "UP");
        assert_eq!(AppStatus::Down.to_string(), "DOWN");
        assert_eq!(AppStatus::Unknown.to_string(), "UNKNOWN");
    }

    #[test]
    fn app_status_default_is_unknown() {
        assert_eq!(AppStatus::default(), AppStatus::Unknown);
    }

    #[test]
    fn app_status_serde_roundtrip() {
        for status in [AppStatus::Up, AppStatus::Down, AppStatus::Unknown] {
            let json = serde_json::to_string(&status).unwrap();
            let parsed: AppStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, status);
        }
    }

    // -- SpringApp serde --

    #[test]
    fn spring_app_serde_roundtrip() {
        let app = SpringApp {
            name: "my-app".into(),
            url: "http://localhost:8080".into(),
            status: AppStatus::Up,
        };
        let json = serde_json::to_string(&app).unwrap();
        let parsed: SpringApp = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "my-app");
        assert_eq!(parsed.url, "http://localhost:8080");
        assert_eq!(parsed.status, AppStatus::Up);
    }

    // -- Bean serde (has rename) --

    #[test]
    fn bean_serde_rename_type() {
        let bean = Bean {
            name: "myBean".into(),
            scope: "singleton".into(),
            type_name: "com.example.MyBean".into(),
        };
        let json = serde_json::to_string(&bean).unwrap();
        assert!(json.contains(r#""type":"com.example.MyBean"#));

        let parsed: Bean = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.type_name, "com.example.MyBean");
    }

    // -- Logger --

    #[test]
    fn logger_serde_with_optional_configured_level() {
        let logger = Logger {
            name: "com.example".into(),
            configured_level: None,
            effective_level: "INFO".into(),
        };
        let json = serde_json::to_string(&logger).unwrap();
        let parsed: Logger = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.configured_level, None);
        assert_eq!(parsed.effective_level, "INFO");

        let logger2 = Logger {
            configured_level: Some("DEBUG".into()),
            ..logger
        };
        let json2 = serde_json::to_string(&logger2).unwrap();
        let parsed2: Logger = serde_json::from_str(&json2).unwrap();
        assert_eq!(parsed2.configured_level, Some("DEBUG".into()));
    }

    // -- DashboardData default --

    #[test]
    fn dashboard_data_default_all_zero() {
        let d = DashboardData::default();
        assert_eq!(d.app_status, "");
        assert!(d.health_components.is_empty());
        assert_eq!(d.heap_used_mb, 0.0);
        assert_eq!(d.threads_live, 0);
        assert_eq!(d.cpu_system, 0.0);
        assert_eq!(d.http_total_count, 0);
        assert_eq!(d.uptime_seconds, 0.0);
        assert!(d.active_profiles.is_empty());
    }

    // -- EnvProperty --

    #[test]
    fn env_property_serde() {
        let prop = EnvProperty {
            name: "server.port".into(),
            value: "8080".into(),
            source: "application.properties".into(),
        };
        let json = serde_json::to_string(&prop).unwrap();
        let parsed: EnvProperty = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "server.port");
        assert_eq!(parsed.value, "8080");
        assert_eq!(parsed.source, "application.properties");
    }

    // -- Mapping --

    #[test]
    fn mapping_serde() {
        let m = Mapping {
            pattern: "/api/users".into(),
            handler: "com.example.UserController#list".into(),
        };
        let json = serde_json::to_string(&m).unwrap();
        let parsed: Mapping = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.pattern, "/api/users");
        assert_eq!(parsed.handler, "com.example.UserController#list");
    }

    // -- InitializrMetadata --

    #[test]
    fn initializr_dependency_default_description() {
        let json = r#"{"id": "web", "name": "Spring Web"}"#;
        let dep: InitializrDependency = serde_json::from_str(json).unwrap();
        assert_eq!(dep.id, "web");
        assert_eq!(dep.description, ""); // default
    }
}
