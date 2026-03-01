use std::fs;
use std::io::Write;

use tempfile::TempDir;

use objfs::config::ObjfsConfig;

#[test]
fn test_parse_minimal_toml() {
    let toml = "";
    let config = ObjfsConfig::from_toml_str(toml).unwrap();
    assert_eq!(config.remote.instance, "main");
    assert_eq!(config.remote.min_remote_size, 100_000);
    assert!(config.remote.endpoint.is_none());
    assert!(config.worker.auto_start);
    assert!(config.worker.targets.is_empty());
    assert!(config.project.project_type.is_empty());
}

#[test]
fn test_parse_full_toml() {
    let toml = r#"
[remote]
endpoint = "https://cache.example.com:50051"
instance = "prod"
min_remote_size = 200000

[worker]
auto_start = false
targets = ["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"]

[project]
type = "rust"
"#;
    let config = ObjfsConfig::from_toml_str(toml).unwrap();
    assert_eq!(
        config.remote.endpoint.as_deref(),
        Some("https://cache.example.com:50051")
    );
    assert_eq!(config.remote.instance, "prod");
    assert_eq!(config.remote.min_remote_size, 200_000);
    assert!(!config.worker.auto_start);
    assert_eq!(config.worker.targets.len(), 2);
    assert_eq!(config.project.project_type, "rust");
}

#[test]
fn test_roundtrip_toml() {
    let original = ObjfsConfig::default();
    let serialized = original.to_toml_string().unwrap();
    let parsed = ObjfsConfig::from_toml_str(&serialized).unwrap();
    assert_eq!(original, parsed);
}

#[test]
fn test_parse_json5() {
    let json5 = r#"{
        // Remote cache endpoint
        remote: {
            endpoint: "https://cache.example.com:50051",
            instance: "staging",
            min_remote_size: 50000,
        },
        worker: {
            auto_start: true,
            targets: ["x86_64-unknown-linux-gnu"],
        },
        project: {
            type: "rust",
        },
    }"#;
    let config = ObjfsConfig::from_json5_str(json5).unwrap();
    assert_eq!(
        config.remote.endpoint.as_deref(),
        Some("https://cache.example.com:50051")
    );
    assert_eq!(config.remote.instance, "staging");
    assert_eq!(config.remote.min_remote_size, 50_000);
    assert!(config.worker.auto_start);
    assert_eq!(config.worker.targets, vec!["x86_64-unknown-linux-gnu"]);
    assert_eq!(config.project.project_type, "rust");
}

#[test]
fn test_load_from_file_toml() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("objfs.toml");
    let mut f = std::fs::File::create(&path).unwrap();
    writeln!(
        f,
        r#"
[remote]
endpoint = "https://file-test:50051"
"#
    )
    .unwrap();

    let config = ObjfsConfig::from_file(&path).unwrap();
    assert_eq!(
        config.remote.endpoint.as_deref(),
        Some("https://file-test:50051")
    );
    assert_eq!(config.remote.instance, "main"); // default
}

#[test]
fn test_load_from_file_json5() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("objfs.json5");
    std::fs::write(
        &path,
        r#"{
        remote: { endpoint: "https://json5-test:50051" },
    }"#,
    )
    .unwrap();

    let config = ObjfsConfig::from_file(&path).unwrap();
    assert_eq!(
        config.remote.endpoint.as_deref(),
        Some("https://json5-test:50051")
    );
}

#[test]
fn test_find_in_ancestors() {
    let root = TempDir::new().unwrap();
    // Create nested dirs: root/a/b/c
    let nested = root.path().join("a").join("b").join("c");
    fs::create_dir_all(&nested).unwrap();

    // Place config at root
    let config_path = root.path().join("objfs.toml");
    fs::write(
        &config_path,
        r#"
[remote]
endpoint = "https://ancestor-test:50051"
"#,
    )
    .unwrap();

    // Search from deepest dir should find config at root
    let found = ObjfsConfig::find_in_ancestors(&nested).unwrap();
    assert_eq!(found, config_path);

    // Verify it loads correctly
    let config = ObjfsConfig::from_file(&found).unwrap();
    assert_eq!(
        config.remote.endpoint.as_deref(),
        Some("https://ancestor-test:50051")
    );
}

#[test]
fn test_find_dotfile_variant() {
    let root = TempDir::new().unwrap();
    let child = root.path().join("sub");
    fs::create_dir_all(&child).unwrap();

    // Use dotfile variant
    let dotfile = root.path().join(".objfs.toml");
    fs::write(
        &dotfile,
        r#"
[remote]
instance = "dotfile"
"#,
    )
    .unwrap();

    let found = ObjfsConfig::find_in_ancestors(&child).unwrap();
    assert_eq!(found, dotfile);

    let config = ObjfsConfig::from_file(&found).unwrap();
    assert_eq!(config.remote.instance, "dotfile");
}

#[test]
fn test_env_overrides() {
    let mut config = ObjfsConfig::default();

    // Set env vars
    unsafe {
        std::env::set_var("OBJFS_REMOTE_ENDPOINT", "https://env-test:50051");
        std::env::set_var("OBJFS_MIN_REMOTE_SIZE", "42");
        std::env::set_var("OBJFS_NO_AUTO_WORKER", "1");
    }

    config.apply_env_overrides();

    assert_eq!(
        config.remote.endpoint.as_deref(),
        Some("https://env-test:50051")
    );
    assert_eq!(config.remote.min_remote_size, 42);
    assert!(!config.worker.auto_start);

    // Cleanup
    unsafe {
        std::env::remove_var("OBJFS_REMOTE_ENDPOINT");
        std::env::remove_var("OBJFS_MIN_REMOTE_SIZE");
        std::env::remove_var("OBJFS_NO_AUTO_WORKER");
    }
}
