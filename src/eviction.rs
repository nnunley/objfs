use std::fs;
use std::time::{SystemTime, Duration};
use std::io;
use crate::cas::Cas;

#[derive(Debug)]
pub struct ObjectMetadata {
    pub created: Option<SystemTime>,
    pub last_accessed: Option<SystemTime>,
    pub size: u64,
}

pub struct EvictionStats {
    pub objects_evicted: usize,
    pub bytes_freed: u64,
}

impl Cas {
    /// Get metadata for an object
    pub fn get_object_metadata(&self, hash: &str) -> io::Result<ObjectMetadata> {
        let hash_string = hash.to_string();
        let object_path = self.object_path(&hash_string);

        let metadata = fs::metadata(&object_path)?;
        let size = metadata.len();

        let modified = metadata.modified().ok();
        let created = metadata.created().ok();

        Ok(ObjectMetadata {
            created,
            last_accessed: modified,
            size,
        })
    }

    /// List all object hashes in CAS
    pub fn list_all_objects(&self) -> io::Result<Vec<String>> {
        let mut objects = Vec::new();
        let objects_dir = self.root().join("objects");

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

    /// Find objects older than TTL
    pub fn find_expired_objects(&self, ttl: Duration) -> io::Result<Vec<String>> {
        let mut expired = Vec::new();
        let now = SystemTime::now();

        for hash in self.list_all_objects()? {
            if let Ok(metadata) = self.get_object_metadata(&hash) {
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

    /// Evict objects older than TTL
    pub fn evict_expired_objects(&self, ttl: Duration) -> io::Result<EvictionStats> {
        let expired = self.find_expired_objects(ttl)?;
        let mut bytes_freed = 0u64;

        for hash in &expired {
            if let Ok(metadata) = self.get_object_metadata(hash) {
                bytes_freed += metadata.size;
            }

            let hash_string = hash.to_string();
            let object_path = self.object_path(&hash_string);
            fs::remove_file(object_path)?;
        }

        Ok(EvictionStats {
            objects_evicted: expired.len(),
            bytes_freed,
        })
    }

    /// Evict objects to get under size limit (LRU)
    pub fn evict_by_size_limit(&self, max_bytes: u64) -> io::Result<EvictionStats> {
        // Get all objects with metadata
        let mut objects: Vec<(String, ObjectMetadata)> = Vec::new();

        for hash in self.list_all_objects()? {
            if let Ok(metadata) = self.get_object_metadata(&hash) {
                objects.push((hash, metadata));
            }
        }

        // Sort by last accessed (oldest first)
        objects.sort_by_key(|(_, meta)| meta.last_accessed);

        // Calculate current size
        let total_size: u64 = objects.iter().map(|(_, meta)| meta.size).sum();

        if total_size <= max_bytes {
            return Ok(EvictionStats {
                objects_evicted: 0,
                bytes_freed: 0,
            });
        }

        // Evict oldest until under limit
        let mut evicted = 0;
        let mut bytes_freed = 0u64;
        let mut current_size = total_size;

        for (hash, metadata) in objects {
            if current_size <= max_bytes {
                break;
            }

            let hash_string = hash.to_string();
            let object_path = self.object_path(&hash_string);
            fs::remove_file(object_path)?;
            current_size -= metadata.size;
            bytes_freed += metadata.size;
            evicted += 1;
        }

        Ok(EvictionStats {
            objects_evicted: evicted,
            bytes_freed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_list_all_objects() {
        let temp_dir = TempDir::new().unwrap();
        let cas = Cas::new(temp_dir.path().to_path_buf()).unwrap();

        let hash1 = cas.store(b"test 1").unwrap();
        let hash2 = cas.store(b"test 2").unwrap();

        let objects = cas.list_all_objects().unwrap();

        assert_eq!(objects.len(), 2);
        assert!(objects.contains(&hash1));
        assert!(objects.contains(&hash2));
    }

    #[test]
    fn test_evict_by_ttl() {
        let temp_dir = TempDir::new().unwrap();
        let cas = Cas::new(temp_dir.path().to_path_buf()).unwrap();

        cas.store(b"test").unwrap();

        // TTL of 0 means everything is expired
        let stats = cas.evict_expired_objects(Duration::from_secs(0)).unwrap();

        assert_eq!(stats.objects_evicted, 1);
        assert!(stats.bytes_freed > 0);
    }
}
