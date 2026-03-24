use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use minijinja::{context, Environment};
use serde::Deserialize;

use crate::model::NewProjectParams;

// ---------------------------------------------------------------------------
// Embedded templates
// ---------------------------------------------------------------------------

const TPL_POM: &str = include_str!("../resources/templates/maven/pom.xml");
const TPL_MAVEN_WRAPPER: &str =
    include_str!("../resources/templates/maven/maven-wrapper.properties");

const TPL_BUILD_GRADLE: &str = include_str!("../resources/templates/gradle/build.gradle");
const TPL_SETTINGS_GRADLE: &str = include_str!("../resources/templates/gradle/settings.gradle");

const TPL_BUILD_GRADLE_KTS: &str =
    include_str!("../resources/templates/gradle-kotlin/build.gradle.kts");
const TPL_SETTINGS_GRADLE_KTS: &str =
    include_str!("../resources/templates/gradle-kotlin/settings.gradle.kts");

const TPL_APP_JAVA: &str = include_str!("../resources/templates/common/Application.java");
const TPL_APP_TEST_JAVA: &str = include_str!("../resources/templates/common/ApplicationTests.java");
const TPL_APP_KT: &str = include_str!("../resources/templates/common/Application.kt");
const TPL_APP_TEST_KT: &str = include_str!("../resources/templates/common/ApplicationTests.kt");
const TPL_APP_PROPS: &str = include_str!("../resources/templates/common/application.properties");
const TPL_GITIGNORE_MAVEN: &str = include_str!("../resources/templates/common/gitignore-maven");
const TPL_GITIGNORE_GRADLE: &str = include_str!("../resources/templates/common/gitignore-gradle");
const TPL_GITATTRIBUTES_MAVEN: &str =
    include_str!("../resources/templates/common/gitattributes-maven");

const EMBEDDED_DEPENDENCIES: &str = include_str!("../resources/dependencies.json");

// ---------------------------------------------------------------------------
// Dependency resolution types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RawDep {
    #[serde(rename = "groupId")]
    group_id: String,
    #[serde(rename = "artifactId")]
    artifact_id: String,
    scope: Option<String>,
    version: Option<String>,
    bom: Option<String>,
    repository: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawBom {
    #[serde(rename = "groupId")]
    group_id: String,
    #[serde(rename = "artifactId")]
    artifact_id: String,
    version: String,
}

#[derive(Debug, Deserialize)]
struct RawRepo {
    name: String,
    url: String,
    #[serde(rename = "snapshotEnabled")]
    snapshot_enabled: bool,
}

#[derive(Debug, Deserialize)]
struct DepsRoot {
    dependencies: HashMap<String, RawDep>,
    boms: HashMap<String, RawBom>,
    repositories: HashMap<String, RawRepo>,
}

// ---------------------------------------------------------------------------
// Template context types (serializable for minijinja)
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Serialize)]
struct DepCtx {
    group_id: String,
    artifact_id: String,
    version: String, // empty string if managed by parent/bom
}

#[derive(Debug, serde::Serialize)]
struct BomCtx {
    group_id: String,
    artifact_id: String,
    version: String,
}

#[derive(Debug, serde::Serialize)]
struct RepoCtx {
    id: String,
    name: String,
    url: String,
    snapshot_enabled: String, // "true"/"false" for xml
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Generate a Spring Boot project locally using embedded templates.
/// Returns the path to the generated project directory.
pub fn generate_project(params: &NewProjectParams) -> Result<String> {
    // Load dependency catalog
    let deps_path = crate::config::TsbConfig::config_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("dependencies.json");

    let deps_json: String = if deps_path.exists() {
        std::fs::read_to_string(&deps_path).unwrap_or_else(|_| EMBEDDED_DEPENDENCIES.to_string())
    } else {
        EMBEDDED_DEPENDENCIES.to_string()
    };

    let catalog: DepsRoot =
        serde_json::from_str(&deps_json).context("failed to parse dependencies.json")?;

    // Resolve selected dependencies
    let mut compile_deps = Vec::new();
    let mut runtime_deps = Vec::new();
    let mut test_deps = Vec::new();
    let mut annotation_processor_deps = Vec::new();
    let mut needed_boms: HashMap<String, &RawBom> = HashMap::new();
    let mut needed_repos: HashMap<String, &RawRepo> = HashMap::new();
    let extra_properties: Vec<(String, String)> = Vec::new();
    let mut has_jpa = false;

    for dep_id in &params.dependencies {
        if let Some(raw) = catalog.dependencies.get(dep_id.as_str()) {
            // Check if this is a JPA-related dependency
            if dep_id.contains("jpa") || dep_id.contains("data-jpa") {
                has_jpa = true;
            }

            let dep = DepCtx {
                group_id: raw.group_id.clone(),
                artifact_id: raw.artifact_id.clone(),
                version: if raw.bom.is_some() {
                    String::new() // managed by BOM
                } else {
                    raw.version.clone().unwrap_or_default()
                },
            };

            match raw.scope.as_deref() {
                Some("runtime") => runtime_deps.push(dep),
                Some("test") => test_deps.push(dep),
                Some("annotationProcessor") => annotation_processor_deps.push(dep),
                _ => compile_deps.push(dep),
            }

            // Collect BOMs
            if let Some(bom_key) = &raw.bom {
                if let Some(bom) = catalog.boms.get(bom_key.as_str()) {
                    needed_boms.insert(bom_key.clone(), bom);
                }
            }

            // Collect repositories
            if let Some(repo_key) = &raw.repository {
                if let Some(repo) = catalog.repositories.get(repo_key.as_str()) {
                    needed_repos.insert(repo_key.clone(), repo);
                }
            }
        }
    }

    // Always add spring-boot-starter-test as a test dependency
    test_deps.push(DepCtx {
        group_id: "org.springframework.boot".to_string(),
        artifact_id: "spring-boot-starter-test".to_string(),
        version: String::new(),
    });

    let boms: Vec<BomCtx> = needed_boms
        .values()
        .map(|b| BomCtx {
            group_id: b.group_id.clone(),
            artifact_id: b.artifact_id.clone(),
            version: b.version.clone(),
        })
        .collect();

    let repositories: Vec<RepoCtx> = needed_repos
        .iter()
        .map(|(id, r)| RepoCtx {
            id: id.clone(),
            name: r.name.clone(),
            url: r.url.clone(),
            snapshot_enabled: r.snapshot_enabled.to_string(),
        })
        .collect();

    // Determine output directory
    let output_base = if params.output_dir.is_empty() || params.output_dir == "." {
        std::env::current_dir().context("failed to get current directory")?
    } else {
        PathBuf::from(&params.output_dir)
    };

    let project_dir = output_base.join(&params.artifact_id);
    std::fs::create_dir_all(&project_dir)
        .with_context(|| format!("failed to create directory {}", project_dir.display()))?;

    // Derive class name: prefer name, fall back to artifact_id
    let raw_name = if params.name.is_empty() {
        &params.artifact_id
    } else {
        &params.name
    };
    let application_name = {
        let pascal = to_pascal_case(raw_name);
        if pascal.is_empty() {
            "Application".to_string()
        } else {
            pascal
        }
    };
    let is_kotlin = params.language == "kotlin";
    let is_maven = params.project_type == "maven-project";
    let is_gradle_kotlin = params.project_type == "gradle-project-kotlin";

    // Source directory structure
    let lang_dir = if is_kotlin { "kotlin" } else { "java" };
    let pkg_path = params.package_name.replace('.', "/");

    let main_src = format!("src/main/{}/{}", lang_dir, pkg_path);
    let test_src = format!("src/test/{}/{}", lang_dir, pkg_path);

    std::fs::create_dir_all(project_dir.join(&main_src))?;
    std::fs::create_dir_all(project_dir.join(&test_src))?;
    std::fs::create_dir_all(project_dir.join("src/main/resources/static"))?;
    std::fs::create_dir_all(project_dir.join("src/main/resources/templates"))?;

    // Setup minijinja environment
    let mut env = Environment::new();
    env.add_template("pom.xml", TPL_POM)?;
    env.add_template("build.gradle", TPL_BUILD_GRADLE)?;
    env.add_template("settings.gradle", TPL_SETTINGS_GRADLE)?;
    env.add_template("build.gradle.kts", TPL_BUILD_GRADLE_KTS)?;
    env.add_template("settings.gradle.kts", TPL_SETTINGS_GRADLE_KTS)?;
    env.add_template("Application.java", TPL_APP_JAVA)?;
    env.add_template("ApplicationTests.java", TPL_APP_TEST_JAVA)?;
    env.add_template("Application.kt", TPL_APP_KT)?;
    env.add_template("ApplicationTests.kt", TPL_APP_TEST_KT)?;
    env.add_template("application.properties", TPL_APP_PROPS)?;

    let boot_version = normalize_boot_version(&params.boot_version);

    let ctx = context! {
        boot_version => boot_version,
        group_id => params.group_id,
        artifact_id => params.artifact_id,
        name => params.name,
        description => params.description,
        package_name => params.package_name,
        java_version => params.java_version,
        application_name => application_name,
        compile_deps => compile_deps,
        runtime_deps => runtime_deps,
        test_deps => test_deps,
        annotation_processor_deps => annotation_processor_deps,
        boms => boms,
        repositories => repositories,
        extra_properties => extra_properties,
        has_jpa => has_jpa,
        kotlin_version => "2.2.21",
    };

    // Write build file
    if is_maven {
        write_template(&env, "pom.xml", &ctx, &project_dir.join("pom.xml"))?;

        // Maven wrapper
        let mvn_dir = project_dir.join(".mvn/wrapper");
        std::fs::create_dir_all(&mvn_dir)?;
        std::fs::write(mvn_dir.join("maven-wrapper.properties"), TPL_MAVEN_WRAPPER)?;

        // .gitignore, .gitattributes
        std::fs::write(project_dir.join(".gitignore"), TPL_GITIGNORE_MAVEN)?;
        std::fs::write(project_dir.join(".gitattributes"), TPL_GITATTRIBUTES_MAVEN)?;
    } else if is_gradle_kotlin {
        write_template(
            &env,
            "build.gradle.kts",
            &ctx,
            &project_dir.join("build.gradle.kts"),
        )?;
        write_template(
            &env,
            "settings.gradle.kts",
            &ctx,
            &project_dir.join("settings.gradle.kts"),
        )?;
        std::fs::write(project_dir.join(".gitignore"), TPL_GITIGNORE_GRADLE)?;
    } else {
        // gradle-project (Groovy)
        write_template(
            &env,
            "build.gradle",
            &ctx,
            &project_dir.join("build.gradle"),
        )?;
        write_template(
            &env,
            "settings.gradle",
            &ctx,
            &project_dir.join("settings.gradle"),
        )?;
        std::fs::write(project_dir.join(".gitignore"), TPL_GITIGNORE_GRADLE)?;
    }

    // Write source files
    if is_kotlin {
        let app_file = format!("{}/{}.kt", main_src, application_name);
        let test_file = format!("{}/{}Tests.kt", test_src, application_name);
        write_template(&env, "Application.kt", &ctx, &project_dir.join(app_file))?;
        write_template(
            &env,
            "ApplicationTests.kt",
            &ctx,
            &project_dir.join(test_file),
        )?;
    } else {
        let app_file = format!("{}/{}.java", main_src, application_name);
        let test_file = format!("{}/{}Tests.java", test_src, application_name);
        write_template(&env, "Application.java", &ctx, &project_dir.join(app_file))?;
        write_template(
            &env,
            "ApplicationTests.java",
            &ctx,
            &project_dir.join(test_file),
        )?;
    }

    // application.properties
    write_template(
        &env,
        "application.properties",
        &ctx,
        &project_dir.join("src/main/resources/application.properties"),
    )?;

    Ok(project_dir.to_string_lossy().to_string())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn write_template(
    env: &Environment,
    tpl_name: &str,
    ctx: &minijinja::Value,
    dest: &Path,
) -> Result<()> {
    let tmpl = env.get_template(tpl_name)?;
    let rendered = tmpl.render(ctx)?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(dest, rendered)?;
    Ok(())
}

fn to_pascal_case(s: &str) -> String {
    s.split(['-', '_', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().to_string() + &chars.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect()
}

fn normalize_boot_version(raw: &str) -> String {
    if let Some(base) = raw.strip_suffix(".RELEASE") {
        return base.to_string();
    }
    if raw.ends_with("-SNAPSHOT") {
        return raw.to_string();
    }
    if let Some(base) = raw.strip_suffix(".BUILD-SNAPSHOT") {
        return format!("{}-SNAPSHOT", base);
    }
    if let Some(dot_pos) = raw.rfind('.') {
        let suffix = &raw[dot_pos + 1..];
        if suffix.starts_with('M') || suffix.starts_with("RC") {
            let base = &raw[..dot_pos];
            return format!("{}-{}", base, suffix);
        }
    }
    raw.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::NewProjectParams;

    // -- to_pascal_case --

    #[test]
    fn pascal_case_simple() {
        assert_eq!(to_pascal_case("demo"), "Demo");
    }

    #[test]
    fn pascal_case_hyphenated() {
        assert_eq!(to_pascal_case("my-demo-app"), "MyDemoApp");
    }

    #[test]
    fn pascal_case_underscored() {
        assert_eq!(to_pascal_case("my_demo_app"), "MyDemoApp");
    }

    #[test]
    fn pascal_case_spaces() {
        assert_eq!(to_pascal_case("my demo app"), "MyDemoApp");
    }

    #[test]
    fn pascal_case_mixed_separators() {
        assert_eq!(to_pascal_case("my-demo_app name"), "MyDemoAppName");
    }

    #[test]
    fn pascal_case_uppercase_input() {
        assert_eq!(to_pascal_case("UPPER-CASE"), "UpperCase");
    }

    #[test]
    fn pascal_case_empty() {
        assert_eq!(to_pascal_case(""), "");
    }

    #[test]
    fn pascal_case_leading_trailing_separators() {
        assert_eq!(to_pascal_case("--leading--"), "Leading");
    }

    #[test]
    fn pascal_case_single_char() {
        assert_eq!(to_pascal_case("a"), "A");
    }

    // -- normalize_boot_version --

    #[test]
    fn normalize_release_suffix() {
        assert_eq!(normalize_boot_version("3.2.0.RELEASE"), "3.2.0");
    }

    #[test]
    fn normalize_snapshot_unchanged() {
        assert_eq!(normalize_boot_version("3.3.0-SNAPSHOT"), "3.3.0-SNAPSHOT");
    }

    #[test]
    fn normalize_build_snapshot_passes_through() {
        // ".BUILD-SNAPSHOT" ends with "-SNAPSHOT", so the second branch catches it
        assert_eq!(
            normalize_boot_version("3.2.0.BUILD-SNAPSHOT"),
            "3.2.0.BUILD-SNAPSHOT"
        );
    }

    #[test]
    fn normalize_milestone() {
        assert_eq!(normalize_boot_version("3.3.0.M1"), "3.3.0-M1");
    }

    #[test]
    fn normalize_release_candidate() {
        assert_eq!(normalize_boot_version("3.3.0.RC1"), "3.3.0-RC1");
    }

    #[test]
    fn normalize_plain_version() {
        assert_eq!(normalize_boot_version("3.2.0"), "3.2.0");
    }

    // -- generate_project (integration test with temp dir) --

    #[test]
    fn generate_maven_java_project() {
        let tmp = std::env::temp_dir().join("tsb_test_gen_maven");
        let _ = std::fs::remove_dir_all(&tmp);

        let params = NewProjectParams {
            boot_version: "3.4.5".into(),
            language: "java".into(),
            packaging: "jar".into(),
            java_version: "21".into(),
            project_type: "maven-project".into(),
            group_id: "com.test".into(),
            artifact_id: "myapp".into(),
            version: "0.0.1-SNAPSHOT".into(),
            name: "myapp".into(),
            description: "Test project".into(),
            package_name: "com.test.myapp".into(),
            dependencies: vec!["web".into()],
            output_dir: tmp.to_str().unwrap().into(),
        };

        let result = generate_project(&params);
        assert!(
            result.is_ok(),
            "generate_project failed: {:?}",
            result.err()
        );

        let project_dir = tmp.join("myapp");
        assert!(project_dir.join("pom.xml").exists());
        assert!(project_dir
            .join("src/main/java/com/test/myapp/Myapp.java")
            .exists());
        assert!(project_dir
            .join("src/test/java/com/test/myapp/MyappTests.java")
            .exists());
        assert!(project_dir
            .join("src/main/resources/application.properties")
            .exists());

        // Verify pom.xml contains expected content
        let pom = std::fs::read_to_string(project_dir.join("pom.xml")).unwrap();
        assert!(pom.contains("<groupId>com.test</groupId>"));
        assert!(pom.contains("<artifactId>myapp</artifactId>"));
        assert!(pom.contains("spring-boot-starter-webmvc"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn generate_gradle_kotlin_project() {
        let tmp = std::env::temp_dir().join("tsb_test_gen_gradle_kt");
        let _ = std::fs::remove_dir_all(&tmp);

        let params = NewProjectParams {
            boot_version: "3.4.5".into(),
            language: "kotlin".into(),
            packaging: "jar".into(),
            java_version: "21".into(),
            project_type: "gradle-project-kotlin".into(),
            group_id: "com.test".into(),
            artifact_id: "ktapp".into(),
            version: "0.0.1-SNAPSHOT".into(),
            name: "ktapp".into(),
            description: "Kotlin test".into(),
            package_name: "com.test.ktapp".into(),
            dependencies: vec![],
            output_dir: tmp.to_str().unwrap().into(),
        };

        let result = generate_project(&params);
        assert!(
            result.is_ok(),
            "generate_project failed: {:?}",
            result.err()
        );

        let project_dir = tmp.join("ktapp");
        assert!(project_dir.join("build.gradle.kts").exists());
        assert!(project_dir.join("settings.gradle.kts").exists());
        assert!(project_dir
            .join("src/main/kotlin/com/test/ktapp/Ktapp.kt")
            .exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn generate_gradle_groovy_project() {
        let tmp = std::env::temp_dir().join("tsb_test_gen_gradle_groovy");
        let _ = std::fs::remove_dir_all(&tmp);

        let params = NewProjectParams {
            boot_version: "3.4.5".into(),
            language: "java".into(),
            packaging: "jar".into(),
            java_version: "21".into(),
            project_type: "gradle-project".into(),
            group_id: "com.test".into(),
            artifact_id: "gradleapp".into(),
            version: "0.0.1-SNAPSHOT".into(),
            name: "gradleapp".into(),
            description: "Gradle test".into(),
            package_name: "com.test.gradleapp".into(),
            dependencies: vec![],
            output_dir: tmp.to_str().unwrap().into(),
        };

        let result = generate_project(&params);
        assert!(result.is_ok());

        let project_dir = tmp.join("gradleapp");
        assert!(project_dir.join("build.gradle").exists());
        assert!(project_dir.join("settings.gradle").exists());
        assert!(!project_dir.join("build.gradle.kts").exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn generate_project_with_multiple_dependencies() {
        let tmp = std::env::temp_dir().join("tsb_test_gen_multi_deps");
        let _ = std::fs::remove_dir_all(&tmp);

        let params = NewProjectParams {
            boot_version: "3.4.5".into(),
            language: "java".into(),
            packaging: "jar".into(),
            java_version: "21".into(),
            project_type: "maven-project".into(),
            group_id: "com.test".into(),
            artifact_id: "multidep".into(),
            version: "0.0.1-SNAPSHOT".into(),
            name: "multidep".into(),
            description: "Test".into(),
            package_name: "com.test.multidep".into(),
            dependencies: vec!["web".into(), "actuator".into(), "validation".into()],
            output_dir: tmp.to_str().unwrap().into(),
        };

        let result = generate_project(&params);
        assert!(result.is_ok());

        let pom = std::fs::read_to_string(tmp.join("multidep/pom.xml")).unwrap();
        assert!(pom.contains("spring-boot-starter-webmvc"));
        assert!(pom.contains("spring-boot-starter-actuator"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn generate_project_empty_name_falls_back_to_artifact_id() {
        let tmp = std::env::temp_dir().join("tsb_test_gen_empty_name");
        let _ = std::fs::remove_dir_all(&tmp);

        let params = NewProjectParams {
            boot_version: "3.4.5".into(),
            language: "java".into(),
            packaging: "jar".into(),
            java_version: "21".into(),
            project_type: "maven-project".into(),
            group_id: "com.example".into(),
            artifact_id: "demo".into(),
            version: "0.0.1-SNAPSHOT".into(),
            name: "".into(), // empty name
            description: "Test".into(),
            package_name: "com.example.demo".into(),
            dependencies: vec![],
            output_dir: tmp.to_str().unwrap().into(),
        };

        let result = generate_project(&params);
        assert!(result.is_ok());

        // Should use artifact_id "demo" -> "Demo" as class name
        let project_dir = tmp.join("demo");
        assert!(project_dir
            .join("src/main/java/com/example/demo/Demo.java")
            .exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
