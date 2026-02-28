// Platform detection for Remote Execution API v2
// Ensures builds are architecture and OS-specific

use std::collections::HashMap;

/// Platform properties for Remote Execution API v2
/// See: https://github.com/bazelbuild/remote-apis/blob/main/build/bazel/remote/execution/v2/platform.md
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub properties: HashMap<String, String>,
}

impl Platform {
    /// Detect the current build platform from rustc
    pub fn detect() -> Self {
        let mut properties = HashMap::new();

        // Standard RE API v2 platform properties
        properties.insert("OSFamily".to_string(), Self::detect_os_family());
        properties.insert("ISA".to_string(), Self::detect_arch());

        Self { properties }
    }

    /// Get OS family (Linux, macOS, Windows, etc.)
    fn detect_os_family() -> String {
        std::env::consts::OS.to_string()
    }

    /// Get architecture (x86_64, aarch64, etc.)
    fn detect_arch() -> String {
        std::env::consts::ARCH.to_string()
    }

    /// Create platform from explicit values
    pub fn new(os: &str, arch: &str) -> Self {
        let mut properties = HashMap::new();
        properties.insert("OSFamily".to_string(), os.to_string());
        properties.insert("ISA".to_string(), arch.to_string());
        Self { properties }
    }

    /// Parse a Rust target triple into a Platform
    /// Examples: "x86_64-unknown-linux-gnu", "aarch64-apple-darwin"
    pub fn from_target_triple(triple: &str) -> Option<Self> {
        let parts: Vec<&str> = triple.split('-').collect();
        if parts.is_empty() {
            return None;
        }

        // Parse architecture (first component)
        let arch = parts[0];

        // Parse OS (try to find in the triple)
        let os = if triple.contains("linux") {
            "linux"
        } else if triple.contains("darwin") || triple.contains("apple") {
            "macos"
        } else if triple.contains("windows") {
            "windows"
        } else if triple.contains("freebsd") {
            "freebsd"
        } else {
            // Unknown OS
            return None;
        };

        Some(Self::new(os, arch))
    }

    /// Get a stable string representation for cache keys
    pub fn to_cache_key_string(&self) -> String {
        // Sort keys for determinism
        let mut keys: Vec<_> = self.properties.keys().collect();
        keys.sort();

        keys.iter()
            .map(|k| format!("{}={}", k, self.properties.get(*k).unwrap()))
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detection() {
        let platform = Platform::detect();

        // Should have OSFamily and Arch
        assert!(platform.properties.contains_key("OSFamily"));
        assert!(platform.properties.contains_key("Arch"));

        // OSFamily should be one of the known values
        let os = platform.properties.get("OSFamily").unwrap();
        assert!(
            os == "linux" || os == "macos" || os == "windows" || os == "ios",
            "Unexpected OS: {}",
            os
        );

        // Arch should be populated
        let arch = platform.properties.get("Arch").unwrap();
        assert!(!arch.is_empty());
    }

    #[test]
    fn test_platform_cache_key_is_deterministic() {
        let p1 = Platform::new("linux", "x86_64");
        let p2 = Platform::new("linux", "x86_64");

        assert_eq!(p1.to_cache_key_string(), p2.to_cache_key_string());
    }

    #[test]
    fn test_different_platforms_have_different_keys() {
        let linux_x86 = Platform::new("linux", "x86_64");
        let linux_arm = Platform::new("linux", "aarch64");
        let macos_arm = Platform::new("macos", "aarch64");

        assert_ne!(
            linux_x86.to_cache_key_string(),
            linux_arm.to_cache_key_string()
        );
        assert_ne!(
            linux_arm.to_cache_key_string(),
            macos_arm.to_cache_key_string()
        );
    }

    #[test]
    fn test_cache_key_sorted_for_determinism() {
        let mut p1_props = HashMap::new();
        p1_props.insert("Arch".to_string(), "x86_64".to_string());
        p1_props.insert("OSFamily".to_string(), "linux".to_string());
        let p1 = Platform { properties: p1_props };

        let mut p2_props = HashMap::new();
        p2_props.insert("OSFamily".to_string(), "linux".to_string());
        p2_props.insert("Arch".to_string(), "x86_64".to_string());
        let p2 = Platform { properties: p2_props };

        // Even though properties were added in different order,
        // cache key should be identical
        assert_eq!(p1.to_cache_key_string(), p2.to_cache_key_string());
    }

    #[test]
    fn test_current_platform() {
        let platform = Platform::detect();
        let key = platform.to_cache_key_string();

        // On macOS aarch64 (Apple Silicon)
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        assert!(key.contains("Arch=aarch64"));
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        assert!(key.contains("OSFamily=macos"));

        // Should be non-empty
        assert!(!key.is_empty());
    }

    #[test]
    fn test_parse_target_triple_linux_x86() {
        let platform = Platform::from_target_triple("x86_64-unknown-linux-gnu").unwrap();
        assert_eq!(platform.properties.get("OSFamily").unwrap(), "linux");
        assert_eq!(platform.properties.get("Arch").unwrap(), "x86_64");
    }

    #[test]
    fn test_parse_target_triple_macos_arm() {
        let platform = Platform::from_target_triple("aarch64-apple-darwin").unwrap();
        assert_eq!(platform.properties.get("OSFamily").unwrap(), "macos");
        assert_eq!(platform.properties.get("Arch").unwrap(), "aarch64");
    }

    #[test]
    fn test_parse_target_triple_windows() {
        let platform = Platform::from_target_triple("x86_64-pc-windows-msvc").unwrap();
        assert_eq!(platform.properties.get("OSFamily").unwrap(), "windows");
        assert_eq!(platform.properties.get("Arch").unwrap(), "x86_64");
    }

    #[test]
    fn test_cross_compile_different_platforms() {
        // Host: macOS arm64
        let host = Platform::detect();

        // Target: Linux x86_64
        let target = Platform::from_target_triple("x86_64-unknown-linux-gnu").unwrap();

        // Different platforms should produce different cache keys
        assert_ne!(host.to_cache_key_string(), target.to_cache_key_string());
    }
}
