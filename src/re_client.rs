// Remote Execution API v2 client for objfs
// This module provides a client for interacting with RE API v2 compatible servers
// like NativeLink, BuildBarn, etc.

use std::io;
use std::path::PathBuf;
use crate::cas::Cas;
use crate::platform::Platform;
use sha2::{Sha256, Digest as Sha2Digest};

/// A content digest using SHA256
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Digest {
    pub hash: String,
    pub size_bytes: i64,
}

impl Digest {
    pub fn new(hash: String, size_bytes: i64) -> Self {
        Self { hash, size_bytes }
    }

    /// Compute digest from data
    pub fn from_data(data: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = hex::encode(hasher.finalize());
        Self {
            hash,
            size_bytes: data.len() as i64,
        }
    }
}

/// Command represents a single executable action
/// This maps to Command in RE API v2
#[derive(Debug, Clone)]
pub struct Command {
    pub arguments: Vec<String>,
    pub working_directory: String,
}

impl Command {
    /// Create a new command
    pub fn new(arguments: Vec<String>, working_directory: &str) -> Self {
        Self {
            arguments,
            working_directory: working_directory.to_string(),
        }
    }

    /// Create a command from rustc arguments
    pub fn from_rustc_args(args: &[&str], working_dir: &PathBuf) -> Self {
        // Use rustc from PATH (works on both NixOS and macOS)
        let mut full_args = vec!["rustc".to_string()];

        // Convert absolute paths to relative
        for arg in args {
            let path = PathBuf::from(arg);
            if path.is_absolute() {
                // Try to make relative to working_dir first
                if let Ok(rel) = path.strip_prefix(working_dir) {
                    full_args.push(rel.to_string_lossy().to_string());
                } else if let Ok(rel) = path.strip_prefix("/tmp") {
                    full_args.push(rel.to_string_lossy().to_string());
                } else {
                    // Last resort: use just the filename
                    if let Some(name) = path.file_name() {
                        full_args.push(name.to_string_lossy().to_string());
                    } else {
                        full_args.push(arg.to_string());
                    }
                }
            } else {
                full_args.push(arg.to_string());
            }
        }

        Self {
            arguments: full_args,
            working_directory: working_dir.to_string_lossy().to_string(),
        }
    }

    /// Serialize command to bytes for hashing
    pub fn to_bytes(&self) -> Vec<u8> {
        // Simple serialization: working_dir + null + args joined by null
        let mut bytes = Vec::new();
        bytes.extend_from_slice(self.working_directory.as_bytes());
        bytes.push(0);
        for arg in &self.arguments {
            bytes.extend_from_slice(arg.as_bytes());
            bytes.push(0);
        }
        bytes
    }

    pub fn output_files(&self) -> Vec<String> {
        let mut outputs = Vec::new();
        let mut i = 0;
        while i < self.arguments.len() {
            if self.arguments[i] == "-o" && i + 1 < self.arguments.len() {
                outputs.push(self.arguments[i + 1].clone());
                i += 2;
            } else {
                i += 1;
            }
        }
        outputs
    }
}

/// Action represents a build action to execute remotely
/// This maps to Action in RE API v2
#[derive(Debug, Clone)]
pub struct Action {
    pub command: Command,
    pub command_digest: Digest,
    pub input_files: Vec<PathBuf>,
    pub platform: Option<Platform>,
}

impl Action {
    /// Create a new action
    pub fn new(command: Command, input_files: Vec<PathBuf>) -> Self {
        // Compute command digest
        let command_bytes = command.to_bytes();
        let command_digest = Digest::from_data(&command_bytes);

        // Use Linux platform for remote execution
        // The worker runs on Linux, even if we're compiling for macOS
        let mut platform = Platform::new("linux", "x86-64");
        // Add container-image property for Rust workers
        platform.properties.insert("container-image".to_string(), "rust:latest".to_string());

        Self {
            command,
            command_digest,
            input_files,
            platform: Some(platform),
        }
    }
}

/// Output file from remote execution
#[derive(Debug, Clone, PartialEq)]
pub struct OutputFile {
    pub path: String,
    pub hash: String,
    pub size_bytes: i64,
    pub is_executable: bool,
}

/// Result from executing an action remotely
#[derive(Debug, Clone)]
pub struct ActionResult {
    pub output_files: Vec<OutputFile>,
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Remote executor for running actions on RE API v2 servers
pub struct RemoteExecutor {
    endpoint: String,
    instance_name: String,
    use_tls: bool,
}

impl RemoteExecutor {
    /// Create a new remote executor
    pub fn new(endpoint: String, instance_name: String, use_tls: bool) -> Self {
        Self {
            endpoint,
            instance_name,
            use_tls,
        }
    }

    /// Execute an action remotely and return the ActionResult
    pub fn execute(&self, action: &Action) -> io::Result<ActionResult> {
        use crate::grpc_client::GrpcRemoteCas;

        // Normalize endpoint URL for gRPC client
        let endpoint = if self.use_tls {
            if !self.endpoint.starts_with("https://") {
                format!("https://{}", self.endpoint)
            } else {
                self.endpoint.clone()
            }
        } else {
            if !self.endpoint.starts_with("http://") {
                format!("http://{}", self.endpoint)
            } else {
                self.endpoint.clone()
            }
        };

        // Create gRPC client
        let grpc_client = GrpcRemoteCas::new(
            endpoint,
            self.instance_name.clone(),
        );

        // Execute via gRPC
        grpc_client.execute_action(action, &action.command)
    }
}

/// Remote CAS client trait
/// This abstracts the Content Addressable Storage service from RE API v2
pub trait RemoteCas {
    /// Check if a blob exists in remote CAS
    fn exists(&self, digest: &Digest) -> io::Result<bool>;

    /// Upload a blob to remote CAS
    fn upload(&self, data: &[u8]) -> io::Result<Digest>;

    /// Download a blob from remote CAS
    fn download(&self, digest: &Digest) -> io::Result<Vec<u8>>;
}

/// HTTP-based Remote CAS client for RE API v2
/// Implements the REST API: GET/PUT/HEAD /{instance}/cas/<hash>
pub struct HttpRemoteCas {
    base_url: String,
    instance_name: String,
    client: reqwest::blocking::Client,
}

impl HttpRemoteCas {
    /// Create a new HTTP CAS client
    /// base_url should be like "http://build-server:50051"
    /// instance_name is typically "main" or empty string
    pub fn new(base_url: String) -> Self {
        Self::with_instance(base_url, String::new())
    }

    /// Create HTTP CAS client with custom instance name
    pub fn with_instance(base_url: String, instance_name: String) -> Self {
        Self {
            base_url,
            instance_name,
            client: reqwest::blocking::Client::new(),
        }
    }

    fn cas_url(&self, hash: &str) -> String {
        if self.instance_name.is_empty() {
            format!("{}/cas/{}", self.base_url, hash)
        } else {
            format!("{}/{}/cas/{}", self.base_url, self.instance_name, hash)
        }
    }
}

impl RemoteCas for HttpRemoteCas {
    fn exists(&self, digest: &Digest) -> io::Result<bool> {
        let url = self.cas_url(&digest.hash);

        let response = self.client
            .head(&url)
            .send()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        Ok(response.status().is_success())
    }

    fn upload(&self, data: &[u8]) -> io::Result<Digest> {
        use sha2::{Sha256, Digest as Sha2Digest};

        // Compute digest
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = hex::encode(hasher.finalize());
        let digest = Digest::new(hash.clone(), data.len() as i64);

        // Upload via PUT
        let url = self.cas_url(&hash);
        let response = self.client
            .put(&url)
            .body(data.to_vec())
            .send()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        if !response.status().is_success() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Upload failed: {}", response.status())
            ));
        }

        Ok(digest)
    }

    fn download(&self, digest: &Digest) -> io::Result<Vec<u8>> {
        let url = self.cas_url(&digest.hash);

        let response = self.client
            .get(&url)
            .send()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        if !response.status().is_success() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Download failed: {}", response.status())
            ));
        }

        let data = response
            .bytes()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
            .to_vec();

        Ok(data)
    }
}

/// CAS backend types - can be local or remote
pub enum CasBackend {
    Local(Cas),
    Remote(Box<dyn RemoteCas>),
}

/// Hierarchical CAS with multiple tiers (local, team cache, company cache, etc.)
/// Checks backends in order, implements write-back caching
pub struct HybridCas {
    backends: Vec<CasBackend>,
}

impl HybridCas {
    /// Create a new HybridCas with ordered backends
    /// First backend is checked first (typically local, fastest)
    pub fn new(backends: Vec<CasBackend>) -> Self {
        Self { backends }
    }

    /// Check if a blob exists in any backend
    pub fn exists(&self, hash: &str) -> io::Result<bool> {
        let hash_string = hash.to_string();
        for backend in &self.backends {
            let exists = match backend {
                CasBackend::Local(cas) => cas.exists(&hash_string),
                CasBackend::Remote(remote) => {
                    let digest = Digest::new(hash_string.clone(), 0);
                    remote.exists(&digest)?
                }
            };

            if exists {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Get a blob from the hierarchy
    /// Checks each backend in order, populates earlier tiers on hit (write-back)
    pub fn get(&self, hash: &str) -> io::Result<Vec<u8>> {
        let hash_string = hash.to_string();
        for (idx, backend) in self.backends.iter().enumerate() {
            let result = match backend {
                CasBackend::Local(cas) => cas.get(&hash_string),
                CasBackend::Remote(remote) => {
                    let digest = Digest::new(hash_string.clone(), 0);
                    remote.download(&digest)
                }
            };

            if let Ok(data) = result {
                // Write-back: populate earlier tiers
                for earlier_idx in 0..idx {
                    if let CasBackend::Local(earlier_cas) = &self.backends[earlier_idx] {
                        let _ = earlier_cas.store(&data);
                    }
                }
                return Ok(data);
            }
        }

        Err(io::Error::new(io::ErrorKind::NotFound, "Blob not found in any backend"))
    }

    /// Store a blob in the first backend (typically local)
    pub fn store(&self, data: &[u8]) -> io::Result<String> {
        if self.backends.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "No backends configured"));
        }

        match &self.backends[0] {
            CasBackend::Local(cas) => cas.store(data),
            CasBackend::Remote(remote) => {
                let digest = remote.upload(data)?;
                Ok(digest.hash)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    /// In-memory implementation of RemoteCas for testing
    struct InMemoryCas {
        storage: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    }

    impl InMemoryCas {
        fn new() -> Self {
            Self {
                storage: Arc::new(Mutex::new(HashMap::new())),
            }
        }
    }

    impl RemoteCas for InMemoryCas {
        fn exists(&self, digest: &Digest) -> io::Result<bool> {
            let storage = self.storage.lock().unwrap();
            Ok(storage.contains_key(&digest.hash))
        }

        fn upload(&self, data: &[u8]) -> io::Result<Digest> {
            use sha2::{Sha256, Digest as Sha2Digest};

            // Compute SHA256 hash
            let mut hasher = Sha256::new();
            hasher.update(data);
            let hash = hex::encode(hasher.finalize());

            let digest = Digest::new(hash.clone(), data.len() as i64);

            // Store in memory
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
    fn test_upload_and_download() {
        let cas = InMemoryCas::new();
        let data = b"hello world";

        // Upload data
        let digest = cas.upload(data).unwrap();

        // Verify digest has correct hash and size
        assert_eq!(digest.size_bytes, data.len() as i64);
        assert!(!digest.hash.is_empty());

        // Download and verify
        let downloaded = cas.download(&digest).unwrap();
        assert_eq!(downloaded, data);
    }

    #[test]
    fn test_exists_before_and_after_upload() {
        let cas = InMemoryCas::new();
        let data = b"test data";

        // Upload and get digest
        let digest = cas.upload(data).unwrap();

        // Should exist after upload
        assert!(cas.exists(&digest).unwrap());

        // Non-existent digest should not exist
        let fake_digest = Digest::new("nonexistent".to_string(), 0);
        assert!(!cas.exists(&fake_digest).unwrap());
    }

    #[test]
    fn test_download_nonexistent_blob_fails() {
        let cas = InMemoryCas::new();
        let fake_digest = Digest::new("nonexistent".to_string(), 0);

        let result = cas.download(&fake_digest);
        assert!(result.is_err());
    }

    #[test]
    fn test_hybrid_cas_checks_backends_in_order() {
        use tempfile::TempDir;

        // Setup 3-tier hierarchy
        let temp_dir = TempDir::new().unwrap();
        let local = Cas::new(temp_dir.path().to_path_buf()).unwrap();
        let remote1 = InMemoryCas::new();
        let remote2 = InMemoryCas::new();

        // Store in remote1 only
        let data = b"test data";
        let digest = remote1.upload(data).unwrap();

        // Create hierarchy: local -> remote1 -> remote2
        let hybrid = HybridCas::new(vec![
            CasBackend::Local(local),
            CasBackend::Remote(Box::new(remote1)),
            CasBackend::Remote(Box::new(remote2)),
        ]);

        // Should find in remote1 (second backend)
        assert!(hybrid.exists(&digest.hash).unwrap());

        // Should be able to retrieve
        let retrieved = hybrid.get(&digest.hash).unwrap();
        assert_eq!(retrieved, data);
    }

    #[test]
    fn test_hybrid_cas_write_back_to_earlier_tiers() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let local = Cas::new(temp_dir.path().to_path_buf()).unwrap();
        let remote = InMemoryCas::new();

        // Store in remote only
        let data = b"remote data";
        let digest = remote.upload(data).unwrap();

        // Verify local doesn't have it
        assert!(!local.exists(&digest.hash));

        // Create hierarchy
        let hybrid = HybridCas::new(vec![
            CasBackend::Local(local.clone()),
            CasBackend::Remote(Box::new(remote)),
        ]);

        // Get from hybrid (will hit remote)
        let retrieved = hybrid.get(&digest.hash).unwrap();
        assert_eq!(retrieved, data);

        // Now local should have it (write-back)
        assert!(local.exists(&digest.hash));
    }

    #[test]
    fn test_hybrid_cas_stores_to_first_tier() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let local = Cas::new(temp_dir.path().to_path_buf()).unwrap();

        let hybrid = HybridCas::new(vec![
            CasBackend::Local(local.clone()),
        ]);

        let data = b"new data";
        let hash = hybrid.store(data).unwrap();

        // Should exist in local
        assert!(local.exists(&hash));
    }
}
