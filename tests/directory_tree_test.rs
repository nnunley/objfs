use objfs::directory_tree::DirectoryTreeBuilder;
use tempfile::TempDir;

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

#[test]
fn test_files_sorted_lexicographically() {
    let temp = TempDir::new().unwrap();
    let file_z = temp.path().join("zebra.rs");
    let file_a = temp.path().join("apple.rs");
    let file_m = temp.path().join("mango.rs");

    std::fs::write(&file_z, b"// zebra").unwrap();
    std::fs::write(&file_a, b"// apple").unwrap();
    std::fs::write(&file_m, b"// mango").unwrap();

    let builder = DirectoryTreeBuilder::new();
    let files = vec![file_z, file_a, file_m]; // Unsorted input
    let directory = builder.build(&files).unwrap();

    assert_eq!(directory.files.len(), 3);
    // Verify lexicographic ordering
    assert_eq!(directory.files[0].name, "apple.rs");
    assert_eq!(directory.files[1].name, "mango.rs");
    assert_eq!(directory.files[2].name, "zebra.rs");
}

#[test]
fn test_directory_serialization() {
    use objfs::directory_tree::{Directory, FileNode};
    use objfs::re_client::Digest;

    let directory = Directory {
        files: vec![FileNode {
            name: "main.rs".to_string(),
            digest: Digest::new("abc123".to_string(), 100),
            is_executable: false,
        }],
        directories: vec![],
    };

    let proto_bytes = directory.to_proto_bytes().unwrap();
    assert!(!proto_bytes.is_empty());
}
