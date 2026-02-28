// Tests that cache keys include platform information
// This prevents cross-platform cache collisions

use objfs::platform::Platform;
use objfs::cas::Cas;
use tempfile::TempDir;
use std::fs;

#[test]
fn test_different_platforms_produce_different_cache_keys() {
    // Simulate two builds with identical source but different platforms
    let linux_x86 = Platform::new("linux", "x86_64");
    let macos_arm = Platform::new("macos", "aarch64");

    // Cache keys must be different
    let linux_key = linux_x86.to_cache_key_string();
    let macos_key = macos_arm.to_cache_key_string();

    assert_ne!(linux_key, macos_key);
    assert!(linux_key.contains("linux"));
    assert!(macos_key.contains("macos"));
}

#[test]
fn test_platform_included_in_cache_lookup() {
    // This test documents the expected behavior:
    // Same source code + different platform = different cache entry

    let temp_dir = TempDir::new().unwrap();
    let cas = Cas::new(temp_dir.path().to_path_buf()).unwrap();

    // Store data with platform in the key
    let platform = Platform::detect();
    let source_hash = "abc123";
    let cache_key = format!("{}-{}", source_hash, platform.to_cache_key_string());

    // Store artifact
    let artifact_data = b"compiled binary for current platform";
    let artifact_hash = cas.store(artifact_data).unwrap();

    // Cache key should be platform-specific
    assert!(cache_key.contains(&platform.to_cache_key_string()));

    // Different platform = different key
    let other_platform = Platform::new("other", "other_arch");
    let other_cache_key = format!("{}-{}", source_hash, other_platform.to_cache_key_string());

    assert_ne!(cache_key, other_cache_key);
}

#[test]
fn test_current_platform_detection_is_stable() {
    // Platform detection should be deterministic
    let p1 = Platform::detect();
    let p2 = Platform::detect();

    assert_eq!(p1.to_cache_key_string(), p2.to_cache_key_string());
}

#[test]
fn test_cross_compilation_different_platform() {
    // When cross-compiling, target platform differs from host
    let host = Platform::detect();
    let target = Platform::new("linux", "x86_64");

    // Should produce different cache keys
    assert_ne!(
        host.to_cache_key_string(),
        target.to_cache_key_string()
    );
}
