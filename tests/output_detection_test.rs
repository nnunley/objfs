use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

// Test detecting actual rustc output files

#[test]
fn test_detect_files_created_in_directory() {
    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().join("output");
    fs::create_dir_all(&output_dir).unwrap();

    // Create some initial files
    fs::write(output_dir.join("existing.txt"), b"old").unwrap();

    // Simulate rustc creating new files
    fs::write(output_dir.join("new_binary-abc123"), b"binary").unwrap();
    fs::write(output_dir.join("new_binary-abc123.d"), b"deps").unwrap();

    // Detect only new files
    let new_files = detect_new_files(&output_dir, &["existing.txt"]).unwrap();

    assert_eq!(new_files.len(), 2);
    assert!(new_files.iter().any(|f| f.file_name().unwrap() == "new_binary-abc123"));
    assert!(new_files.iter().any(|f| f.file_name().unwrap() == "new_binary-abc123.d"));
}

#[test]
fn test_detect_files_matching_pattern() {
    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().join("output");
    fs::create_dir_all(&output_dir).unwrap();

    // Create files matching expected patterns
    fs::write(output_dir.join("libmylib-abc123.rlib"), b"rlib").unwrap();
    fs::write(output_dir.join("libmylib-abc123.d"), b"deps").unwrap();
    fs::write(output_dir.join("libmylib-abc123.rmeta"), b"meta").unwrap();
    fs::write(output_dir.join("other.txt"), b"other").unwrap();

    // Detect files matching crate name pattern
    let crate_files = detect_crate_outputs(&output_dir, "mylib").unwrap();

    assert_eq!(crate_files.len(), 3);
    assert!(crate_files.iter().any(|f| f.to_str().unwrap().ends_with(".rlib")));
    assert!(crate_files.iter().any(|f| f.to_str().unwrap().ends_with(".d")));
    assert!(crate_files.iter().any(|f| f.to_str().unwrap().ends_with(".rmeta")));
    assert!(!crate_files.iter().any(|f| f.file_name().unwrap() == "other.txt"));
}

#[test]
fn test_parse_emit_types() {
    let emit_str = "dep-info,metadata,link";
    let types = parse_emit_types(emit_str);

    assert_eq!(types.len(), 3);
    assert!(types.contains(&"dep-info".to_string()));
    assert!(types.contains(&"metadata".to_string()));
    assert!(types.contains(&"link".to_string()));
}

#[test]
fn test_expected_extensions_from_emit() {
    let emit_types = vec!["dep-info", "metadata", "link"];
    let extensions = expected_extensions(&emit_types);

    assert!(extensions.contains(&"d".to_string()));
    assert!(extensions.contains(&"rmeta".to_string()));
    assert!(extensions.contains(&"rlib".to_string()) || extensions.contains(&"".to_string()));
}

// Functions implementation

fn detect_new_files(dir: &Path, existing: &[&str]) -> std::io::Result<Vec<PathBuf>> {
    let mut new_files = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            let filename = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            if !existing.contains(&filename) {
                new_files.push(path);
            }
        }
    }

    Ok(new_files)
}

fn detect_crate_outputs(dir: &Path, crate_name: &str) -> std::io::Result<Vec<PathBuf>> {
    let mut outputs = Vec::new();

    // Pattern: lib{crate_name}-{hash}.{ext} or {crate_name}-{hash}.{ext}
    let lib_prefix = format!("lib{}-", crate_name);
    let bin_prefix = format!("{}-", crate_name);

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            let filename = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            if filename.starts_with(&lib_prefix) || filename.starts_with(&bin_prefix) {
                // Check if it's a known artifact type
                if filename.ends_with(".rlib") ||
                   filename.ends_with(".d") ||
                   filename.ends_with(".rmeta") ||
                   filename.ends_with(".so") ||
                   filename.ends_with(".dylib") ||
                   !filename.contains('.') // binary without extension
                {
                    outputs.push(path);
                }
            }
        }
    }

    Ok(outputs)
}

fn parse_emit_types(emit_str: &str) -> Vec<String> {
    emit_str.split(',')
        .map(|s| s.trim().to_string())
        .collect()
}

fn expected_extensions(emit_types: &[&str]) -> Vec<String> {
    let mut extensions = Vec::new();

    for emit_type in emit_types {
        match *emit_type {
            "dep-info" => extensions.push("d".to_string()),
            "metadata" => extensions.push("rmeta".to_string()),
            "link" => extensions.push("rlib".to_string()),
            _ => {}
        }
    }

    extensions
}
