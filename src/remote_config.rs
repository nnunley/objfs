// Configuration for remote execution
// Determines when to use remote workers vs local compilation

use std::env;

/// Configuration for remote execution
#[derive(Debug, Clone)]
pub struct RemoteConfig {
    /// URL of the remote execution server (e.g., "https://scheduler-host:50051")
    pub endpoint: Option<String>,

    /// Instance name for the remote server (e.g., "main")
    pub instance_name: String,

    /// Target platforms that remote workers CAN BUILD
    /// This is what the workers can compile TO (via cross-compilation if needed)
    /// NOT what platform the workers ARE running on
    /// Example: x86_64-linux workers with osxcross can build aarch64-apple-darwin
    pub remote_target_platforms: Vec<String>,

    /// Minimum compilation size to use remote execution (bytes)
    /// Small compilations are faster locally
    pub min_remote_size: u64,
}

impl RemoteConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        // Default to localhost if no endpoint specified
        // This enables local remote execution with caching benefits
        let endpoint = env::var("OBJFS_REMOTE_ENDPOINT")
            .ok()
            .or_else(|| {
                // Check if localhost worker is available
                if Self::localhost_worker_available() {
                    Some("http://localhost:50051".to_string())
                } else {
                    None
                }
            });

        let instance_name = env::var("OBJFS_REMOTE_INSTANCE")
            .unwrap_or_else(|_| "main".to_string());

        // Parse remote worker target capabilities
        // If not specified, default to host's target triple
        let remote_target_platforms = env::var("OBJFS_REMOTE_TARGETS")
            .ok()
            .map(|s| s.split(',').map(|t| t.trim().to_string()).collect())
            .unwrap_or_else(|| {
                // Default to current host's target
                vec![Self::host_target_triple()]
            });

        let min_remote_size = env::var("OBJFS_MIN_REMOTE_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(100 * 1024); // Default: 100KB

        Self {
            endpoint,
            instance_name,
            remote_target_platforms,
            min_remote_size,
        }
    }

    /// Get the current host's target triple
    fn host_target_triple() -> String {
        // Use Rust's built-in target detection
        #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
        {
            "aarch64-apple-darwin".to_string()
        }
        #[cfg(all(target_arch = "x86_64", target_os = "macos"))]
        {
            "x86_64-apple-darwin".to_string()
        }
        #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
        {
            "x86_64-unknown-linux-gnu".to_string()
        }
        #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
        {
            "aarch64-unknown-linux-gnu".to_string()
        }
        #[cfg(not(any(
            all(target_arch = "aarch64", target_os = "macos"),
            all(target_arch = "x86_64", target_os = "macos"),
            all(target_arch = "x86_64", target_os = "linux"),
            all(target_arch = "aarch64", target_os = "linux")
        )))]
        {
            // Fallback for other platforms
            env::var("TARGET").unwrap_or_else(|_| "unknown".to_string())
        }
    }

    /// Check if a localhost worker is available
    fn localhost_worker_available() -> bool {
        // Quick check if localhost:50051 is listening
        // Use a very short timeout to avoid blocking
        std::net::TcpStream::connect_timeout(
            &"127.0.0.1:50051".parse().unwrap(),
            std::time::Duration::from_millis(100)
        ).is_ok()
    }

    /// Check if remote execution is enabled
    pub fn is_enabled(&self) -> bool {
        self.endpoint.is_some() && !self.remote_target_platforms.is_empty()
    }

    /// Check if remote workers can build for the given target triple
    pub fn can_build_target(&self, target_triple: &str) -> bool {
        self.remote_target_platforms.iter()
            .any(|t| t == target_triple)
    }

    /// Determine if this build should use remote execution
    pub fn should_use_remote(&self, target_triple: &str, input_size: u64) -> bool {
        if !self.is_enabled() {
            return false;
        }

        // Use remote if:
        // 1. Remote workers can build this target
        // 2. Input size is above threshold
        self.can_build_target(target_triple) && input_size >= self.min_remote_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_remote_config_disabled_by_default() {
        // Clear environment
        unsafe {
            env::remove_var("OBJFS_REMOTE_ENDPOINT");
            env::remove_var("OBJFS_REMOTE_TARGETS");
        }

        let config = RemoteConfig::from_env();
        assert!(!config.is_enabled());
    }

    #[test]
    fn test_remote_config_from_env() {
        unsafe {
            env::set_var("OBJFS_REMOTE_ENDPOINT", "http://scheduler-host:50051");
            env::set_var("OBJFS_REMOTE_TARGETS", "aarch64-apple-darwin,x86_64-unknown-linux-gnu");
            env::set_var("OBJFS_REMOTE_INSTANCE", "test-instance");
        }

        let config = RemoteConfig::from_env();

        assert!(config.is_enabled());
        assert_eq!(config.endpoint.unwrap(), "http://scheduler-host:50051");
        assert_eq!(config.instance_name, "test-instance");
        assert_eq!(config.remote_target_platforms.len(), 2);

        // Cleanup
        unsafe {
            env::remove_var("OBJFS_REMOTE_ENDPOINT");
            env::remove_var("OBJFS_REMOTE_TARGETS");
            env::remove_var("OBJFS_REMOTE_INSTANCE");
        }
    }

    #[test]
    fn test_can_build_macos_with_osxcross() {
        unsafe {
            env::set_var("OBJFS_REMOTE_ENDPOINT", "http://scheduler-host:50051");
            // Linux workers with osxcross can build macOS targets
            env::set_var("OBJFS_REMOTE_TARGETS", "aarch64-apple-darwin,x86_64-apple-darwin");
        }

        let config = RemoteConfig::from_env();

        // Should be able to build macOS targets
        assert!(config.can_build_target("aarch64-apple-darwin"));
        assert!(config.can_build_target("x86_64-apple-darwin"));

        // Should use remote for macOS builds above threshold
        assert!(config.should_use_remote("aarch64-apple-darwin", 200 * 1024));

        // Should NOT use remote for small builds
        assert!(!config.should_use_remote("aarch64-apple-darwin", 50 * 1024));

        // Cleanup
        unsafe {
            env::remove_var("OBJFS_REMOTE_ENDPOINT");
            env::remove_var("OBJFS_REMOTE_TARGETS");
        }
    }

    #[test]
    fn test_cannot_build_unsupported_target() {
        unsafe {
            env::set_var("OBJFS_REMOTE_ENDPOINT", "http://scheduler-host:50051");
            env::set_var("OBJFS_REMOTE_TARGETS", "aarch64-apple-darwin");
        }

        let config = RemoteConfig::from_env();

        // macOS supported
        assert!(config.can_build_target("aarch64-apple-darwin"));

        // Linux NOT supported (workers only configured for macOS)
        assert!(!config.can_build_target("x86_64-unknown-linux-gnu"));
        assert!(!config.should_use_remote("x86_64-unknown-linux-gnu", 200 * 1024));

        // Cleanup
        unsafe {
            env::remove_var("OBJFS_REMOTE_ENDPOINT");
            env::remove_var("OBJFS_REMOTE_TARGETS");
        }
    }
}
