use std::fs;
use std::path::{Path, PathBuf};
use std::io;

/// Detect files created in a directory after compilation
pub fn detect_new_files(dir: &Path, existing: &[String]) -> io::Result<Vec<PathBuf>> {
    let mut new_files = Vec::new();

    if !dir.exists() {
        return Ok(new_files);
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            let filename = path.file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
                .unwrap_or_default();

            if !existing.contains(&filename) {
                new_files.push(path);
            }
        }
    }

    Ok(new_files)
}

/// Detect all output files for a given crate name
pub fn detect_crate_outputs(dir: &Path, crate_name: &str) -> io::Result<Vec<PathBuf>> {
    let mut outputs = Vec::new();

    if !dir.exists() {
        return Ok(outputs);
    }

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
                   filename.ends_with(".a") ||
                   (!filename.contains('.') && !filename.contains("incremental")) // binary without extension
                {
                    outputs.push(path);
                }
            }
        }
    }

    Ok(outputs)
}

/// List all files currently in a directory (for before/after comparison)
pub fn list_directory_files(dir: &Path) -> io::Result<Vec<String>> {
    let mut files = Vec::new();

    if !dir.exists() {
        return Ok(files);
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.path().is_file() {
            if let Some(filename) = entry.file_name().to_str() {
                files.push(filename.to_string());
            }
        }
    }

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect_new_files() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("old.txt"), b"old").unwrap();
        fs::write(temp_dir.path().join("new.txt"), b"new").unwrap();

        let existing = vec!["old.txt".to_string()];
        let new_files = detect_new_files(temp_dir.path(), &existing).unwrap();

        assert_eq!(new_files.len(), 1);
        assert!(new_files[0].file_name().unwrap() == "new.txt");
    }

    #[test]
    fn test_detect_crate_outputs() {
        let temp_dir = TempDir::new().unwrap();

        fs::write(temp_dir.path().join("libmylib-abc123.rlib"), b"").unwrap();
        fs::write(temp_dir.path().join("libmylib-abc123.d"), b"").unwrap();
        fs::write(temp_dir.path().join("other.txt"), b"").unwrap();

        let outputs = detect_crate_outputs(temp_dir.path(), "mylib").unwrap();

        assert_eq!(outputs.len(), 2);
        assert!(!outputs.iter().any(|f| f.file_name().unwrap() == "other.txt"));
    }
}
