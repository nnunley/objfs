use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Top-level objfs configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ObjfsConfig {
    #[serde(default)]
    pub remote: RemoteSection,
    #[serde(default)]
    pub worker: WorkerSection,
    #[serde(default)]
    pub project: ProjectSection,
}

impl Default for ObjfsConfig {
    fn default() -> Self {
        Self {
            remote: RemoteSection::default(),
            worker: WorkerSection::default(),
            project: ProjectSection::default(),
        }
    }
}

/// Remote cache/execution settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemoteSection {
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default = "default_instance")]
    pub instance: String,
    #[serde(default = "default_min_remote_size")]
    pub min_remote_size: u64,
}

fn default_instance() -> String {
    "main".to_string()
}

fn default_min_remote_size() -> u64 {
    100_000
}

impl Default for RemoteSection {
    fn default() -> Self {
        Self {
            endpoint: None,
            instance: default_instance(),
            min_remote_size: default_min_remote_size(),
        }
    }
}

/// Local worker settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkerSection {
    #[serde(default = "default_auto_start")]
    pub auto_start: bool,
    #[serde(default)]
    pub targets: Vec<String>,
}

fn default_auto_start() -> bool {
    true
}

impl Default for WorkerSection {
    fn default() -> Self {
        Self {
            auto_start: default_auto_start(),
            targets: Vec::new(),
        }
    }
}

/// Project-specific settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectSection {
    #[serde(rename = "type", default)]
    pub project_type: String,
}

impl Default for ProjectSection {
    fn default() -> Self {
        Self {
            project_type: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

impl ObjfsConfig {
    /// Parse from a TOML string.
    pub fn from_toml_str(s: &str) -> Result<Self> {
        toml::from_str(s).context("failed to parse TOML config")
    }

    /// Parse from a JSON5 string.
    pub fn from_json5_str(s: &str) -> Result<Self> {
        json5::from_str(s).context("failed to parse JSON5 config")
    }

    /// Serialize to a TOML string.
    pub fn to_toml_string(&self) -> Result<String> {
        toml::to_string_pretty(self).context("failed to serialize config to TOML")
    }

    /// Serialize to a JSON5-compatible string (uses serde_json, which is valid JSON5).
    pub fn to_json5_string(&self) -> Result<String> {
        serde_json::to_string_pretty(self).context("failed to serialize config to JSON5")
    }

    // -----------------------------------------------------------------------
    // File I/O
    // -----------------------------------------------------------------------

    /// Load from a file, detecting format by extension.
    pub fn from_file(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read config file: {}", path.display()))?;

        match path.extension().and_then(|e| e.to_str()) {
            Some("toml") => Self::from_toml_str(&contents),
            Some("json5" | "json") => Self::from_json5_str(&contents),
            _ => Self::from_toml_str(&contents), // default to TOML
        }
    }

    /// Write to a file, detecting format by extension.
    pub fn write_to_file(&self, path: &Path) -> Result<()> {
        let contents = match path.extension().and_then(|e| e.to_str()) {
            Some("json5" | "json") => self.to_json5_string()?,
            _ => self.to_toml_string()?,
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, contents)
            .with_context(|| format!("failed to write config file: {}", path.display()))?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Ancestor directory search
    // -----------------------------------------------------------------------

    /// Walk up from `start_dir` looking for `objfs.toml` or `.objfs.toml`.
    pub fn find_in_ancestors(start_dir: &Path) -> Option<PathBuf> {
        let mut dir = start_dir.to_path_buf();
        loop {
            let candidate = dir.join("objfs.toml");
            if candidate.is_file() {
                return Some(candidate);
            }
            let dotfile = dir.join(".objfs.toml");
            if dotfile.is_file() {
                return Some(dotfile);
            }
            if !dir.pop() {
                return None;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Full load with precedence
    // -----------------------------------------------------------------------

    /// Load configuration with 4-level precedence:
    /// env overrides > project config > global config > defaults.
    pub fn load() -> Result<Self> {
        let mut config = Self::default();

        // 1. Global config (~/.config/objfs/config.toml)
        if let Some(config_dir) = dirs::config_dir() {
            let global_path = config_dir.join("objfs").join("config.toml");
            if global_path.is_file() {
                config = Self::from_file(&global_path)?;
            }
        }

        // 2. Project config (walk up from cwd)
        if let Ok(cwd) = env::current_dir() {
            if let Some(project_path) = Self::find_in_ancestors(&cwd) {
                let project_config = Self::from_file(&project_path)?;
                config = Self::merge(config, project_config);
            }
        }

        // 3. Environment overrides (highest precedence)
        config.apply_env_overrides();

        Ok(config)
    }

    /// Merge two configs: values from `overlay` take precedence where set.
    fn merge(base: Self, overlay: Self) -> Self {
        Self {
            remote: RemoteSection {
                endpoint: overlay.remote.endpoint.or(base.remote.endpoint),
                instance: if overlay.remote.instance != default_instance() {
                    overlay.remote.instance
                } else {
                    base.remote.instance
                },
                min_remote_size: if overlay.remote.min_remote_size != default_min_remote_size() {
                    overlay.remote.min_remote_size
                } else {
                    base.remote.min_remote_size
                },
            },
            worker: WorkerSection {
                auto_start: overlay.worker.auto_start,
                targets: if overlay.worker.targets.is_empty() {
                    base.worker.targets
                } else {
                    overlay.worker.targets
                },
            },
            project: if overlay.project.project_type.is_empty() {
                base.project
            } else {
                overlay.project
            },
        }
    }

    /// Apply environment variable overrides.
    pub fn apply_env_overrides(&mut self) {
        if let Ok(val) = env::var("OBJFS_REMOTE_ENDPOINT") {
            self.remote.endpoint = Some(val);
        }
        if let Ok(val) = env::var("OBJFS_REMOTE_INSTANCE") {
            self.remote.instance = val;
        }
        if let Ok(val) = env::var("OBJFS_MIN_REMOTE_SIZE") {
            if let Ok(n) = val.parse::<u64>() {
                self.remote.min_remote_size = n;
            }
        }
        if let Ok(val) = env::var("OBJFS_REMOTE_TARGETS") {
            self.worker.targets = val.split(',').map(|s| s.trim().to_string()).collect();
        }
        if env::var("OBJFS_NO_AUTO_WORKER").is_ok() {
            self.worker.auto_start = false;
        }
    }
}

/// Detect the project type by checking for build-system marker files.
pub fn detect_project_type(dir: &Path) -> &'static str {
    let has_cargo = dir.join("Cargo.toml").is_file();
    let has_cmake = dir.join("CMakeLists.txt").is_file();
    let has_make = dir.join("Makefile").is_file() || dir.join("makefile").is_file();

    let count = has_cargo as u8 + has_cmake as u8 + has_make as u8;
    match count {
        0 => "unknown",
        1 if has_cargo => "rust",
        1 if has_cmake => "cmake",
        1 if has_make => "make",
        _ => "mixed",
    }
}
