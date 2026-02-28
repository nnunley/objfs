use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

use objfs::cas::Cas;

#[test]
fn test_cache_stores_and_retrieves_identical_content() {
    let temp_dir = TempDir::new().unwrap();
    let cas = Cas::new(temp_dir.path().to_path_buf()).unwrap();

    // Store some content
    let content = b"hello world";
    let hash = cas.store(content).unwrap();

    // Retrieve it
    let retrieved = cas.get(&hash).unwrap();

    assert_eq!(content.as_slice(), retrieved.as_slice());
}

#[test]
fn test_identical_content_produces_same_hash() {
    let temp_dir = TempDir::new().unwrap();
    let cas = Cas::new(temp_dir.path().to_path_buf()).unwrap();

    let content = b"test content";

    let hash1 = cas.store(content).unwrap();
    let hash2 = cas.store(content).unwrap();

    assert_eq!(hash1, hash2);
}

#[test]
fn test_different_content_produces_different_hash() {
    let temp_dir = TempDir::new().unwrap();
    let cas = Cas::new(temp_dir.path().to_path_buf()).unwrap();

    let content1 = b"content one";
    let content2 = b"content two";

    let hash1 = cas.store(content1).unwrap();
    let hash2 = cas.store(content2).unwrap();

    assert_ne!(hash1, hash2);
}

#[test]
fn test_cas_deduplicates_identical_files() {
    let temp_dir = TempDir::new().unwrap();
    let cas = Cas::new(temp_dir.path().to_path_buf()).unwrap();

    // Create two files with identical content
    let file1 = temp_dir.path().join("file1.txt");
    let file2 = temp_dir.path().join("file2.txt");

    fs::write(&file1, b"same content").unwrap();
    fs::write(&file2, b"same content").unwrap();

    // Store both
    let hash1 = cas.store_file(&file1).unwrap();
    let hash2 = cas.store_file(&file2).unwrap();

    // Should produce same hash
    assert_eq!(hash1, hash2);

    // Should only store once
    let stats = cas.stats().unwrap();
    assert_eq!(stats.object_count, 1);
}

#[test]
fn test_cache_key_stability() {
    // This test verifies that the cache key computation is deterministic
    // Same inputs should always produce the same cache key

    let temp_dir = TempDir::new().unwrap();

    // Create test input file
    let src_file = temp_dir.path().join("src.rs");
    fs::write(&src_file, b"fn main() {}").unwrap();

    // Compute cache key twice with same inputs
    let key1 = compute_test_cache_key(&src_file, &["--edition", "2024"]);
    let key2 = compute_test_cache_key(&src_file, &["--edition", "2024"]);

    assert_eq!(key1, key2);
}

#[test]
fn test_cache_key_changes_with_source() {
    let temp_dir = TempDir::new().unwrap();

    let src_file = temp_dir.path().join("src.rs");

    // Write initial content
    fs::write(&src_file, b"fn main() {}").unwrap();
    let key1 = compute_test_cache_key(&src_file, &["--edition", "2024"]);

    // Modify source
    fs::write(&src_file, b"fn main() { println!(\"hello\"); }").unwrap();
    let key2 = compute_test_cache_key(&src_file, &["--edition", "2024"]);

    assert_ne!(key1, key2);
}

#[test]
fn test_cache_key_changes_with_flags() {
    let temp_dir = TempDir::new().unwrap();

    let src_file = temp_dir.path().join("src.rs");
    fs::write(&src_file, b"fn main() {}").unwrap();

    let key1 = compute_test_cache_key(&src_file, &["--edition", "2024"]);
    let key2 = compute_test_cache_key(&src_file, &["--edition", "2021"]);

    assert_ne!(key1, key2);
}

// Helper function to compute cache key (mirrors rustc_wrapper logic)
fn compute_test_cache_key(input_file: &PathBuf, flags: &[&str]) -> String {
    use sha2::{Sha256, Digest};

    let mut hasher = Sha256::new();

    // Hash input file
    if input_file.exists() {
        let data = fs::read(input_file).unwrap();
        hasher.update(&data);
    }

    // Hash flags (sorted for stability)
    let mut sorted_flags: Vec<String> = flags.iter().map(|s| s.to_string()).collect();
    sorted_flags.sort();
    for flag in sorted_flags {
        hasher.update(flag.as_bytes());
        hasher.update(b"\0");
    }

    hex::encode(hasher.finalize())
}
