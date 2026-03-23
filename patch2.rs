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
        let data = std::fs::read_to_string(&meta_path).context("failed to read local metadata file")?;
        let body: Value = serde_json::from_str(&data).context("failed to parse local metadata JSON")?;

        let parsed = Self::parse_initializr_metadata(&body)?;
        Ok(parsed)
    }
