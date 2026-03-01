use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use tempfile::TempDir;

/// Run `objfs init` in a Rust project directory, providing an empty line for
/// the endpoint prompt via stdin. Verify that objfs.toml and .cargo/config.toml
/// are created with the expected content.
#[test]
fn test_init_creates_config_in_rust_project() {
    let dir = TempDir::new().unwrap();

    // Minimal Rust project
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    let src = dir.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("main.rs"), "fn main() {}\n").unwrap();

    // Spawn objfs init, pipe an empty line to stdin for the endpoint prompt
    let mut child = Command::new(env!("CARGO_BIN_EXE_objfs"))
        .args(["init"])
        .current_dir(dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn objfs");

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"\n")
        .expect("failed to write to stdin");

    let output = child.wait_with_output().expect("failed to wait on objfs");
    assert!(
        output.status.success(),
        "objfs init failed: {}",
        String::from_utf8_lossy(&output.stderr),
    );

    // objfs.toml must exist and mention the detected project type
    let config_contents = fs::read_to_string(dir.path().join("objfs.toml"))
        .expect("objfs.toml not created");
    assert!(
        config_contents.contains("rust"),
        "objfs.toml should contain 'rust', got:\n{config_contents}",
    );

    // .cargo/config.toml must exist and reference the rustc wrapper
    let cargo_config = fs::read_to_string(dir.path().join(".cargo/config.toml"))
        .expect(".cargo/config.toml not created");
    assert!(
        cargo_config.contains("cargo-objfs-rustc"),
        ".cargo/config.toml should contain 'cargo-objfs-rustc', got:\n{cargo_config}",
    );
}

/// Run `objfs init --config <file>` with a pre-written team config file.
/// Verify that the generated objfs.toml picks up the values from the config.
#[test]
fn test_init_with_config_file() {
    let dir = TempDir::new().unwrap();

    // Minimal Rust project
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    let src = dir.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("main.rs"), "fn main() {}\n").unwrap();

    // Shared team config
    let team_config = dir.path().join("team-config.toml");
    fs::write(
        &team_config,
        r#"[remote]
endpoint = "http://build-cluster:50051"
instance = "team"
min_remote_size = 1

[worker]
auto_start = false

[project]
type = "rust"
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_objfs"))
        .args(["init", "--config", team_config.to_str().unwrap()])
        .current_dir(dir.path())
        .output()
        .expect("failed to run objfs");

    assert!(
        output.status.success(),
        "objfs init --config failed: {}",
        String::from_utf8_lossy(&output.stderr),
    );

    let config_contents = fs::read_to_string(dir.path().join("objfs.toml"))
        .expect("objfs.toml not created");
    assert!(
        config_contents.contains("build-cluster"),
        "objfs.toml should contain 'build-cluster', got:\n{config_contents}",
    );
    assert!(
        config_contents.contains("team"),
        "objfs.toml should contain 'team', got:\n{config_contents}",
    );
}

/// Run `objfs init --export-config=json5` with an existing objfs.toml.
/// Verify that objfs.json5 is created with the expected content.
#[test]
fn test_export_config() {
    let dir = TempDir::new().unwrap();

    // Write an existing objfs.toml that the export command will read
    fs::write(
        dir.path().join("objfs.toml"),
        r#"[remote]
endpoint = "http://existing:50051"

[project]
type = "rust"
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_objfs"))
        .args(["init", "--export-config=json5"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run objfs");

    assert!(
        output.status.success(),
        "objfs init --export-config=json5 failed: {}",
        String::from_utf8_lossy(&output.stderr),
    );

    let json5_contents = fs::read_to_string(dir.path().join("objfs.json5"))
        .expect("objfs.json5 not created");
    assert!(
        json5_contents.contains("existing"),
        "objfs.json5 should contain 'existing', got:\n{json5_contents}",
    );
}
