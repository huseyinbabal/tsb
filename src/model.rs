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
