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
