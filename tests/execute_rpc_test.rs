// Tests for RE API v2 Execute RPC implementation

use objfs::re_client::{Command, Action, RemoteExecutor};
use std::path::PathBuf;

#[test]
#[ignore] // Only run with --ignored when NativeLink is available
fn test_execute_simple_command_remotely() {
    // Requires OBJFS_REMOTE_ENDPOINT to be set
    let endpoint = match std::env::var("OBJFS_REMOTE_ENDPOINT") {
        Ok(e) => e,
        Err(_) => {
            eprintln!("Skipping: OBJFS_REMOTE_ENDPOINT not set");
            return;
        }
    };

    let executor = RemoteExecutor::new(endpoint, "main".to_string(), false);

    // Simple echo command that should succeed
    let command = Command::new(
        vec!["/bin/echo".to_string(), "test".to_string()],
        "/tmp"
    );

    let action = Action::new(command, vec![]);

    // Execute remotely
    let result = executor.execute(&action);

    // Should either succeed or fail with clear error
    match result {
        Ok(action_result) => {
            // If execution succeeds, should have result
            eprintln!("Remote execution succeeded with exit code {}", action_result.exit_code);
            eprintln!("Output files: {}", action_result.output_files.len());
        }
        Err(e) => {
            // If it fails, should be a connection/config error, not a panic
            eprintln!("Remote execution failed (expected if NativeLink not configured): {}", e);
        }
    }
}

#[test]
fn test_action_digest_is_deterministic() {
    // Same inputs should produce same action digest
    let command1 = Command::new(
        vec!["rustc".to_string(), "main.rs".to_string()],
        "/build"
    );
    let action1 = Action::new(command1, vec![PathBuf::from("main.rs")]);

    let command2 = Command::new(
        vec!["rustc".to_string(), "main.rs".to_string()],
        "/build"
    );
    let action2 = Action::new(command2, vec![PathBuf::from("main.rs")]);

    // Digests should match
    assert_eq!(action1.command_digest.hash, action2.command_digest.hash);
    assert_eq!(action1.command_digest.size_bytes, action2.command_digest.size_bytes);
}

#[test]
fn test_different_commands_have_different_digests() {
    let command1 = Command::new(
        vec!["rustc".to_string(), "main.rs".to_string()],
        "/build"
    );
    let action1 = Action::new(command1, vec![]);

    let command2 = Command::new(
        vec!["rustc".to_string(), "lib.rs".to_string()],
        "/build"
    );
    let action2 = Action::new(command2, vec![]);

    // Different commands should have different digests
    assert_ne!(action1.command_digest.hash, action2.command_digest.hash);
}

#[test]
fn test_working_directory_affects_digest() {
    let command1 = Command::new(
        vec!["rustc".to_string()],
        "/build1"
    );
    let action1 = Action::new(command1, vec![]);

    let command2 = Command::new(
        vec!["rustc".to_string()],
        "/build2"
    );
    let action2 = Action::new(command2, vec![]);

    // Different working directories should produce different digests
    assert_ne!(action1.command_digest.hash, action2.command_digest.hash);
}
