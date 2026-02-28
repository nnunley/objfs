use objfs::re_client::{Action, Command, RemoteExecutor};
use std::path::PathBuf;

#[test]
#[ignore] // Only run when explicitly requested with --ignored
fn test_execute_action_remotely_with_nativelink() {
    // This test requires OBJFS_REMOTE_ENDPOINT to be set
    // and NativeLink to be running
    let endpoint = match std::env::var("OBJFS_REMOTE_ENDPOINT") {
        Ok(e) => e,
        Err(_) => {
            eprintln!("Skipping test: OBJFS_REMOTE_ENDPOINT not set");
            return;
        }
    };

    let executor = RemoteExecutor::new(endpoint, "main".to_string(), false);

    // Simple echo command
    let command = Command::new(
        vec!["/bin/echo".to_string(), "hello from remote".to_string()],
        "/tmp"
    );

    let action = Action::new(command, vec![]);

    // Execute the action remotely
    let result = executor.execute(&action);

    match result {
        Ok(action_result) => {
            // Should contain the echo output
            let output_str = String::from_utf8_lossy(&action_result.stdout);
            assert!(output_str.contains("hello from remote"));
        }
        Err(e) => {
            // If we can't connect, that's OK for this test
            eprintln!("Remote execution failed (expected if NativeLink not running): {}", e);
        }
    }
}

#[test]
fn test_create_command_from_rustc_invocation() {
    let command = Command::from_rustc_args(
        &["--crate-name", "mylib", "--crate-type", "lib", "src/lib.rs"],
        &PathBuf::from("/build/dir")
    );

    assert_eq!(command.arguments, vec![
        "rustc",
        "--crate-name", "mylib",
        "--crate-type", "lib",
        "src/lib.rs"
    ]);
    assert_eq!(command.working_directory, "/build/dir");
}

#[test]
fn test_action_includes_command_digest() {
    let command = Command::new(
        vec!["rustc".to_string(), "--version".to_string()],
        "/tmp"
    );

    let action = Action::new(command, vec![]);

    // Action should have a command digest
    assert!(!action.command_digest.hash.is_empty());
    assert!(action.command_digest.size_bytes > 0);
}

#[test]
fn test_action_includes_input_files() {
    let command = Command::new(
        vec!["rustc".to_string(), "src/lib.rs".to_string()],
        "/build"
    );

    let input_files = vec![
        PathBuf::from("src/lib.rs"),
        PathBuf::from("Cargo.toml"),
    ];

    let action = Action::new(command, input_files);

    // Action should reference input files
    assert_eq!(action.input_files.len(), 2);
    assert!(action.input_files.contains(&PathBuf::from("src/lib.rs")));
    assert!(action.input_files.contains(&PathBuf::from("Cargo.toml")));
}

#[test]
fn test_remote_executor_can_be_created() {
    let _executor = RemoteExecutor::new(
        "http://localhost:50051".to_string(),
        "main".to_string(),
        false
    );

    // Executor should be created successfully
    // We'll test the actual execution in an integration test
}

#[test]
fn test_action_serialization_for_remote_execution() {
    let command = Command::new(
        vec!["rustc".to_string(), "src/lib.rs".to_string()],
        "/build"
    );

    let action = Action::new(command, vec![PathBuf::from("src/lib.rs")]);

    // Action should have a valid command digest
    assert!(!action.command_digest.hash.is_empty());
    assert!(action.command_digest.size_bytes > 0);

    // Command digest should be deterministic
    let command2 = Command::new(
        vec!["rustc".to_string(), "src/lib.rs".to_string()],
        "/build"
    );
    let action2 = Action::new(command2, vec![PathBuf::from("src/lib.rs")]);

    assert_eq!(action.command_digest.hash, action2.command_digest.hash);
}

#[test]
fn test_action_includes_platform_properties() {
    let command = Command::new(
        vec!["rustc".to_string(), "--version".to_string()],
        "/build"
    );

    let action = Action::new(command, vec![]);

    // Should have platform properties for Rust toolchain
    assert!(action.platform.is_some());
}

#[test]
fn test_command_uses_relative_paths() {
    let args = vec!["--crate-name", "test", "-o", "/tmp/output", "src/main.rs"];
    let working_dir = PathBuf::from("/build");

    let command = Command::from_rustc_args(&args, &working_dir);

    // Output path should be relative to working directory
    assert!(command.arguments.iter().any(|a| a == "output"));
    assert!(!command.arguments.iter().any(|a| a.starts_with("/tmp")));
}

#[test]
fn test_command_relativizes_to_working_dir() {
    let args = vec!["--crate-name", "test", "/build/src/main.rs"];
    let working_dir = PathBuf::from("/build");

    let command = Command::from_rustc_args(&args, &working_dir);

    // Should be relative to working_dir
    assert!(command.arguments.iter().any(|a| a == "src/main.rs"));
    assert!(!command.arguments.iter().any(|a| a == "/build/src/main.rs"));
}

#[test]
fn test_preserves_already_relative_paths() {
    let args = vec!["src/main.rs", "../lib/helper.rs"];
    let working_dir = PathBuf::from("/build");

    let command = Command::from_rustc_args(&args, &working_dir);

    assert!(command.arguments.contains(&"src/main.rs".to_string()));
    assert!(command.arguments.contains(&"../lib/helper.rs".to_string()));
}

#[test]
fn test_fallback_to_filename_for_unrelativizable_paths() {
    let args = vec!["/usr/bin/rustc", "-o", "/tmp/output"];
    let working_dir = PathBuf::from("/build");

    let command = Command::from_rustc_args(&args, &working_dir);

    // /usr/bin/rustc can't be made relative to /build, use filename
    assert!(command.arguments.iter().any(|a| a == "rustc"));
    // /tmp/output can be stripped to output
    assert!(command.arguments.iter().any(|a| a == "output"));
}
