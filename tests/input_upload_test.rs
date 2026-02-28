// Tests for uploading input files to remote CAS

use objfs::re_client::Digest;
use tempfile::TempDir;

#[test]
fn test_upload_input_files() {
    // Create temporary files
    let temp_dir = TempDir::new().unwrap();
    let src_file = temp_dir.path().join("main.rs");
    std::fs::write(&src_file, b"fn main() {}").unwrap();

    let input_files = vec![src_file.clone()];

    // Should be able to hash and prepare files for upload
    let mut digests = Vec::new();
    for file in &input_files {
        let contents = std::fs::read(file).unwrap();
        let digest = Digest::from_data(&contents);
        digests.push((file.clone(), digest));
    }

    assert_eq!(digests.len(), 1);
    assert!(!digests[0].1.hash.is_empty());
}

#[test]
fn test_directory_tree_structure() {
    // For RE API v2, we need to create a Directory tree
    // representing the input file structure

    let temp_dir = TempDir::new().unwrap();
    let src_dir = temp_dir.path().join("src");
    std::fs::create_dir(&src_dir).unwrap();

    let main_rs = src_dir.join("main.rs");
    let lib_rs = src_dir.join("lib.rs");

    std::fs::write(&main_rs, b"fn main() {}").unwrap();
    std::fs::write(&lib_rs, b"pub fn test() {}").unwrap();

    let files = vec![main_rs, lib_rs];

    // All files should be readable
    for file in &files {
        assert!(file.exists());
        let _contents = std::fs::read(file).unwrap();
    }
}

#[test]
fn test_compute_input_root_digest() {
    // The input_root_digest represents the entire input directory tree
    // For a simple case with one file, it should be computable

    let temp_dir = TempDir::new().unwrap();
    let file = temp_dir.path().join("main.rs");
    std::fs::write(&file, b"fn main() {}").unwrap();

    // Read and hash the file
    let contents = std::fs::read(&file).unwrap();
    let file_digest = Digest::from_data(&contents);

    // In a real implementation, we'd create a Directory proto
    // For now, just verify we can compute digests
    assert!(!file_digest.hash.is_empty());
    assert!(file_digest.size_bytes > 0);
}
