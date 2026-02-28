use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

use objfs::cas::Cas;

// This will test the full cache workflow:
// 1. First compile: cache miss, store artifact
// 2. Second compile: cache hit, retrieve artifact

#[test]
fn test_full_cache_miss_then_hit_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let cas_dir = temp_dir.path().join("cas");
    let cas = Cas::new(cas_dir.clone()).unwrap();

    // Simulate first compilation
    let output_file = temp_dir.path().join("output.rlib");
    let artifact_content = b"compiled artifact data";

    // First time: cache miss - we compile and store
    fs::write(&output_file, artifact_content).unwrap();
    let hash = cas.store_file(&output_file).unwrap();

    // Store cache entry
    let cache_key = "test_cache_key_123";
    store_cache_index(&cas_dir, cache_key, &hash).unwrap();

    // Delete the output file (simulating cargo clean)
    fs::remove_file(&output_file).unwrap();
    assert!(!output_file.exists());

    // Second time: cache hit - retrieve from CAS
    let retrieved_hash = lookup_cache_index(&cas_dir, cache_key).unwrap();
    assert_eq!(retrieved_hash, Some(hash.clone()));

    // Restore output file from CAS
    cas.get_to_file(&hash, &output_file).unwrap();

    // Verify restored content matches
    let restored_content = fs::read(&output_file).unwrap();
    assert_eq!(restored_content, artifact_content);
}

#[test]
fn test_cache_miss_when_key_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let cas_dir = temp_dir.path().join("cas");
    let _cas = Cas::new(cas_dir.clone()).unwrap();

    // Try to lookup non-existent key
    let result = lookup_cache_index(&cas_dir, "non_existent_key").unwrap();

    assert_eq!(result, None);
}

#[test]
fn test_cache_miss_when_hash_not_in_cas() {
    let temp_dir = TempDir::new().unwrap();
    let cas_dir = temp_dir.path().join("cas");
    let cas = Cas::new(cas_dir.clone()).unwrap();

    let fake_hash = "deadbeefcafe";
    let cache_key = "test_key";

    // Store index entry pointing to non-existent hash
    store_cache_index(&cas_dir, cache_key, fake_hash).unwrap();

    // Lookup should find the key
    let result = lookup_cache_index(&cas_dir, cache_key).unwrap();
    assert_eq!(result, Some(fake_hash.to_string()));

    // But hash doesn't exist in CAS
    assert!(!cas.exists(&fake_hash.to_string()));
}

#[test]
fn test_multiple_cache_entries() {
    let temp_dir = TempDir::new().unwrap();
    let cas_dir = temp_dir.path().join("cas");
    let cas = Cas::new(cas_dir.clone()).unwrap();

    // Store multiple different artifacts
    let artifact1 = b"artifact one";
    let artifact2 = b"artifact two";

    let hash1 = cas.store(artifact1).unwrap();
    let hash2 = cas.store(artifact2).unwrap();

    store_cache_index(&cas_dir, "key1", &hash1).unwrap();
    store_cache_index(&cas_dir, "key2", &hash2).unwrap();

    // Both should be retrievable
    assert_eq!(lookup_cache_index(&cas_dir, "key1").unwrap(), Some(hash1.clone()));
    assert_eq!(lookup_cache_index(&cas_dir, "key2").unwrap(), Some(hash2.clone()));

    // Verify we can retrieve the artifacts
    let retrieved1 = cas.get(&hash1).unwrap();
    let retrieved2 = cas.get(&hash2).unwrap();

    assert_eq!(retrieved1, artifact1);
    assert_eq!(retrieved2, artifact2);
}

// Helper functions implementing cache index

fn store_cache_index(cas_dir: &PathBuf, cache_key: &str, hash: &str) -> std::io::Result<()> {
    let cache_index = cas_dir.join("index.json");

    // Load existing index or create new
    let mut index: serde_json::Map<String, serde_json::Value> = if cache_index.exists() {
        let data = fs::read_to_string(&cache_index)?;
        serde_json::from_str(&data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
    } else {
        serde_json::Map::new()
    };

    // Store entry
    index.insert(cache_key.to_string(), serde_json::Value::String(hash.to_string()));

    // Write back
    let data = serde_json::to_string_pretty(&index)?;
    fs::write(cache_index, data)?;

    Ok(())
}

fn lookup_cache_index(cas_dir: &PathBuf, cache_key: &str) -> std::io::Result<Option<String>> {
    let cache_index = cas_dir.join("index.json");

    if !cache_index.exists() {
        return Ok(None);
    }

    let data = fs::read_to_string(cache_index)?;
    let index: serde_json::Value = serde_json::from_str(&data)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    Ok(index.get(cache_key).and_then(|v| v.as_str()).map(String::from))
}
