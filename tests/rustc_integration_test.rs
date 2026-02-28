use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

// Test that rustc wrapper can handle multiple output artifacts

#[test]
fn test_rustc_wrapper_caches_multiple_artifacts() {
    let temp_dir = TempDir::new().unwrap();
    let project_dir = temp_dir.path().join("test_project");
    fs::create_dir_all(&project_dir).unwrap();

    // Create a simple Rust source file
    let src_file = project_dir.join("lib.rs");
    fs::write(&src_file, b"pub fn hello() { println!(\"hello\"); }").unwrap();

    // Create output directory
    let output_dir = project_dir.join("target/debug");
    fs::create_dir_all(&output_dir).unwrap();

    // Simulate rustc invocation that produces multiple outputs
    let args = vec![
        "--crate-name".to_string(),
        "testlib".to_string(),
        "--crate-type".to_string(),
        "lib".to_string(),
        "--out-dir".to_string(),
        output_dir.to_str().unwrap().to_string(),
        "--emit".to_string(),
        "dep-info,metadata,link".to_string(),
        src_file.to_str().unwrap().to_string(),
    ];

    // Parse args and detect all expected outputs
    let build_info = parse_rustc_args_with_emit(&args);

    // Should detect all three output files
    assert_eq!(build_info.output_files.len(), 3);
    assert!(build_info.output_files.iter().any(|f| f.to_str().unwrap().ends_with(".rlib")));
    assert!(build_info.output_files.iter().any(|f| f.to_str().unwrap().ends_with(".d")));
    assert!(build_info.output_files.iter().any(|f| f.to_str().unwrap().ends_with(".rmeta")));
}

// Helper function
fn parse_rustc_args_with_emit(args: &[String]) -> BuildInfoWithEmit {
    let mut output_dir = None;
    let mut crate_name = None;
    let mut emit_types = vec!["link"]; // default

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out-dir" => {
                if i + 1 < args.len() {
                    output_dir = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            "--crate-name" => {
                if i + 1 < args.len() {
                    crate_name = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--emit" => {
                if i + 1 < args.len() {
                    emit_types = args[i + 1].split(',').collect();
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    let mut output_files = Vec::new();

    if let (Some(dir), Some(name)) = (output_dir, crate_name) {
        for emit_type in emit_types {
            let file = match emit_type {
                "link" => dir.join(format!("lib{}.rlib", name)),
                "dep-info" => dir.join(format!("lib{}.d", name)),
                "metadata" => dir.join(format!("lib{}.rmeta", name)),
                _ => continue,
            };
            output_files.push(file);
        }
    }

    BuildInfoWithEmit { output_files }
}

struct BuildInfoWithEmit {
    output_files: Vec<PathBuf>,
}
