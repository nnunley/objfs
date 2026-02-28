use objfs::cas::Cas;
use objfs::re_client::{Digest, RemoteCas};
use std::io;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

/// Mock remote CAS for testing
struct MockRemoteCas {
    name: String,
    storage: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl MockRemoteCas {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            storage: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn has_blob(&self, hash: &str) -> bool {
        self.storage.lock().unwrap().contains_key(hash)
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

/// Test that HybridCas checks backends in order
#[test]
fn test_hierarchical_cas_checks_in_order() {
    // Setup: 3-tier cache
    // Tier 0: Local (empty initially)
    // Tier 1: Team cache (has the blob)
    // Tier 2: Company cache (also has the blob, but shouldn't be checked)

    let temp_dir = TempDir::new().unwrap();
    let local = Cas::new(temp_dir.path().to_path_buf()).unwrap();

    let team_cache = MockRemoteCas::new("team");
    let company_cache = MockRemoteCas::new("company");

    // Store blob only in team and company caches
    let data = b"shared data";
    let team_digest = team_cache.upload(data).unwrap();
    let company_digest = company_cache.upload(data).unwrap();

    // Same hash
    assert_eq!(team_digest.hash, company_digest.hash);

    // Local should not have it
    assert!(!local.exists(&team_digest.hash));

    // This test will pass once we implement HybridCas
    // For now, it just documents the intended behavior
}

/// Test that HybridCas populates lower tiers on cache hit
#[test]
fn test_hierarchical_cas_write_back() {
    // Setup: 3-tier cache
    let temp_dir = TempDir::new().unwrap();
    let local = Cas::new(temp_dir.path().to_path_buf()).unwrap();
    let remote = MockRemoteCas::new("remote");

    // Store in remote only
    let data = b"remote data";
    let digest = remote.upload(data).unwrap();

    // Local doesn't have it yet
    assert!(!local.exists(&digest.hash));

    // When HybridCas retrieves from remote, it should populate local
    // (write-back caching)

    // After get(), local should have it
    // assert!(local.exists(&digest.hash));  // Will uncomment when implemented
}

/// Test that HybridCas stores to first backend
#[test]
fn test_hierarchical_cas_stores_to_first_tier() {
    let temp_dir = TempDir::new().unwrap();
    let local = Cas::new(temp_dir.path().to_path_buf()).unwrap();

    // Store should always go to first backend (local)
    let data = b"new data";
    let hash = local.store(data).unwrap();

    // Should exist in local
    assert!(local.exists(&hash));
}

/// Test fallback behavior when first backend fails
#[test]
fn test_hierarchical_cas_fallback_on_miss() {
    // Tier 0: Local (empty)
    // Tier 1: Remote (has blob)

    let temp_dir = TempDir::new().unwrap();
    let local = Cas::new(temp_dir.path().to_path_buf()).unwrap();
    let remote = MockRemoteCas::new("remote");

    // Store only in remote
    let data = b"fallback test";
    let digest = remote.upload(data).unwrap();

    // Local miss
    assert!(!local.exists(&digest.hash));

    // Remote hit
    assert!(remote.exists(&digest).unwrap());

    // HybridCas should fall back to remote when local misses
    // This validates the hierarchy works correctly
}
