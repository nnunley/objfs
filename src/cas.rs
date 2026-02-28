use sha2::{Sha256, Digest};
use std::path::{Path, PathBuf};
use std::fs;
use std::io::{self, Write};
use serde::{Deserialize, Serialize};
use std::os::unix::fs::PermissionsExt;

/// Content hash type (SHA256)
pub type Hash = String;

/// Content-Addressed Storage
#[derive(Clone)]
pub struct Cas {
    root: PathBuf,
}

impl Cas {
    /// Get the root directory of the CAS
    pub fn root(&self) -> &PathBuf {
        &self.root
    }

    /// Get path for metadata file
    fn metadata_path(&self, hash: &Hash) -> PathBuf {
        self.object_path(hash).with_extension("meta")
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    /// Map of project-relative paths to content hashes
    pub entries: std::collections::HashMap<PathBuf, Hash>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FileMetadata {
    /// Unix file permissions (mode)
    mode: u32,
}

impl Cas {
    pub fn new(root: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&root)?;
        fs::create_dir_all(root.join("objects"))?;
        Ok(Self { root })
    }

    /// Get default CAS location
    pub fn default_location() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".cache")
            .join("objfs")
            .join("cas")
    }

    /// Compute SHA256 hash of data
    pub fn hash_data(data: &[u8]) -> Hash {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    /// Get path for a given hash in CAS
    pub fn object_path(&self, hash: &Hash) -> PathBuf {
        // Use git-style sharding: first 2 chars as directory
        let (prefix, suffix) = hash.split_at(2);
        self.root.join("objects").join(prefix).join(suffix)
    }

    /// Check if object exists in CAS
    pub fn exists(&self, hash: &Hash) -> bool {
        self.object_path(hash).exists()
    }

    /// Store data in CAS, return its hash
    pub fn store(&self, data: &[u8]) -> io::Result<Hash> {
        let hash = Self::hash_data(data);

        // Skip if already exists
        if self.exists(&hash) {
            return Ok(hash);
        }

        let path = self.object_path(&hash);

        // Create parent directory
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write atomically via temp file
        let temp_path = path.with_extension("tmp");
        let mut file = fs::File::create(&temp_path)?;
        file.write_all(data)?;
        file.sync_all()?;

        // Rename to final location
        fs::rename(temp_path, path)?;

        Ok(hash)
    }

    /// Store file in CAS
    pub fn store_file(&self, path: &Path) -> io::Result<Hash> {
        let data = fs::read(path)?;
        let hash = self.store(&data)?;

        // Store metadata (permissions)
        let metadata = fs::metadata(path)?;
        let file_meta = FileMetadata {
            mode: metadata.permissions().mode(),
        };
        let meta_json = serde_json::to_string(&file_meta)?;
        let meta_path = self.metadata_path(&hash);

        if let Some(parent) = meta_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(meta_path, meta_json)?;

        Ok(hash)
    }

    /// Retrieve data from CAS
    pub fn get(&self, hash: &Hash) -> io::Result<Vec<u8>> {
        let path = self.object_path(hash);
        fs::read(path)
    }

    /// Retrieve data and write to file
    pub fn get_to_file(&self, hash: &Hash, dest: &Path) -> io::Result<()> {
        let data = self.get(hash)?;

        // Create parent directory
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(dest, data)?;

        // Restore permissions if metadata exists
        let meta_path = self.metadata_path(hash);
        if meta_path.exists() {
            let meta_json = fs::read_to_string(meta_path)?;
            let file_meta: FileMetadata = serde_json::from_str(&meta_json)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            let mut perms = fs::metadata(dest)?.permissions();
            perms.set_mode(file_meta.mode);
            fs::set_permissions(dest, perms)?;
        }

        Ok(())
    }

    /// Get CAS statistics
    pub fn stats(&self) -> io::Result<CasStats> {
        let mut total_size = 0u64;
        let mut object_count = 0u64;

        let objects_dir = self.root.join("objects");
        if objects_dir.exists() {
            for entry in walkdir::WalkDir::new(objects_dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| !e.path().extension().map_or(false, |ext| ext == "meta"))
            {
                if let Ok(metadata) = entry.metadata() {
                    total_size += metadata.len();
                    object_count += 1;
                }
            }
        }

        Ok(CasStats {
            object_count,
            total_size,
        })
    }
}

#[derive(Debug)]
pub struct CasStats {
    pub object_count: u64,
    pub total_size: u64,
}

impl Manifest {
    pub fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
        }
    }

    pub fn load(path: &Path) -> io::Result<Self> {
        let data = fs::read_to_string(path)?;
        serde_json::from_str(&data)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let data = serde_json::to_string_pretty(self)?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(path, data)?;
        Ok(())
    }

    pub fn add(&mut self, path: PathBuf, hash: Hash) {
        self.entries.insert(path, hash);
    }

    pub fn get(&self, path: &Path) -> Option<&Hash> {
        self.entries.get(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_cas_store_and_retrieve() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cas = Cas::new(temp_dir.path().to_path_buf()).unwrap();

        let data = b"hello world";
        let hash = cas.store(data).unwrap();

        let retrieved = cas.get(&hash).unwrap();
        assert_eq!(data, retrieved.as_slice());
    }

    #[test]
    fn test_cas_deduplication() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cas = Cas::new(temp_dir.path().to_path_buf()).unwrap();

        let data = b"hello world";
        let hash1 = cas.store(data).unwrap();
        let hash2 = cas.store(data).unwrap();

        assert_eq!(hash1, hash2);

        let stats = cas.stats().unwrap();
        assert_eq!(stats.object_count, 1);
    }
}
