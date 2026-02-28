use objfs::cas::Cas;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

#[test]
fn test_preserves_executable_permissions() {
    let temp_dir = TempDir::new().unwrap();
    let cas = Cas::new(temp_dir.path().join("cas")).unwrap();

    // Create an executable file
    let source = temp_dir.path().join("executable");
    fs::write(&source, b"#!/bin/bash\necho hello").unwrap();
    let mut perms = fs::metadata(&source).unwrap().permissions();
    perms.set_mode(0o755); // rwxr-xr-x
    fs::set_permissions(&source, perms).unwrap();

    // Store in CAS
    let hash = cas.store_file(&source).unwrap();

    // Restore to new location
    let dest = temp_dir.path().join("restored");
    cas.get_to_file(&hash, &dest).unwrap();

    // Check permissions are preserved
    let restored_perms = fs::metadata(&dest).unwrap().permissions();
    assert_eq!(restored_perms.mode() & 0o777, 0o755);
}

#[test]
fn test_preserves_readonly_permissions() {
    let temp_dir = TempDir::new().unwrap();
    let cas = Cas::new(temp_dir.path().join("cas")).unwrap();

    // Create a readonly file
    let source = temp_dir.path().join("readonly");
    fs::write(&source, b"some data").unwrap();
    let mut perms = fs::metadata(&source).unwrap().permissions();
    perms.set_mode(0o644); // rw-r--r--
    fs::set_permissions(&source, perms).unwrap();

    // Store in CAS
    let hash = cas.store_file(&source).unwrap();

    // Restore to new location
    let dest = temp_dir.path().join("restored");
    cas.get_to_file(&hash, &dest).unwrap();

    // Check permissions are preserved
    let restored_perms = fs::metadata(&dest).unwrap().permissions();
    assert_eq!(restored_perms.mode() & 0o777, 0o644);
}
