use objfs::bundle::ArtifactBundle;
use objfs::cas::Cas;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_bundle_restore_fails_gracefully_when_object_missing() {
    let temp_dir = TempDir::new().unwrap();
    let cas = Cas::new(temp_dir.path().join("cas")).unwrap();

    let output_dir = temp_dir.path().join("output");
    fs::create_dir_all(&output_dir).unwrap();

    // Create and store a bundle
    let file1 = output_dir.join("file1.txt");
    let file2 = output_dir.join("file2.txt");
    fs::write(&file1, b"content 1").unwrap();
    fs::write(&file2, b"content 2").unwrap();

    let bundle = ArtifactBundle::new(vec![file1.clone(), file2.clone()]);
    let bundle_hash = bundle.store(&cas).unwrap();

    // Delete one of the object files (simulating partial expiration)
    let objects = cas.list_all_objects().unwrap();
    assert!(objects.len() >= 2);

    // Delete first object file
    let hash_to_delete = &objects[0];
    let object_path = cas.object_path(hash_to_delete);
    fs::remove_file(object_path).unwrap();

    // Try to restore - should fail with clear error
    let restore_dir = temp_dir.path().join("restored");
    fs::create_dir_all(&restore_dir).unwrap();

    let result = ArtifactBundle::restore(&cas, &bundle_hash, &restore_dir);

    // Should fail because object is missing
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn test_bundle_can_check_if_complete() {
    let temp_dir = TempDir::new().unwrap();
    let cas = Cas::new(temp_dir.path().join("cas")).unwrap();

    let output_dir = temp_dir.path().join("output");
    fs::create_dir_all(&output_dir).unwrap();

    // Create and store a bundle
    let file1 = output_dir.join("file1.txt");
    let file2 = output_dir.join("file2.txt");
    fs::write(&file1, b"content 1").unwrap();
    fs::write(&file2, b"content 2").unwrap();

    let bundle = ArtifactBundle::new(vec![file1.clone(), file2.clone()]);
    let bundle_hash = bundle.store(&cas).unwrap();

    // All objects exist - should be complete
    assert!(ArtifactBundle::is_complete(&cas, &bundle_hash).unwrap());

    // Delete one object
    let objects = cas.list_all_objects().unwrap();
    let hash_to_delete = &objects[0];
    let object_path = cas.object_path(hash_to_delete);
    fs::remove_file(object_path).unwrap();

    // Now bundle is incomplete
    assert!(!ArtifactBundle::is_complete(&cas, &bundle_hash).unwrap());
}
