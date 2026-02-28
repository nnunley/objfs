use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

use objfs::cas::Cas;

#[test]
fn test_store_multiple_artifacts() {
    let temp_dir = TempDir::new().unwrap();
    let cas = Cas::new(temp_dir.path().to_path_buf()).unwrap();

    // Simulate rustc producing multiple output files
    let output_dir = temp_dir.path().join("output");
    fs::create_dir_all(&output_dir).unwrap();

    let rlib_file = output_dir.join("libmylib.rlib");
    let dep_file = output_dir.join("libmylib.d");
    let rmeta_file = output_dir.join("libmylib.rmeta");

    fs::write(&rlib_file, b"rlib content").unwrap();
    fs::write(&dep_file, b"dependency info").unwrap();
    fs::write(&rmeta_file, b"metadata").unwrap();

    // Store all artifacts as a bundle
    let artifact_bundle = ArtifactBundle::new(vec![
        rlib_file.clone(),
        dep_file.clone(),
        rmeta_file.clone(),
    ]);

    let bundle_hash = store_artifact_bundle(&cas, &artifact_bundle).unwrap();

    // Verify bundle was stored
    assert!(!bundle_hash.is_empty());
}

#[test]
fn test_retrieve_multiple_artifacts() {
    let temp_dir = TempDir::new().unwrap();
    let cas = Cas::new(temp_dir.path().to_path_buf()).unwrap();

    // Create and store artifacts
    let output_dir = temp_dir.path().join("output");
    fs::create_dir_all(&output_dir).unwrap();

    let rlib_file = output_dir.join("libmylib.rlib");
    let dep_file = output_dir.join("libmylib.d");

    fs::write(&rlib_file, b"rlib content").unwrap();
    fs::write(&dep_file, b"dependency info").unwrap();

    let artifact_bundle = ArtifactBundle::new(vec![
        rlib_file.clone(),
        dep_file.clone(),
    ]);

    let bundle_hash = store_artifact_bundle(&cas, &artifact_bundle).unwrap();

    // Delete the files
    fs::remove_file(&rlib_file).unwrap();
    fs::remove_file(&dep_file).unwrap();

    // Restore from bundle
    restore_artifact_bundle(&cas, &bundle_hash, &output_dir).unwrap();

    // Verify all files restored
    assert!(rlib_file.exists());
    assert!(dep_file.exists());

    assert_eq!(fs::read(&rlib_file).unwrap(), b"rlib content");
    assert_eq!(fs::read(&dep_file).unwrap(), b"dependency info");
}

#[test]
fn test_bundle_deduplication() {
    let temp_dir = TempDir::new().unwrap();
    let cas = Cas::new(temp_dir.path().to_path_buf()).unwrap();

    // Create two bundles with same files
    let output_dir = temp_dir.path().join("output");
    fs::create_dir_all(&output_dir).unwrap();

    let file1 = output_dir.join("file1.txt");
    let file2 = output_dir.join("file2.txt");

    fs::write(&file1, b"content one").unwrap();
    fs::write(&file2, b"content two").unwrap();

    let bundle1 = ArtifactBundle::new(vec![file1.clone(), file2.clone()]);
    let bundle2 = ArtifactBundle::new(vec![file1.clone(), file2.clone()]);

    let hash1 = store_artifact_bundle(&cas, &bundle1).unwrap();
    let hash2 = store_artifact_bundle(&cas, &bundle2).unwrap();

    // Same files should produce same bundle hash
    assert_eq!(hash1, hash2);
}

// Structures and functions to be implemented

#[derive(Debug)]
struct ArtifactBundle {
    files: Vec<PathBuf>,
}

impl ArtifactBundle {
    fn new(files: Vec<PathBuf>) -> Self {
        Self { files }
    }
}

fn store_artifact_bundle(cas: &Cas, bundle: &ArtifactBundle) -> std::io::Result<String> {
    use sha2::{Sha256, Digest};
    use serde::{Serialize, Deserialize};

    #[derive(Serialize, Deserialize)]
    struct BundleManifest {
        files: Vec<FileEntry>,
    }

    #[derive(Serialize, Deserialize)]
    struct FileEntry {
        path: String,
        hash: String,
    }

    // Store each file in CAS and build manifest
    let mut file_entries = Vec::new();
    let mut bundle_hasher = Sha256::new();

    for file_path in &bundle.files {
        let hash = cas.store_file(file_path)?;

        // Use relative filename
        let filename = file_path.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid filename"))?;

        file_entries.push(FileEntry {
            path: filename.to_string(),
            hash: hash.clone(),
        });

        // Contribute to bundle hash
        bundle_hasher.update(filename.as_bytes());
        bundle_hasher.update(b"\0");
        bundle_hasher.update(hash.as_bytes());
    }

    // Create manifest
    let manifest = BundleManifest { files: file_entries };
    let manifest_json = serde_json::to_string(&manifest)?;

    // Store manifest in CAS
    let manifest_hash = cas.store(manifest_json.as_bytes())?;

    Ok(manifest_hash)
}

fn restore_artifact_bundle(cas: &Cas, bundle_hash: &str, output_dir: &Path) -> std::io::Result<()> {
    use serde::{Deserialize};

    #[derive(Deserialize)]
    struct BundleManifest {
        files: Vec<FileEntry>,
    }

    #[derive(Deserialize)]
    struct FileEntry {
        path: String,
        hash: String,
    }

    // Retrieve manifest
    let manifest_data = cas.get(&bundle_hash.to_string())?;
    let manifest_json = String::from_utf8(manifest_data)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let manifest: BundleManifest = serde_json::from_str(&manifest_json)?;

    // Restore each file
    for entry in manifest.files {
        let output_path = output_dir.join(&entry.path);
        cas.get_to_file(&entry.hash, &output_path)?;
    }

    Ok(())
}
