use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, Duration};
use tempfile::TempDir;

use objfs::cas::Cas;

#[test]
fn test_track_object_access_time() {
    let temp_dir = TempDir::new().unwrap();
    let cas = Cas::new(temp_dir.path().to_path_buf()).unwrap();

    // Store an object
    let content = b"test content";
    let hash = cas.store(content).unwrap();

    // Get access metadata
    let metadata = get_object_metadata(&cas, &hash).unwrap();

    assert!(metadata.last_accessed.is_some());
    assert!(metadata.created.is_some());
}

#[test]
fn test_update_access_time_on_get() {
    let temp_dir = TempDir::new().unwrap();
    let cas = Cas::new(temp_dir.path().to_path_buf()).unwrap();

    let content = b"test content";
    let hash = cas.store(content).unwrap();

    // Wait a bit
    std::thread::sleep(Duration::from_millis(100));

    // Access the object
    let _ = cas.get(&hash).unwrap();

    // Check access time was updated
    let metadata = get_object_metadata(&cas, &hash).unwrap();
    let accessed_age = SystemTime::now()
        .duration_since(metadata.last_accessed.unwrap())
        .unwrap();

    // Should be accessed very recently (within 1 second)
    assert!(accessed_age < Duration::from_secs(1));
}

#[test]
fn test_find_expired_objects() {
    let temp_dir = TempDir::new().unwrap();
    let cas = Cas::new(temp_dir.path().to_path_buf()).unwrap();

    // Store objects
    let hash1 = cas.store(b"object 1").unwrap();
    let hash2 = cas.store(b"object 2").unwrap();

    // Simulate old access times by directly modifying metadata
    set_object_access_time(&cas, &hash1, SystemTime::now() - Duration::from_secs(86400 * 31)).unwrap(); // 31 days old

    // Find objects older than 30 days
    let ttl = Duration::from_secs(86400 * 30);
    let expired = find_expired_objects(&cas, ttl).unwrap();

    assert_eq!(expired.len(), 1);
    assert!(expired.contains(&hash1));
    assert!(!expired.contains(&hash2));
}

#[test]
fn test_evict_expired_objects() {
    let temp_dir = TempDir::new().unwrap();
    let cas = Cas::new(temp_dir.path().to_path_buf()).unwrap();

    // Store objects
    let hash1 = cas.store(b"old object").unwrap();
    let hash2 = cas.store(b"new object").unwrap();

    // Make hash1 old
    set_object_access_time(&cas, &hash1, SystemTime::now() - Duration::from_secs(86400 * 31)).unwrap();

    // Evict objects older than 30 days
    let ttl = Duration::from_secs(86400 * 30);
    let evicted_count = evict_expired_objects(&cas, ttl).unwrap();

    assert_eq!(evicted_count, 1);

    // Verify hash1 is gone, hash2 remains
    assert!(!cas.exists(&hash1));
    assert!(cas.exists(&hash2));
}

#[test]
fn test_evict_by_size_limit() {
    let temp_dir = TempDir::new().unwrap();
    let cas = Cas::new(temp_dir.path().to_path_buf()).unwrap();

    // Store multiple objects with known sizes
    let hash1 = cas.store(&vec![b'a'; 1000]).unwrap(); // 1KB
    std::thread::sleep(Duration::from_millis(10));
    let hash2 = cas.store(&vec![b'b'; 1000]).unwrap(); // 1KB
    std::thread::sleep(Duration::from_millis(10));
    let hash3 = cas.store(&vec![b'c'; 1000]).unwrap(); // 1KB

    // Set max size to 2KB (should evict oldest)
    let evicted = evict_by_size_limit(&cas, 2000).unwrap();

    assert!(evicted > 0);
    assert!(!cas.exists(&hash1)); // Oldest evicted
    assert!(cas.exists(&hash2));
    assert!(cas.exists(&hash3));
}

// Implementation

#[derive(Debug)]
struct ObjectMetadata {
    created: Option<SystemTime>,
    last_accessed: Option<SystemTime>,
    size: u64,
}

fn get_object_metadata(cas: &Cas, hash: &str) -> std::io::Result<ObjectMetadata> {
    let object_path = get_object_path(cas, hash);

    let metadata = fs::metadata(&object_path)?;
    let size = metadata.len();

    // For now, use file modification time as access time
    let modified = metadata.modified().ok();
    let created = metadata.created().ok();

    Ok(ObjectMetadata {
        created,
        last_accessed: modified,
        size,
    })
}

fn set_object_access_time(cas: &Cas, hash: &str, time: SystemTime) -> std::io::Result<()> {
    let object_path = get_object_path(cas, hash);

    // Use filetime crate to set access/modification times
    let filetime = filetime::FileTime::from_system_time(time);
    filetime::set_file_times(&object_path, filetime, filetime)?;

    Ok(())
}

fn find_expired_objects(cas: &Cas, ttl: Duration) -> std::io::Result<Vec<String>> {
    let mut expired = Vec::new();
    let now = SystemTime::now();

    // Walk all objects
    for hash in list_all_objects(cas)? {
        if let Ok(metadata) = get_object_metadata(cas, &hash) {
            if let Some(last_accessed) = metadata.last_accessed {
                if let Ok(age) = now.duration_since(last_accessed) {
                    if age > ttl {
                        expired.push(hash);
                    }
                }
            }
        }
    }

    Ok(expired)
}

fn evict_expired_objects(cas: &Cas, ttl: Duration) -> std::io::Result<usize> {
    let expired = find_expired_objects(cas, ttl)?;
    let count = expired.len();

    for hash in expired {
        let object_path = get_object_path(cas, &hash);
        fs::remove_file(object_path)?;
    }

    Ok(count)
}

fn evict_by_size_limit(cas: &Cas, max_bytes: u64) -> std::io::Result<usize> {
    // Get all objects with their metadata
    let mut objects: Vec<(String, ObjectMetadata)> = Vec::new();

    for hash in list_all_objects(cas)? {
        if let Ok(metadata) = get_object_metadata(cas, &hash) {
            objects.push((hash, metadata));
        }
    }

    // Sort by last accessed time (oldest first)
    objects.sort_by_key(|(_, meta)| meta.last_accessed);

    // Calculate current size
    let total_size: u64 = objects.iter().map(|(_, meta)| meta.size).sum();

    if total_size <= max_bytes {
        return Ok(0);
    }

    // Evict oldest until under limit
    let mut evicted = 0;
    let mut current_size = total_size;

    for (hash, metadata) in objects {
        if current_size <= max_bytes {
            break;
        }

        let object_path = get_object_path(cas, &hash);
        fs::remove_file(object_path)?;
        current_size -= metadata.size;
        evicted += 1;
    }

    Ok(evicted)
}

// Helper functions

fn get_object_path(cas: &Cas, hash: &str) -> PathBuf {
    let (prefix, suffix) = hash.split_at(2.min(hash.len()));
    cas.root().join("objects").join(prefix).join(suffix)
}

fn list_all_objects(cas: &Cas) -> std::io::Result<Vec<String>> {
    let mut objects = Vec::new();
    let objects_dir = cas.root().join("objects");

    if !objects_dir.exists() {
        return Ok(objects);
    }

    for prefix_entry in fs::read_dir(objects_dir)? {
        let prefix_entry = prefix_entry?;
        if prefix_entry.path().is_dir() {
            let prefix = prefix_entry.file_name().to_string_lossy().to_string();

            for obj_entry in fs::read_dir(prefix_entry.path())? {
                let obj_entry = obj_entry?;
                if obj_entry.path().is_file() {
                    let suffix = obj_entry.file_name().to_string_lossy().to_string();
                    let hash = format!("{}{}", prefix, suffix);
                    objects.push(hash);
                }
            }
        }
    }

    Ok(objects)
}

