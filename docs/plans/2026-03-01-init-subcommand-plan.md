# Init Subcommand Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `objfs init` subcommand and runtime config file support so teams share build cache configuration through a committed `objfs.toml`.

**Architecture:** A new `config` module handles parsing, merging, and writing `objfs.toml`/JSON5 files. The `objfs init` command auto-detects the project type, writes `objfs.toml`, and configures the build system. Both `cargo-objfs-rustc` and `objfs-cc-wrapper` load the config file at runtime, with env vars taking precedence.

**Tech Stack:** Rust, toml crate (serialize/deserialize), serde_json5 crate (JSON5 support), existing manual arg parsing in cli.rs.

**Design doc:** `docs/plans/2026-03-01-init-subcommand-design.md`

---

### Task 1: Add toml and json5 dependencies

**Files:**
- Modify: `Cargo.toml:6-12`

**Step 1: Add dependencies**

Add `toml` and `json5` to `[dependencies]` in `Cargo.toml`:

```toml
toml = { version = "0.8", features = ["preserve_order"] }
json5 = "0.4"
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: PASS (no code uses the crates yet, but they resolve)

**Step 3: Commit**

```
feat: Add toml and json5 dependencies for config file support
```

---

### Task 2: Create the ObjfsConfig struct and TOML parsing

**Files:**
- Create: `src/config.rs`
- Modify: `src/lib.rs:1` (add `pub mod config;`)
- Create: `tests/config_test.rs`

**Step 1: Write the failing test**

Create `tests/config_test.rs`:

```rust
use objfs::config::ObjfsConfig;

#[test]
fn test_parse_minimal_toml() {
    let toml_str = r#"
[project]
type = "rust"
"#;
    let config = ObjfsConfig::from_toml_str(toml_str).unwrap();
    assert_eq!(config.project.project_type, "rust");
    assert_eq!(config.remote.instance, "main"); // default
    assert!(config.remote.endpoint.is_none());
    assert!(config.worker.auto_start); // default
}

#[test]
fn test_parse_full_toml() {
    let toml_str = r#"
[remote]
endpoint = "http://build-server:50051"
instance = "staging"
min_remote_size = 1

[worker]
auto_start = false
targets = ["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"]

[project]
type = "mixed"
"#;
    let config = ObjfsConfig::from_toml_str(toml_str).unwrap();
    assert_eq!(config.remote.endpoint.as_deref(), Some("http://build-server:50051"));
    assert_eq!(config.remote.instance, "staging");
    assert_eq!(config.remote.min_remote_size, 1);
    assert!(!config.worker.auto_start);
    assert_eq!(config.worker.targets.len(), 2);
    assert_eq!(config.project.project_type, "mixed");
}

#[test]
fn test_roundtrip_toml() {
    let config = ObjfsConfig {
        remote: objfs::config::RemoteSection {
            endpoint: Some("http://build-server:50051".into()),
            instance: "main".into(),
            min_remote_size: 100_000,
        },
        worker: objfs::config::WorkerSection {
            auto_start: true,
            targets: vec!["aarch64-apple-darwin".into()],
        },
        project: objfs::config::ProjectSection {
            project_type: "rust".into(),
        },
    };
    let toml_str = config.to_toml_string().unwrap();
    let parsed = ObjfsConfig::from_toml_str(&toml_str).unwrap();
    assert_eq!(parsed.remote.endpoint, config.remote.endpoint);
    assert_eq!(parsed.remote.instance, config.remote.instance);
    assert_eq!(parsed.project.project_type, config.project.project_type);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test config_test`
Expected: FAIL - module `config` not found

**Step 3: Write the implementation**

Add `pub mod config;` to `src/lib.rs` after the existing module declarations.

Create `src/config.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjfsConfig {
    #[serde(default)]
    pub remote: RemoteSection,
    #[serde(default)]
    pub worker: WorkerSection,
    #[serde(default)]
    pub project: ProjectSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSection {
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default = "default_instance")]
    pub instance: String,
    #[serde(default = "default_min_remote_size")]
    pub min_remote_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerSection {
    #[serde(default = "default_true")]
    pub auto_start: bool,
    #[serde(default)]
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSection {
    #[serde(default, rename = "type")]
    pub project_type: String,
}

fn default_instance() -> String { "main".into() }
fn default_min_remote_size() -> u64 { 100_000 }
fn default_true() -> bool { true }

impl Default for RemoteSection {
    fn default() -> Self {
        Self {
            endpoint: None,
            instance: default_instance(),
            min_remote_size: default_min_remote_size(),
        }
    }
}

impl Default for WorkerSection {
    fn default() -> Self {
        Self {
            auto_start: true,
            targets: Vec::new(),
        }
    }
}

impl Default for ProjectSection {
    fn default() -> Self {
        Self {
            project_type: String::new(),
        }
    }
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

impl ObjfsConfig {
    /// Parse from a TOML string.
    pub fn from_toml_str(s: &str) -> Result<Self, io::Error> {
        toml::from_str(s).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Parse from a JSON5 string.
    pub fn from_json5_str(s: &str) -> Result<Self, io::Error> {
        json5::from_str(s).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
    }

    /// Serialize to a TOML string.
    pub fn to_toml_string(&self) -> Result<String, io::Error> {
        toml::to_string_pretty(self).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }

    /// Serialize to a JSON5 string (uses serde_json since json5 crate lacks serialization).
    pub fn to_json5_string(&self) -> Result<String, io::Error> {
        serde_json::to_string_pretty(self).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }

    /// Load from a file. Detects format by extension (.json5 vs .toml).
    pub fn from_file(path: &Path) -> Result<Self, io::Error> {
        let content = std::fs::read_to_string(path)?;
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        match ext {
            "json5" | "json" => Self::from_json5_str(&content),
            _ => Self::from_toml_str(&content),
        }
    }

    /// Write to a file. Detects format by extension.
    pub fn write_to_file(&self, path: &Path) -> Result<(), io::Error> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let content = match ext {
            "json5" | "json" => self.to_json5_string()?,
            _ => self.to_toml_string()?,
        };
        std::fs::write(path, content)
    }

    /// Search upward from `start` for objfs.toml or .objfs.toml.
    /// Returns the path and parsed config if found.
    pub fn find_in_ancestors(start: &Path) -> Option<(PathBuf, Self)> {
        let mut dir = start.to_path_buf();
        loop {
            for name in &["objfs.toml", ".objfs.toml"] {
                let candidate = dir.join(name);
                if candidate.is_file() {
                    if let Ok(config) = Self::from_file(&candidate) {
                        return Some((candidate, config));
                    }
                }
            }
            if !dir.pop() {
                break;
            }
        }
        None
    }

    /// Load config with full precedence chain:
    /// 1. Environment variables (highest)
    /// 2. Project objfs.toml (walk up from cwd)
    /// 3. Global ~/.config/objfs/config.toml
    /// 4. Defaults
    pub fn load() -> Self {
        let cwd = std::env::current_dir().unwrap_or_default();

        // Start with defaults
        let mut config = Self::default();

        // Layer 3: global config
        if let Some(config_dir) = dirs::config_dir() {
            let global_path = config_dir.join("objfs").join("config.toml");
            if let Ok(global) = Self::from_file(&global_path) {
                config = global;
            }
        }

        // Layer 2: project config
        if let Some((_path, project_config)) = Self::find_in_ancestors(&cwd) {
            config = project_config;
        }

        // Layer 1: environment variable overrides
        config.apply_env_overrides();

        config
    }

    /// Apply environment variable overrides.
    fn apply_env_overrides(&mut self) {
        if let Ok(v) = std::env::var("OBJFS_REMOTE_ENDPOINT") {
            self.remote.endpoint = Some(v);
        }
        if let Ok(v) = std::env::var("OBJFS_REMOTE_INSTANCE") {
            self.remote.instance = v;
        }
        if let Ok(v) = std::env::var("OBJFS_MIN_REMOTE_SIZE") {
            if let Ok(n) = v.parse() {
                self.remote.min_remote_size = n;
            }
        }
        if let Ok(v) = std::env::var("OBJFS_REMOTE_TARGETS") {
            self.worker.targets = v.split(',').map(|s| s.trim().to_string()).collect();
        }
        if let Ok(_) = std::env::var("OBJFS_NO_AUTO_WORKER") {
            self.worker.auto_start = false;
        }
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --test config_test`
Expected: PASS (3 tests)

**Step 5: Commit**

```
feat: Add ObjfsConfig struct with TOML and JSON5 parsing
```

---

### Task 3: Add JSON5 parsing tests

**Files:**
- Modify: `tests/config_test.rs`

**Step 1: Write the failing test**

Add to `tests/config_test.rs`:

```rust
#[test]
fn test_parse_json5() {
    let json5_str = r#"{
        // CI configuration
        remote: {
            endpoint: "http://build-server:50051",
            instance: "main",
            min_remote_size: 1,
        },
        worker: {
            auto_start: false,
            targets: ["x86_64-unknown-linux-gnu"],
        },
        project: {
            type: "rust",
        },
    }"#;
    let config = ObjfsConfig::from_json5_str(json5_str).unwrap();
    assert_eq!(config.remote.endpoint.as_deref(), Some("http://build-server:50051"));
    assert!(!config.worker.auto_start);
}

#[test]
fn test_load_from_file_toml() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("objfs.toml");
    std::fs::write(&path, r#"
[remote]
endpoint = "http://test:50051"

[project]
type = "cmake"
"#).unwrap();
    let config = ObjfsConfig::from_file(&path).unwrap();
    assert_eq!(config.remote.endpoint.as_deref(), Some("http://test:50051"));
    assert_eq!(config.project.project_type, "cmake");
}

#[test]
fn test_load_from_file_json5() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json5");
    std::fs::write(&path, r#"{ remote: { endpoint: "http://test:50051" }, project: { type: "make" } }"#).unwrap();
    let config = ObjfsConfig::from_file(&path).unwrap();
    assert_eq!(config.remote.endpoint.as_deref(), Some("http://test:50051"));
    assert_eq!(config.project.project_type, "make");
}
```

**Step 2: Run tests to verify they pass**

Run: `cargo test --test config_test`
Expected: PASS (6 tests)

**Step 3: Commit**

```
test: Add JSON5 and file-based config loading tests
```

---

### Task 4: Add ancestor directory search and env override tests

**Files:**
- Modify: `tests/config_test.rs`

**Step 1: Write the tests**

Add to `tests/config_test.rs`:

```rust
#[test]
fn test_find_in_ancestors() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("a").join("b").join("c");
    std::fs::create_dir_all(&nested).unwrap();

    // Place config at root
    std::fs::write(dir.path().join("objfs.toml"), r#"
[remote]
endpoint = "http://found:50051"

[project]
type = "rust"
"#).unwrap();

    // Search from deeply nested dir should find it
    let result = ObjfsConfig::find_in_ancestors(&nested);
    assert!(result.is_some());
    let (path, config) = result.unwrap();
    assert_eq!(path, dir.path().join("objfs.toml"));
    assert_eq!(config.remote.endpoint.as_deref(), Some("http://found:50051"));
}

#[test]
fn test_find_dotfile_variant() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".objfs.toml"), r#"
[project]
type = "cmake"
"#).unwrap();

    let result = ObjfsConfig::find_in_ancestors(dir.path());
    assert!(result.is_some());
    let (_, config) = result.unwrap();
    assert_eq!(config.project.project_type, "cmake");
}

#[test]
fn test_env_overrides() {
    let mut config = ObjfsConfig::default();
    assert!(config.remote.endpoint.is_none());

    unsafe {
        std::env::set_var("OBJFS_REMOTE_ENDPOINT", "http://env-override:50051");
        std::env::set_var("OBJFS_MIN_REMOTE_SIZE", "42");
        std::env::set_var("OBJFS_NO_AUTO_WORKER", "1");
    }

    config.apply_env_overrides();

    assert_eq!(config.remote.endpoint.as_deref(), Some("http://env-override:50051"));
    assert_eq!(config.remote.min_remote_size, 42);
    assert!(!config.worker.auto_start);

    // Cleanup
    unsafe {
        std::env::remove_var("OBJFS_REMOTE_ENDPOINT");
        std::env::remove_var("OBJFS_MIN_REMOTE_SIZE");
        std::env::remove_var("OBJFS_NO_AUTO_WORKER");
    }
}
```

Note: `apply_env_overrides` must be `pub` for tests. Change `fn apply_env_overrides` to `pub fn apply_env_overrides` in `src/config.rs`.

**Step 2: Run tests**

Run: `cargo test --test config_test`
Expected: PASS (9 tests)

**Step 3: Commit**

```
test: Add ancestor search and env override config tests
```

---

### Task 5: Add project type auto-detection

**Files:**
- Modify: `src/config.rs`
- Modify: `tests/config_test.rs`

**Step 1: Write the failing test**

Add to `tests/config_test.rs`:

```rust
use objfs::config::detect_project_type;

#[test]
fn test_detect_rust_project() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
    assert_eq!(detect_project_type(dir.path()), "rust");
}

#[test]
fn test_detect_cmake_project() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("CMakeLists.txt"), "project(test)").unwrap();
    assert_eq!(detect_project_type(dir.path()), "cmake");
}

#[test]
fn test_detect_make_project() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Makefile"), "all:").unwrap();
    assert_eq!(detect_project_type(dir.path()), "make");
}

#[test]
fn test_detect_mixed_project() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
    std::fs::write(dir.path().join("CMakeLists.txt"), "project(test)").unwrap();
    assert_eq!(detect_project_type(dir.path()), "mixed");
}

#[test]
fn test_detect_unknown_project() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(detect_project_type(dir.path()), "unknown");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test config_test test_detect`
Expected: FAIL - `detect_project_type` not found

**Step 3: Implement**

Add to `src/config.rs`:

```rust
/// Detect project type by scanning the directory for build system files.
pub fn detect_project_type(dir: &Path) -> &'static str {
    let has_cargo = dir.join("Cargo.toml").is_file();
    let has_cmake = dir.join("CMakeLists.txt").is_file();
    let has_make = dir.join("Makefile").is_file() || dir.join("makefile").is_file();

    match (has_cargo, has_cmake || has_make) {
        (true, true) => "mixed",
        (true, false) => "rust",
        (false, true) if has_cmake => "cmake",
        (false, true) => "make",
        (false, false) => "unknown",
    }
}
```

**Step 4: Run tests**

Run: `cargo test --test config_test`
Expected: PASS (14 tests)

**Step 5: Commit**

```
feat: Add project type auto-detection
```

---

### Task 6: Implement `objfs init` subcommand

**Files:**
- Modify: `src/bin/cli.rs:22-44` (add init to match)
- Modify: `src/bin/cli.rs:47-67` (update print_usage)

**Step 1: Add init command routing**

In `src/bin/cli.rs`, add to the match block (line 22):

```rust
"init" => cmd_init(&args[1..]),
```

Add init to `print_usage()`:

```rust
println!("    init          Initialize objfs for current project");
println!("    init --config <file>    Apply shared config (TOML or JSON5)");
println!("    init --export-config    Export current config to objfs.toml");
```

**Step 2: Implement cmd_init**

Add to `src/bin/cli.rs`:

```rust
use objfs::config::{ObjfsConfig, detect_project_type};
```

```rust
fn cmd_init(args: &[String]) -> io::Result<()> {
    let cwd = env::current_dir()?;

    // Handle --export-config
    if args.iter().any(|a| a.starts_with("--export-config")) {
        return cmd_export_config(args);
    }

    // Load base config: from --config file, existing objfs.toml, or defaults
    let mut config = if let Some(pos) = args.iter().position(|a| a == "--config") {
        let path = args.get(pos + 1)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "--config requires a file path"))?;
        let path = PathBuf::from(path);
        if !path.is_file() {
            return Err(io::Error::new(io::ErrorKind::NotFound, format!("Config file not found: {}", path.display())));
        }
        println!("Loading config from: {}", path.display());
        ObjfsConfig::from_file(&path)?
    } else if let Some((path, existing)) = ObjfsConfig::find_in_ancestors(&cwd) {
        println!("Found existing config: {}", path.display());
        existing
    } else {
        ObjfsConfig::default()
    };

    // Auto-detect project type if not set
    if config.project.project_type.is_empty() {
        config.project.project_type = detect_project_type(&cwd).to_string();
        println!("Detected project type: {}", config.project.project_type);
    }

    // Prompt for remote endpoint if not set and no --config was given
    if config.remote.endpoint.is_none() && !args.iter().any(|a| a == "--config") {
        eprint!("Remote endpoint (empty for local-only): ");
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        if !input.is_empty() {
            config.remote.endpoint = Some(input.to_string());
        }
    }

    // Write objfs.toml
    let config_path = cwd.join("objfs.toml");
    config.write_to_file(&config_path)?;
    println!("Wrote: {}", config_path.display());

    // Configure build system integration
    match config.project.project_type.as_str() {
        "rust" | "mixed" => configure_rust_project(&cwd)?,
        "cmake" => print_cmake_instructions(),
        "make" => print_make_instructions(),
        _ => eprintln!("Warning: Unknown project type; skipping build system configuration"),
    }

    // Verify remote connectivity
    if let Some(ref endpoint) = config.remote.endpoint {
        verify_endpoint(endpoint);
    }

    println!();
    println!("objfs initialized. Configuration saved to objfs.toml.");

    Ok(())
}

fn cmd_export_config(args: &[String]) -> io::Result<()> {
    let config = ObjfsConfig::load();
    let export_arg = args.iter().find(|a| a.starts_with("--export-config")).unwrap();

    let path = if export_arg.contains("=json5") {
        PathBuf::from("objfs.json5")
    } else {
        PathBuf::from("objfs.toml")
    };

    config.write_to_file(&path)?;
    println!("Exported config to: {}", path.display());
    Ok(())
}

fn configure_rust_project(project_dir: &Path) -> io::Result<()> {
    let config_dir = project_dir.join(".cargo");
    let config_file = config_dir.join("config.toml");

    std::fs::create_dir_all(&config_dir)?;

    let wrapper_line = "rustc-wrapper = \"cargo-objfs-rustc\"";

    if config_file.exists() {
        let content = std::fs::read_to_string(&config_file)?;
        if content.contains("cargo-objfs-rustc") {
            println!(".cargo/config.toml already configured");
            return Ok(());
        }
        if content.contains("rustc-wrapper") {
            eprintln!("Warning: .cargo/config.toml has a different rustc-wrapper");
            eprintln!("  Please update it manually to:");
            eprintln!("  {}", wrapper_line);
            return Ok(());
        }
        // Append to existing file
        let mut new_content = content;
        if !new_content.contains("[build]") {
            new_content.push_str("\n[build]\n");
        }
        new_content.push_str(&format!("{}\n", wrapper_line));
        std::fs::write(&config_file, new_content)?;
    } else {
        std::fs::write(&config_file, format!("[build]\n{}\n", wrapper_line))?;
    }
    println!("Configured: {}", config_file.display());

    Ok(())
}

fn print_cmake_instructions() {
    println!();
    println!("For CMake, add to your build command:");
    println!("  cmake .. \\");
    println!("    -DCMAKE_C_COMPILER_LAUNCHER=/usr/local/bin/objfs-cc-wrapper \\");
    println!("    -DCMAKE_CXX_COMPILER_LAUNCHER=/usr/local/bin/objfs-cc-wrapper");
}

fn print_make_instructions() {
    println!();
    println!("For Make, set CC and CXX:");
    println!("  CC=\"objfs-cc-wrapper gcc\" CXX=\"objfs-cc-wrapper g++\" make");
}

fn verify_endpoint(endpoint: &str) {
    let health_url = format!("{}/health", endpoint.trim_end_matches('/'));
    print!("Checking {}... ", health_url);

    match reqwest::blocking::Client::new()
        .get(&health_url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
    {
        Ok(resp) if resp.status().is_success() => println!("OK"),
        Ok(resp) => eprintln!("Warning: server returned {}", resp.status()),
        Err(e) => eprintln!("Warning: unreachable ({})", e),
    }
}
```

**Step 3: Run the binary to verify it works**

Run: `cargo build --bin objfs && ./target/debug/objfs help`
Expected: Output includes `init` in the command list.

Run: `cargo build --bin objfs && echo "" | ./target/debug/objfs init` (in a temp Cargo project)
Expected: Creates objfs.toml, detects project type, writes .cargo/config.toml.

**Step 4: Commit**

```
feat: Add objfs init subcommand with auto-detection and config export
```

---

### Task 7: Wire ObjfsConfig into cargo-objfs-rustc

**Files:**
- Modify: `src/bin/rustc_wrapper.rs:11,76` (use ObjfsConfig instead of RemoteConfig::from_env)

**Step 1: Update imports and config loading**

In `src/bin/rustc_wrapper.rs`, replace:

```rust
use objfs::remote_config::RemoteConfig;
```

with:

```rust
use objfs::config::ObjfsConfig;
use objfs::remote_config::RemoteConfig;
```

At line 76 where `RemoteConfig::from_env()` is called, change it to load via `ObjfsConfig`:

```rust
let objfs_config = ObjfsConfig::load();
let remote_config = RemoteConfig::from_objfs_config(&objfs_config);
```

**Step 2: Add `from_objfs_config` to RemoteConfig**

In `src/remote_config.rs`, add:

```rust
use crate::config::ObjfsConfig;

impl RemoteConfig {
    /// Create RemoteConfig from an ObjfsConfig.
    /// ObjfsConfig already handles the full precedence chain
    /// (env vars > objfs.toml > global config > defaults).
    pub fn from_objfs_config(config: &ObjfsConfig) -> Self {
        let endpoint = config.remote.endpoint.clone()
            .or_else(|| {
                if Self::localhost_worker_available() {
                    Some("http://localhost:50051".to_string())
                } else {
                    None
                }
            });

        let remote_target_platforms = if config.worker.targets.is_empty() {
            vec![Self::host_target_triple()]
        } else {
            config.worker.targets.clone()
        };

        Self {
            endpoint,
            instance_name: config.remote.instance.clone(),
            remote_target_platforms,
            min_remote_size: config.remote.min_remote_size,
        }
    }
}
```

**Step 3: Verify tests still pass**

Run: `cargo test`
Expected: All existing tests pass. The `from_env()` path remains intact for backward compatibility.

**Step 4: Commit**

```
feat: Wire ObjfsConfig into cargo-objfs-rustc runtime
```

---

### Task 8: Wire ObjfsConfig into objfs-cc-wrapper

**Files:**
- Modify: `src/bin/objfs-cc-wrapper.rs` (check OBJFS_DISABLE via config, future remote support)

**Step 1: Update the wrapper to check config**

The cc-wrapper currently checks `OBJFS_DISABLE` directly. Add config file awareness so a project's `objfs.toml` is respected. At the top of `run()`, after parsing args:

```rust
use objfs::config::ObjfsConfig;

// Check if objfs is disabled
if std::env::var("OBJFS_DISABLE").is_ok() {
    return exec_compiler(compiler, compiler_args);
}
```

This is minimal -- the cc-wrapper doesn't use remote execution yet, so the config integration is light. The key value is that a future remote C/C++ execution feature can read `ObjfsConfig::load()` for the endpoint.

**Step 2: Verify the wrapper still works**

Run: `cargo build --bin objfs-cc-wrapper`
Expected: Compiles without errors.

**Step 3: Commit**

```
feat: Add ObjfsConfig awareness to objfs-cc-wrapper
```

---

### Task 9: Add integration test for full init flow

**Files:**
- Create: `tests/init_integration_test.rs`

**Step 1: Write the integration test**

```rust
use std::process::Command;

#[test]
fn test_init_creates_config_in_rust_project() {
    let dir = tempfile::tempdir().unwrap();

    // Create a minimal Cargo.toml
    std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"test-project\"\nversion = \"0.1.0\"\nedition = \"2021\"").unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();

    // Run objfs init with empty endpoint (local only)
    let output = Command::new(env!("CARGO_BIN_EXE_objfs"))
        .arg("init")
        .current_dir(dir.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.take().unwrap().write_all(b"\n")?; // empty endpoint
            child.wait_with_output()
        })
        .unwrap();

    assert!(output.status.success(), "init failed: {}", String::from_utf8_lossy(&output.stderr));

    // objfs.toml should exist
    let config_path = dir.path().join("objfs.toml");
    assert!(config_path.is_file(), "objfs.toml not created");

    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("rust"), "should detect rust project type");

    // .cargo/config.toml should exist with rustc-wrapper
    let cargo_config = dir.path().join(".cargo/config.toml");
    assert!(cargo_config.is_file(), ".cargo/config.toml not created");

    let cargo_content = std::fs::read_to_string(&cargo_config).unwrap();
    assert!(cargo_content.contains("cargo-objfs-rustc"), "rustc-wrapper not set");
}

#[test]
fn test_init_with_config_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2021\"").unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();

    // Create a shared config file
    let shared_config = dir.path().join("team-config.toml");
    std::fs::write(&shared_config, r#"
[remote]
endpoint = "http://build-cluster:50051"
instance = "team"
min_remote_size = 1

[worker]
auto_start = false

[project]
type = "rust"
"#).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_objfs"))
        .args(["init", "--config"])
        .arg(&shared_config)
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success(), "init --config failed: {}", String::from_utf8_lossy(&output.stderr));

    // Verify the written config matches
    let config_path = dir.path().join("objfs.toml");
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("build-cluster"), "endpoint not propagated");
    assert!(content.contains("team"), "instance not propagated");
}

#[test]
fn test_export_config() {
    let dir = tempfile::tempdir().unwrap();

    // Create an existing objfs.toml
    std::fs::write(dir.path().join("objfs.toml"), r#"
[remote]
endpoint = "http://existing:50051"

[project]
type = "rust"
"#).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_objfs"))
        .args(["init", "--export-config=json5"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());

    let json5_path = dir.path().join("objfs.json5");
    assert!(json5_path.is_file(), "objfs.json5 not created");

    let content = std::fs::read_to_string(&json5_path).unwrap();
    assert!(content.contains("existing"), "endpoint not in exported JSON5");
}
```

**Step 2: Run the tests**

Run: `cargo test --test init_integration_test`
Expected: PASS (3 tests)

**Step 3: Commit**

```
test: Add integration tests for objfs init subcommand
```

---

### Task 10: Update documentation

**Files:**
- Modify: `USAGE.md` (add init section)
- Modify: `README.md` (mention init in usage)

**Step 1: Update USAGE.md**

Add a new section after "## Installation" and before "## Local Caching":

```markdown
## Quick Setup

Initialize a project:

```bash
objfs init
```

objfs detects the project type (Rust, CMake, Make) and writes `objfs.toml`.
For Rust projects, it also creates `.cargo/config.toml` with the rustc wrapper.

To share configuration across a team, export and commit the config file:

```bash
objfs init --export-config
git add objfs.toml
```

Other developers apply the shared config:

```bash
objfs init --config objfs.toml
```
```

**Step 2: Update README.md**

In the Usage section, add before "See USAGE.md":

```markdown
# Quick setup
objfs init
```

**Step 3: Commit**

```
docs: Add init subcommand to USAGE.md and README.md
```
