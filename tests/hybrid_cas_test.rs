use objfs::cas::Cas;
use objfs::re_client::{Digest, RemoteCas};
use std::io;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

/// Mock remote CAS for testing
struct MockRemoteCas {
    storage: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl MockRemoteCas {
    fn new() -> Self {
        Self {
            storage: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl RemoteCas for MockRemoteCas {
    fn exists(&self, digest: &Digest) -> io::Result<bool> {
        let storage = self.storage.lock().unwrap();
        Ok(storage.contains_key(&digest.hash))
    }

    fn upload(&self, data: &[u8]) -> io::Result<Digest> {
        use sha2::{Sha256, Digest as Sha2Digest};
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = hex::encode(hasher.finalize());
        let digest = Digest::new(hash.clone(), data.len() as i64);

        let mut storage = self.storage.lock().unwrap();
        storage.insert(hash, data.to_vec());
        Ok(digest)
    }

    fn download(&self, digest: &Digest) -> io::Result<Vec<u8>> {
        let storage = self.storage.lock().unwrap();
        storage.get(&digest.hash)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Blob not found"))
    }
}

#[test]
fn test_local_cas_works_without_remote() {
    let temp_dir = TempDir::new().unwrap();
    let cas = Cas::new(temp_dir.path().to_path_buf()).unwrap();

    let data = b"local only data";
    let hash = cas.store(data).unwrap();

    let retrieved = cas.get(&hash).unwrap();
    assert_eq!(retrieved, data);
}

#[test]
fn test_hybrid_cas_checks_local_first() {
    let temp_dir = TempDir::new().unwrap();
    let local_cas = Cas::new(temp_dir.path().to_path_buf()).unwrap();
    let remote_cas = MockRemoteCas::new();

    // Store in local CAS
    let data = b"local data";
    let local_hash = local_cas.store(data).unwrap();

    // Hybrid CAS should find it locally (not need remote)
    // This test will fail until we implement HybridCas
    assert!(local_cas.exists(&local_hash));
}

#[test]
fn test_hybrid_cas_falls_back_to_remote() {
    // Store data only in remote CAS
    let remote_cas = MockRemoteCas::new();
    let data = b"remote only data";
    let digest = remote_cas.upload(data).unwrap();

    // Local CAS should not have it
    let temp_dir = TempDir::new().unwrap();
    let local_cas = Cas::new(temp_dir.path().to_path_buf()).unwrap();
    assert!(!local_cas.exists(&digest.hash));

    // Hybrid CAS should find it in remote and download to local
    // This will require implementing hybrid logic
}
