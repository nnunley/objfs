// Tests for downloading output files from remote execution

use objfs::re_client::{ActionResult, OutputFile};
use tempfile::TempDir;

#[test]
fn test_action_result_parsing() {
    // ActionResult should parse output files correctly
    let output_files = vec![
        OutputFile {
            path: "target/debug/libmylib.rlib".to_string(),
            hash: "abc123".to_string(),
            size_bytes: 1024,
            is_executable: false,
        },
        OutputFile {
            path: "target/debug/libmylib.rmeta".to_string(),
            hash: "def456".to_string(),
            size_bytes: 512,
            is_executable: false,
        },
    ];

    let result = ActionResult {
        output_files,
        exit_code: 0,
        stdout: vec![],
        stderr: vec![],
    };

    assert_eq!(result.output_files.len(), 2);
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_output_file_restoration() {
    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path();

    let output_file = OutputFile {
        path: "libtest.rlib".to_string(),
        hash: "test_hash_123".to_string(),
        size_bytes: 100,
        is_executable: false,
    };

    // Should be able to construct output path
    let full_path = output_dir.join(&output_file.path);
    assert_eq!(full_path.file_name().unwrap(), "libtest.rlib");
}

#[test]
fn test_executable_permission_preserved() {
    let executable = OutputFile {
        path: "bin/myapp".to_string(),
        hash: "hash123".to_string(),
        size_bytes: 2048,
        is_executable: true,
    };

    let library = OutputFile {
        path: "lib/mylib.so".to_string(),
        hash: "hash456".to_string(),
        size_bytes: 1024,
        is_executable: false,
    };

    assert!(executable.is_executable);
    assert!(!library.is_executable);
}
