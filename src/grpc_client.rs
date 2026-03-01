// gRPC client for Remote Execution API v2
// Implements secure communication with NativeLink over TLS

use crate::re_client::{Digest, RemoteCas};
use std::io;

// Use NativeLink's proto definitions
use nativelink_proto::build::bazel::remote::execution::v2 as cas;

/// gRPC-based Remote CAS client
/// Supports both TLS (https://) and plaintext (grpc://) connections
pub struct GrpcRemoteCas {
    endpoint: String,
    instance_name: String,
    use_tls: bool,
}

impl GrpcRemoteCas {
    /// Create a new gRPC client
    /// endpoint should be like "https://build-server:50051" or "grpc://localhost:50051"
    pub fn new(endpoint: String, instance_name: String) -> Self {
        let use_tls = endpoint.starts_with("https://");
        Self {
            endpoint,
            instance_name,
            use_tls,
        }
    }

    /// Execute async code, handling tokio runtime properly
    /// If we're already in a runtime, use it. Otherwise create a new one.
    fn run_async<F, T>(&self, f: F) -> io::Result<T>
    where
        F: std::future::Future<Output = io::Result<T>> + Send,
        T: Send,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                // We're in a runtime, spawn blocking task
                std::thread::scope(|s| {
                    let result = s.spawn(|| {
                        handle.block_on(f)
                    }).join();
                    result.unwrap()
                })
            }
            Err(_) => {
                // No runtime, create one
                let runtime = tokio::runtime::Runtime::new()
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                runtime.block_on(f)
            }
        }
    }

    /// Create a client with TLS enabled
    pub fn with_tls(host: &str, port: u16, instance_name: String) -> Self {
        Self::new(
            format!("https://{}:{}", host, port),
            instance_name,
        )
    }

    /// Create a client without TLS (insecure, for testing only)
    pub fn without_tls(host: &str, port: u16, instance_name: String) -> Self {
        Self::new(
            format!("http://{}:{}", host, port),
            instance_name,
        )
    }

    async fn connect(&self) -> Result<cas::content_addressable_storage_client::ContentAddressableStorageClient<tonic::transport::Channel>, anyhow::Error> {
        let endpoint = tonic::transport::Channel::from_shared(self.endpoint.clone())?;

        let channel = if self.use_tls {
            endpoint
                .tls_config(tonic::transport::ClientTlsConfig::new())?
                .connect()
                .await?
        } else {
            endpoint
                .connect()
                .await?
        };

        Ok(cas::content_addressable_storage_client::ContentAddressableStorageClient::new(channel))
    }
}

impl RemoteCas for GrpcRemoteCas {
    fn exists(&self, digest: &Digest) -> io::Result<bool> {
        self.run_async(async {
            let mut client = self.connect()
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            let request = cas::FindMissingBlobsRequest {
                instance_name: self.instance_name.clone(),
                blob_digests: vec![cas::Digest {
                    hash: digest.hash.clone(),
                    size_bytes: digest.size_bytes,
                }],
                digest_function: 0,  // 0 = UNKNOWN (use default SHA256)
            };

            let response = client
                .find_missing_blobs(request)
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            // If the digest is in missing_blob_digests, it doesn't exist
            Ok(response.get_ref().missing_blob_digests.is_empty())
        })
    }

    fn upload(&self, data: &[u8]) -> io::Result<Digest> {
        use sha2::{Sha256, Digest as Sha2Digest};

        // Compute digest
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = hex::encode(hasher.finalize());
        let digest = Digest::new(hash.clone(), data.len() as i64);

        self.run_async(async {
            let mut client = self.connect()
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            let request = cas::BatchUpdateBlobsRequest {
                instance_name: self.instance_name.clone(),
                requests: vec![cas::batch_update_blobs_request::Request {
                    digest: Some(cas::Digest {
                        hash: hash.clone(),
                        size_bytes: data.len() as i64,
                    }),
                    data: data.to_vec().into(),
                    compressor: 0,  // 0 = IDENTITY (no compression)
                }],
                digest_function: 0,  // 0 = UNKNOWN (use default SHA256)
            };

            let response = client
                .batch_update_blobs(request)
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            // Check if upload succeeded
            let responses = &response.get_ref().responses;
            if responses.is_empty() {
                return Err(io::Error::new(io::ErrorKind::Other, "No response from server"));
            }

            if let Some(status) = &responses[0].status {
                if status.code != 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("Upload failed: {}", status.message)
                    ));
                }
            }

            Ok(digest)
        })
    }

    fn download(&self, digest: &Digest) -> io::Result<Vec<u8>> {
        self.run_async(async {
            let mut client = self.connect()
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            let request = cas::BatchReadBlobsRequest {
                instance_name: self.instance_name.clone(),
                digests: vec![cas::Digest {
                    hash: digest.hash.clone(),
                    size_bytes: digest.size_bytes,
                }],
                acceptable_compressors: vec![0],  // IDENTITY (no compression)
                digest_function: 0,  // 0 = UNKNOWN (use default SHA256)
            };

            let response = client
                .batch_read_blobs(request)
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            let responses = &response.get_ref().responses;
            if responses.is_empty() {
                return Err(io::Error::new(io::ErrorKind::NotFound, "Blob not found"));
            }

            if let Some(status) = &responses[0].status {
                if status.code != 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("Download failed: {}", status.message)
                    ));
                }
            }

            Ok(responses[0].data.to_vec())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn test_grpc_cas_with_tls() {
        let cas = GrpcRemoteCas::with_tls("localhost", 50051, "main".to_string());

        let data = b"test data from gRPC client";
        let digest = cas.upload(data).unwrap();

        assert!(cas.exists(&digest).unwrap());

        let retrieved = cas.download(&digest).unwrap();
        assert_eq!(retrieved, data);
    }

    #[test]
    #[ignore]
    fn test_grpc_cas_without_tls() {
        // For local testing without TLS
        let cas = GrpcRemoteCas::without_tls("localhost", 50051, "main".to_string());

        let data = b"test data without TLS";
        let digest = cas.upload(data).unwrap();

        assert!(cas.exists(&digest).unwrap());

        let retrieved = cas.download(&digest).unwrap();
        assert_eq!(retrieved, data);
    }
}

// Additional methods for GrpcRemoteCas
impl GrpcRemoteCas {
    /// Get execution client connection
    async fn connect_execution(&self) -> Result<cas::execution_client::ExecutionClient<tonic::transport::Channel>, anyhow::Error> {
        let endpoint = tonic::transport::Channel::from_shared(self.endpoint.clone())?;

        let channel = if self.use_tls {
            endpoint
                .tls_config(tonic::transport::ClientTlsConfig::new())?
                .connect()
                .await?
        } else {
            endpoint
                .connect()
                .await?
        };

        Ok(cas::execution_client::ExecutionClient::new(channel))
    }

    /// Execute an action remotely via gRPC
    pub fn execute_action(
        &self,
        action: &crate::re_client::Action,
        command: &crate::re_client::Command,
    ) -> io::Result<crate::re_client::ActionResult> {
        self.run_async(async {
            // 0. Upload input files to CAS
            let input_root_digest = self.upload_inputs(&action.input_files)?;

            // 1. Upload command to CAS
            let command_proto = cas::Command {
                arguments: command.arguments.clone(),
                environment_variables: vec![
                    cas::command::EnvironmentVariable {
                        name: "PATH".to_string(),
                        value: "/opt/osxcross/target/bin:/root/.nix-profile/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/run/current-system/sw/bin".to_string(),
                    },
                    cas::command::EnvironmentVariable {
                        name: "CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER".to_string(),
                        value: "aarch64-apple-darwin23.5-clang".to_string(),
                    },
                ],
                output_files: command.output_files(),
                output_directories: vec![],
                output_paths: vec![],
                platform: None,
                working_directory: command.working_directory.clone(),
                output_node_properties: vec![],
            };

            let command_bytes = {
                use prost::Message;
                let mut buf = Vec::new();
                command_proto.encode(&mut buf)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                buf
            };

            let command_digest = self.upload(&command_bytes)?;

            // 2. Create action proto
            let action_proto = cas::Action {
                command_digest: Some(cas::Digest {
                    hash: command_digest.hash.clone(),
                    size_bytes: command_digest.size_bytes,
                }),
                input_root_digest: Some(cas::Digest {
                    hash: input_root_digest.hash.clone(),
                    size_bytes: input_root_digest.size_bytes,
                }),
                timeout: Some(prost_types::Duration {
                    seconds: 300,
                    nanos: 0,
                }),
                do_not_cache: false,
                salt: vec![].into(),
                platform: action.platform.as_ref().map(|p| cas::Platform {
                    properties: p.properties.iter().map(|(k, v)| cas::platform::Property {
                        name: k.clone(),
                        value: v.clone(),
                    }).collect(),
                }),
            };

            let action_bytes = {
                use prost::Message;
                let mut buf = Vec::new();
                action_proto.encode(&mut buf)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                buf
            };

            let action_digest = self.upload(&action_bytes)?;

            // 3. Execute via gRPC
            let mut exec_client = self.connect_execution()
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            let execute_request = cas::ExecuteRequest {
                instance_name: self.instance_name.clone(),
                skip_cache_lookup: false,
                action_digest: Some(cas::Digest {
                    hash: action_digest.hash,
                    size_bytes: action_digest.size_bytes,
                }),
                execution_policy: None,
                results_cache_policy: None,
                digest_function: 0,  // 0 = UNKNOWN (use default SHA256)
            };

            let mut stream = exec_client
                .execute(execute_request)
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
                .into_inner();

            // 4. Wait for execution result with timeout
            use tokio::time::{timeout, Duration};

            let timeout_duration = Duration::from_secs(30);
            let mut final_response = None;

            loop {
                match timeout(timeout_duration, stream.message()).await {
                    Ok(Ok(Some(response))) => {
                        final_response = Some(response);
                    }
                    Ok(Ok(None)) => {
                        // Stream ended normally
                        break;
                    }
                    Ok(Err(e)) => {
                        return Err(io::Error::new(io::ErrorKind::Other, e));
                    }
                    Err(_) => {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "Remote execution timed out after 30 seconds. Worker may be missing required toolchain (rustc). Check NativeLink worker configuration."
                        ));
                    }
                }
            }

            let response = final_response
                .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "No response from execution"))?;

            // 5. Extract output from Operation result
            use nativelink_proto::google::longrunning::operation;

            let operation_result = response.result
                .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "No result in operation"))?;

            match operation_result {
                operation::Result::Error(status) => {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("Execution error: code={}, message={}", status.code, status.message)
                    ));
                }
                operation::Result::Response(any) => {
                    // Decode the Any into ExecuteResponse
                    use prost::Message;
                    let execute_response = cas::ExecuteResponse::decode(&*any.value)
                        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to decode response: {}", e)))?;

                    // Check for execution status errors
                    if let Some(status) = &execute_response.status {
                        if status.code != 0 {
                            return Err(io::Error::new(
                                io::ErrorKind::Other,
                                format!("Execution status error: code={}, message={}", status.code, status.message)
                            ));
                        }
                    }

                    let result = execute_response.result
                        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "No ActionResult in ExecuteResponse"))?;

                    // Convert proto result to our ActionResult
                    let action_result = crate::re_client::ActionResult {
                        output_files: result.output_files.iter().map(|f| {
                            crate::re_client::OutputFile {
                                path: f.path.clone(),
                                hash: f.digest.as_ref().map(|d| d.hash.clone()).unwrap_or_default(),
                                size_bytes: f.digest.as_ref().map(|d| d.size_bytes).unwrap_or(0),
                                is_executable: f.is_executable,
                            }
                        }).collect(),
                        exit_code: result.exit_code,
                        stdout: result.stdout_raw.to_vec(),
                        stderr: result.stderr_raw.to_vec(),
                    };

                    Ok(action_result)
                }
            }
        })
    }

    /// Download output files from remote execution result
    pub fn download_outputs(
        &self,
        action_result: &crate::re_client::ActionResult,
        output_dir: &std::path::Path,
    ) -> io::Result<()> {
        for output_file in &action_result.output_files {
            // Download file from CAS
            let digest = crate::re_client::Digest::new(
                output_file.hash.clone(),
                output_file.size_bytes,
            );

            let data = self.download(&digest)?;

            // Write to output directory
            let full_path = output_dir.join(&output_file.path);

            // Create parent directories if needed
            if let Some(parent) = full_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            std::fs::write(&full_path, data)?;

            // Set executable permission if needed
            if output_file.is_executable {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = std::fs::metadata(&full_path)?.permissions();
                    perms.set_mode(0o755);
                    std::fs::set_permissions(&full_path, perms)?;
                }
            }
        }

        Ok(())
    }

    /// Upload input files to remote CAS and return input root digest
    pub fn upload_inputs(
        &self,
        input_files: &[std::path::PathBuf],
    ) -> io::Result<crate::re_client::Digest> {
        use crate::directory_tree::DirectoryTreeBuilder;

        // 1. Upload all file contents to CAS first
        // This must happen before uploading the Directory, because Directory
        // contains digests that reference these files in CAS
        for file in input_files {
            if file.exists() {
                let contents = std::fs::read(file)?;
                self.upload(&contents)?;
            }
        }

        // 2. Build directory tree
        let builder = DirectoryTreeBuilder::new();
        let directory = builder.build(input_files)?;

        // 3. Serialize and upload Directory proto
        let dir_bytes = directory.to_proto_bytes()?;
        let dir_digest = self.upload(&dir_bytes)?;

        Ok(dir_digest)
    }
}
