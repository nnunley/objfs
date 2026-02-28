use objfs::directory_tree::DirectoryTreeBuilder;
use tempfile::TempDir;
use std::path::PathBuf;

#[test]
fn test_build_directory_with_single_file() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("main.rs");
    std::fs::write(&file, b"fn main() {}").unwrap();

    let builder = DirectoryTreeBuilder::new();
    let files = vec![file];
    let directory = builder.build(&files).unwrap();

    assert_eq!(directory.files.len(), 1);
    assert_eq!(directory.files[0].name, "main.rs");
    assert!(!directory.files[0].digest.hash.is_empty());
}
