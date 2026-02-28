use std::path::{Path, PathBuf};
use std::io;
use serde::{Serialize, Deserialize};
use crate::cas::Cas;

#[derive(Debug, Serialize, Deserialize)]
pub struct ArtifactBundle {
    pub files: Vec<PathBuf>,
}

#[derive(Serialize, Deserialize)]
struct BundleManifest {
    files: Vec<FileEntry>,
}

#[derive(Serialize, Deserialize)]
struct FileEntry {
    path: String,
    hash: String,
}

impl ArtifactBundle {
    pub fn new(files: Vec<PathBuf>) -> Self {
        Self { files }
    }

    pub fn store(&self, cas: &Cas) -> io::Result<String> {
        use sha2::{Sha256, Digest};

        // Store each file in CAS and build manifest
        let mut file_entries = Vec::new();
        let mut bundle_hasher = Sha256::new();

        for file_path in &self.files {
            if !file_path.exists() {
                continue; // Skip non-existent files
            }

            let hash = cas.store_file(file_path)?;

            // Use relative filename
            let filename = file_path.file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid filename"))?;

            file_entries.push(FileEntry {
                path: filename.to_string(),
                hash: hash.clone(),
            });

            // Contribute to bundle hash
            bundle_hasher.update(filename.as_bytes());
            bundle_hasher.update(b"\0");
            bundle_hasher.update(hash.as_bytes());
        }

        if file_entries.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "No files to bundle"));
        }

        // Create manifest
        let manifest = BundleManifest { files: file_entries };
        let manifest_json = serde_json::to_string(&manifest)?;

        // Store manifest in CAS
        let manifest_hash = cas.store(manifest_json.as_bytes())?;

        Ok(manifest_hash)
    }

    /// Check if all files in a bundle exist in CAS
    pub fn is_complete(cas: &Cas, bundle_hash: &str) -> io::Result<bool> {
        // Retrieve manifest
        let manifest_data = cas.get(&bundle_hash.to_string())?;
        let manifest_json = String::from_utf8(manifest_data)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let manifest: BundleManifest = serde_json::from_str(&manifest_json)?;

        // Check if all files exist
        for entry in manifest.files {
            if !cas.exists(&entry.hash) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    pub fn restore(cas: &Cas, bundle_hash: &str, output_dir: &Path) -> io::Result<()> {
        // Retrieve manifest
        let manifest_data = cas.get(&bundle_hash.to_string())?;
        let manifest_json = String::from_utf8(manifest_data)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let manifest: BundleManifest = serde_json::from_str(&manifest_json)?;

        // Restore each file
        for entry in manifest.files {
            let output_path = output_dir.join(&entry.path);
            cas.get_to_file(&entry.hash, &output_path)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_bundle_store_and_restore() {
        let temp_dir = TempDir::new().unwrap();
        let cas = Cas::new(temp_dir.path().join("cas")).unwrap();

        let output_dir = temp_dir.path().join("output");
        fs::create_dir_all(&output_dir).unwrap();

        let file1 = output_dir.join("file1.txt");
        let file2 = output_dir.join("file2.txt");

        fs::write(&file1, b"content 1").unwrap();
        fs::write(&file2, b"content 2").unwrap();

        let bundle = ArtifactBundle::new(vec![file1.clone(), file2.clone()]);
        let hash = bundle.store(&cas).unwrap();

        // Delete files
        fs::remove_file(&file1).unwrap();
        fs::remove_file(&file2).unwrap();

        // Restore
        ArtifactBundle::restore(&cas, &hash, &output_dir).unwrap();

        assert_eq!(fs::read(&file1).unwrap(), b"content 1");
        assert_eq!(fs::read(&file2).unwrap(), b"content 2");
    }
}
