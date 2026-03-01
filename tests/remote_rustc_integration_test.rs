// Integration test for remote rustc compilation

use objfs::remote_config::RemoteConfig;
use objfs::re_client::{Command, Action, RemoteExecutor};
use std::env;
use std::path::PathBuf;

#[test]
fn test_should_use_remote_for_cross_compilation() {
    // Setup remote config for cross-compilation scenario
    unsafe {
        env::set_var("OBJFS_REMOTE_ENDPOINT", "http://localhost:50051");
        env::set_var("OBJFS_REMOTE_TARGETS", "aarch64-apple-darwin,x86_64-apple-darwin");
        env::set_var("OBJFS_MIN_REMOTE_SIZE", "1024"); // 1KB threshold
    }

    let config = RemoteConfig::from_env();

    // Should use remote for aarch64-apple-darwin target with sufficient size
    assert!(config.should_use_remote("aarch64-apple-darwin", 2048));

    // Should NOT use remote if size is too small
    assert!(!config.should_use_remote("aarch64-apple-darwin", 512));

    // Should NOT use remote for unsupported target
    assert!(!config.should_use_remote("x86_64-unknown-linux-gnu", 2048));

    // Cleanup
    unsafe {
        env::remove_var("OBJFS_REMOTE_ENDPOINT");
        env::remove_var("OBJFS_REMOTE_TARGETS");
        env::remove_var("OBJFS_MIN_REMOTE_SIZE");
    }
}

#[test]
fn test_remote_execution_disabled_without_config() {
    unsafe {
        env::remove_var("OBJFS_REMOTE_ENDPOINT");
        env::remove_var("OBJFS_REMOTE_TARGETS");
    }

    let config = RemoteConfig::from_env();

    // Should not use remote when not configured
    assert!(!config.should_use_remote("aarch64-apple-darwin", 1_000_000));

    // Should not even be enabled
    assert!(!config.is_enabled());
}

#[test]
fn test_create_action_from_rustc_build() {
    let args = vec![
        "--crate-name", "mylib",
        "--target", "aarch64-apple-darwin",
        "--crate-type", "lib",
        "src/lib.rs"
    ];

    // Create command from rustc args
    let command = Command::from_rustc_args(&args, &PathBuf::from("/build"));

    // Create action with input files
    let input_files = vec![PathBuf::from("src/lib.rs")];
    let action = Action::new(command, input_files);

    // Action should have valid digest
    assert!(!action.command_digest.hash.is_empty());
    assert_eq!(action.input_files.len(), 1);
}

#[test]
fn test_remote_executor_creation_from_config() {
    unsafe {
        env::set_var("OBJFS_REMOTE_ENDPOINT", "https://localhost:50051");
        env::set_var("OBJFS_REMOTE_TARGETS", "aarch64-apple-darwin");
        env::set_var("OBJFS_REMOTE_INSTANCE", "main");
    }

    let config = RemoteConfig::from_env();

    assert!(config.is_enabled());

    // Should be able to create executor from config
    if let Some(endpoint) = config.endpoint {
        let _executor = RemoteExecutor::new(
            endpoint,
            config.instance_name,
            true // use TLS
        );
        // Executor created successfully
    }

    unsafe {
        env::remove_var("OBJFS_REMOTE_ENDPOINT");
        env::remove_var("OBJFS_REMOTE_TARGETS");
        env::remove_var("OBJFS_REMOTE_INSTANCE");
    }
}
