use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::io;
use sha2::{Sha256, Digest};

use objfs::cas::Cas;
use objfs::bundle::ArtifactBundle;
use objfs::output_detection;
use objfs::platform::Platform;
use objfs::remote_config::RemoteConfig;
use objfs::re_client::{Command as ReCommand, Action, RemoteExecutor};
use objfs::grpc_client::GrpcRemoteCas;
use objfs::local_worker;

fn main() {
    // Ensure local worker is running (auto-register with scheduler)
    // Skip if OBJFS_NO_AUTO_WORKER is set
    if env::var("OBJFS_NO_AUTO_WORKER").is_err() {
        let _worker_handle = local_worker::ensure_local_worker()
            .ok();  // Ignore errors - will fall back to remote or direct compilation
    }

    if let Err(e) = run() {
        eprintln!("cargo-objfs-rustc error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let mut args: Vec<String> = env::args().skip(1).collect();

    // Cargo calls us with "rustc" as first arg - skip it
    if args.first().map(|s| s.as_str()) == Some("rustc") {
        args = args.into_iter().skip(1).collect();
    }

    // If CAS is disabled, just pass through to rustc
    if env::var("OBJFS_DISABLE").is_ok() {
        return exec_rustc(&args);
    }

    // Pass through for metadata queries and special invocations
    if args.iter().any(|a| a == "--print" || a == "--version" || a == "-vV" || a == "--crate-name" && args.contains(&"___".to_string())) {
        return exec_rustc(&args);
    }

    // Parse rustc invocation to understand what we're building
    let build_info = parse_rustc_args(&args);

    // Check if this is a compilation we should cache
    if !should_cache(&build_info) {
        return exec_rustc(&args);
    }

    // Compute cache key from inputs
    let cache_key = compute_cache_key(&build_info)?;

    // Check CAS for cached result
    let cas = Cas::new(Cas::default_location())?;

    if let Some(output) = check_cache(&cas, &cache_key)? {
        // Verify bundle is complete before attempting restore
        if ArtifactBundle::is_complete(&cas, &output)? {
            // Cache hit! Restore outputs
            restore_outputs(&cas, &output, &build_info)?;
            eprintln!("[objfs] cache hit: {}", build_info.output_file.display());
            return Ok(());
        } else {
            // Bundle incomplete (partial expiration) - treat as cache miss
            eprintln!("[objfs] cache miss (incomplete bundle): {}", build_info.output_file.display());
        }
    }

    // Cache miss - check if we should use remote execution
    let remote_config = RemoteConfig::from_env();

    // Determine target triple and input size
    // If no --target flag, use host's default target
    let target_triple = build_info.target_triple.as_deref()
        .unwrap_or_else(|| {
            // Default to host target when not specified
            #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
            { "aarch64-apple-darwin" }
            #[cfg(all(target_arch = "x86_64", target_os = "macos"))]
            { "x86_64-apple-darwin" }
            #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
            { "x86_64-unknown-linux-gnu" }
            #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
            { "aarch64-unknown-linux-gnu" }
            #[cfg(not(any(
                all(target_arch = "aarch64", target_os = "macos"),
                all(target_arch = "x86_64", target_os = "macos"),
                all(target_arch = "x86_64", target_os = "linux"),
                all(target_arch = "aarch64", target_os = "linux")
            )))]
            { "unknown" }
        });
    let input_size: u64 = build_info.input_files.iter()
        .filter_map(|f| std::fs::metadata(f).ok())
        .map(|m| m.len())
        .sum();

    // Check if this is a link operation
    // Link operations can be remote if workers support the target platform
    // Otherwise they must run locally
    let is_link = is_link_operation(&build_info);

    if is_link {
        // Link operation - check if remote workers can handle this platform
        if remote_config.can_build_target(target_triple) {
            eprintln!("[objfs] link operation for {} - trying platform-compatible remote worker", target_triple);

            // Try remote execution on platform-compatible worker
            match try_remote_execution(&remote_config, &build_info, &args) {
                Ok(()) => {
                    eprintln!("[objfs] remote link succeeded");
                    return Ok(());
                }
                Err(e) => {
                    eprintln!("[objfs] remote link failed: {}, falling back to local", e);
                    // Fall through to local execution
                }
            }
        } else {
            eprintln!("[objfs] link operation for {} - no compatible remote workers, executing locally", target_triple);
            return execute_and_cache_local(&build_info, &args, &cas, &cache_key);
        }
    } else if remote_config.should_use_remote(target_triple, input_size) {
        eprintln!("[objfs] remote execution: target={}, size={} bytes", target_triple, input_size);

        // Try remote execution
        match try_remote_execution(&remote_config, &build_info, &args) {
            Ok(()) => {
                eprintln!("[objfs] remote execution succeeded");
                return Ok(());
            }
            Err(e) => {
                eprintln!("[objfs] remote execution failed: {}, falling back to local", e);
                // Fall through to local compilation
            }
        }
    }

    // Compile locally (either no remote config, or remote failed)
    eprintln!("[objfs] cache miss: {}", build_info.output_file.display());

    // List files before compilation
    let output_dir = build_info.output_file.parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "No output directory"))?;
    let files_before = output_detection::list_directory_files(output_dir).unwrap_or_default();

    // Compile
    exec_rustc(&args)?;

    // Detect actual output files created by rustc
    let actual_outputs = output_detection::detect_new_files(output_dir, &files_before)?;

    if !actual_outputs.is_empty() {
        // Store result in CAS as bundle
        let bundle = ArtifactBundle::new(actual_outputs.clone());
        if let Ok(hash) = bundle.store(&cas) {
            store_cache_entry(&cas, &cache_key, &hash)?;
            eprintln!("[objfs] cached bundle: {} files -> {}", actual_outputs.len(), &hash[..8]);
        }
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuildInfo {
    pub output_file: PathBuf,
    pub output_files: Vec<PathBuf>, // All outputs from --emit
    pub input_files: Vec<PathBuf>,
    pub flags: Vec<String>,
    pub target_triple: Option<String>, // e.g., "x86_64-unknown-linux-gnu"
}

pub fn parse_rustc_args(args: &[String]) -> BuildInfo {
    let mut output_dir = None;
    let mut output_file = None;
    let mut crate_name = None;
    let mut emit_types: Vec<&str> = vec!["link"]; // default
    let mut input_files = Vec::new();
    let mut flags = Vec::new();
    let mut target_triple = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--target" => {
                if i + 1 < args.len() {
                    target_triple = Some(args[i + 1].clone());
                    flags.push(args[i].clone());
                    flags.push(args[i + 1].clone());
                    i += 1;
                }
            }
            "--out-dir" => {
                if i + 1 < args.len() {
                    output_dir = Some(PathBuf::from(&args[i + 1]));
                    flags.push(args[i].clone());
                    flags.push(args[i + 1].clone());
                    i += 1;
                }
            }
            "-o" => {
                if i + 1 < args.len() {
                    output_file = Some(PathBuf::from(&args[i + 1]));
                    flags.push(args[i].clone());
                    flags.push(args[i + 1].clone());
                    i += 1;
                }
            }
            "--crate-name" => {
                if i + 1 < args.len() {
                    crate_name = Some(args[i + 1].clone());
                    flags.push(args[i].clone());
                    flags.push(args[i + 1].clone());
                    i += 1;
                }
            }
            "--emit" => {
                if i + 1 < args.len() {
                    emit_types = args[i + 1].split(',').collect();
                    flags.push(args[i].clone());
                    flags.push(args[i + 1].clone());
                    i += 1;
                }
            }
            "--crate-type" => {
                if i + 1 < args.len() {
                    flags.push(args[i].clone());
                    flags.push(args[i + 1].clone());
                    i += 1;
                }
            }
            arg if arg.ends_with(".rs") => {
                input_files.push(PathBuf::from(arg));
            }
            _ => {
                flags.push(args[i].clone());
            }
        }
        i += 1;
    }

    // Determine actual output file and all outputs
    let (final_output, output_files) = if let Some(out) = output_file {
        (out.clone(), vec![out])
    } else if let (Some(dir), Some(name)) = (&output_dir, &crate_name) {
        let main_output = dir.join(format!("lib{}.rlib", name));

        // Build list of all output files based on --emit
        let mut all_outputs = Vec::new();
        for emit_type in emit_types {
            let file = match emit_type {
                "link" => dir.join(format!("lib{}.rlib", name)),
                "dep-info" => dir.join(format!("lib{}.d", name)),
                "metadata" => dir.join(format!("lib{}.rmeta", name)),
                _ => continue,
            };
            all_outputs.push(file);
        }

        (main_output, all_outputs)
    } else {
        let default_output = PathBuf::from("output.rlib");
        (default_output.clone(), vec![default_output])
    };

    BuildInfo {
        output_file: final_output,
        output_files,
        input_files,
        flags,
        target_triple,
    }
}

fn should_cache(build_info: &BuildInfo) -> bool {
    // Only cache if we have an output file
    !build_info.output_file.as_os_str().is_empty()
}

/// Detect if this is a link-only operation
/// Link operations take .rlib files as input and produce binaries
fn is_link_operation(build_info: &BuildInfo) -> bool {
    // Check if all inputs are .rlib, .rmeta, or .a files (pre-compiled)
    let all_precompiled = !build_info.input_files.is_empty() &&
        build_info.input_files.iter().all(|f| {
            matches!(
                f.extension().and_then(|e| e.to_str()),
                Some("rlib") | Some("rmeta") | Some("a") | Some("so") | Some("dylib")
            )
        });

    // Check if output is a binary (not a library)
    let output_is_binary = build_info.output_file
        .extension()
        .and_then(|e| e.to_str())
        .map(|ext| !matches!(ext, "rlib" | "rmeta" | "a"))
        .unwrap_or(true);  // If no extension, assume binary

    all_precompiled && output_is_binary
}

/// Execute locally and cache the result
fn execute_and_cache_local(
    build_info: &BuildInfo,
    args: &[String],
    cas: &Cas,
    cache_key: &str,
) -> io::Result<()> {
    let output_dir = build_info.output_file.parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "No output directory"))?;
    let files_before = output_detection::list_directory_files(output_dir).unwrap_or_default();

    // Execute locally
    exec_rustc(args)?;

    // Detect and cache outputs
    let actual_outputs = output_detection::detect_new_files(output_dir, &files_before)?;

    if !actual_outputs.is_empty() {
        let bundle = ArtifactBundle::new(actual_outputs.clone());
        if let Ok(hash) = bundle.store(cas) {
            store_cache_entry(cas, cache_key, &hash)?;
            eprintln!("[objfs] cached bundle: {} files -> {}", actual_outputs.len(), &hash[..8]);
        }
    }

    Ok(())
}

fn compute_cache_key(build_info: &BuildInfo) -> io::Result<String> {
    let mut hasher = Sha256::new();

    // Hash platform (CRITICAL: prevents cross-platform cache collisions)
    // Use target platform if cross-compiling, otherwise host platform
    let platform = if let Some(target) = &build_info.target_triple {
        Platform::from_target_triple(target)
            .unwrap_or_else(|| Platform::detect())
    } else {
        Platform::detect()
    };
    hasher.update(platform.to_cache_key_string().as_bytes());
    hasher.update(b"\0");

    // Hash all input files
    for input in &build_info.input_files {
        if input.exists() {
            let data = std::fs::read(input)?;
            hasher.update(&data);
        }
    }

    // Hash compilation flags (sorted for stability)
    let mut sorted_flags = build_info.flags.clone();
    sorted_flags.sort();
    for flag in sorted_flags {
        hasher.update(flag.as_bytes());
        hasher.update(b"\0");
    }

    Ok(hex::encode(hasher.finalize()))
}

fn check_cache(cas: &Cas, cache_key: &str) -> io::Result<Option<String>> {
    let cache_index = Cas::default_location().join("index.json");

    if !cache_index.exists() {
        return Ok(None);
    }

    let data = std::fs::read_to_string(cache_index)?;
    let index: serde_json::Value = serde_json::from_str(&data)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    if let Some(hash) = index.get(cache_key).and_then(|v| v.as_str()) {
        let hash_string = hash.to_string();
        if cas.exists(&hash_string) {
            return Ok(Some(hash_string));
        }
    }

    Ok(None)
}

fn restore_outputs(cas: &Cas, hash: &str, build_info: &BuildInfo) -> io::Result<()> {
    // Get output directory from first output file
    let output_dir = build_info.output_file.parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "No output directory"))?;

    // Restore bundle to output directory
    ArtifactBundle::restore(cas, hash, output_dir)?;
    Ok(())
}

fn store_cache_entry(_cas: &Cas, cache_key: &str, hash: &str) -> io::Result<()> {
    let cache_index = Cas::default_location().join("index.json");

    let mut index: serde_json::Map<String, serde_json::Value> = if cache_index.exists() {
        let data = std::fs::read_to_string(&cache_index)?;
        serde_json::from_str(&data)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
    } else {
        serde_json::Map::new()
    };

    index.insert(cache_key.to_string(), serde_json::Value::String(hash.to_string()));

    let data = serde_json::to_string_pretty(&index)?;
    std::fs::write(cache_index, data)?;

    Ok(())
}

fn try_remote_execution(
    config: &RemoteConfig,
    build_info: &BuildInfo,
    args: &[String],
) -> io::Result<()> {
    // Get endpoint from config
    let endpoint = config.endpoint.as_ref()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "No remote endpoint"))?;

    // Determine if we should use TLS
    let use_tls = endpoint.starts_with("https://") || endpoint.contains(":443");

    // Create remote executor
    let executor = RemoteExecutor::new(
        endpoint.clone(),
        config.instance_name.clone(),
        use_tls,
    );

    // Create command from rustc args
    let working_dir = build_info.output_file.parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "No working directory"))?
        .to_path_buf();

    let command = ReCommand::from_rustc_args(
        &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        &working_dir,
    );

    // Create action with input files
    let action = Action::new(command, build_info.input_files.clone());

    // Execute remotely
    let result = executor.execute(&action)?;

    // Check exit code
    if result.exit_code != 0 {
        eprintln!("[objfs] remote execution failed with exit code {}", result.exit_code);
        if !result.stdout.is_empty() {
            eprintln!("[objfs] stdout: {}", String::from_utf8_lossy(&result.stdout));
        }
        if !result.stderr.is_empty() {
            eprintln!("[objfs] stderr: {}", String::from_utf8_lossy(&result.stderr));
        }
        eprintln!("[objfs] output_files count: {}", result.output_files.len());
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("Remote compilation failed with exit code {}", result.exit_code)
        ));
    }

    // Download output files from remote CAS
    let output_dir = build_info.output_file.parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "No output directory"))?;

    // Normalize endpoint for gRPC client
    let grpc_endpoint = if use_tls {
        if !endpoint.starts_with("https://") {
            format!("https://{}", endpoint)
        } else {
            endpoint.clone()
        }
    } else {
        if !endpoint.starts_with("http://") {
            format!("http://{}", endpoint)
        } else {
            endpoint.clone()
        }
    };

    let grpc_client = GrpcRemoteCas::new(grpc_endpoint, config.instance_name.clone());
    grpc_client.download_outputs(&result, output_dir)?;

    eprintln!("[objfs] downloaded {} output files from remote", result.output_files.len());
    Ok(())
}

fn exec_rustc(args: &[String]) -> io::Result<()> {
    let rustc = env::var("OBJFS_REAL_RUSTC")
        .unwrap_or_else(|_| "rustc".to_string());

    let status = Command::new(rustc)
        .args(args)
        .status()?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rlib_output() {
        let args = vec![
            "--crate-name".to_string(),
            "mylib".to_string(),
            "--crate-type".to_string(),
            "rlib".to_string(),
            "--out-dir".to_string(),
            "/path/to/target/debug/deps".to_string(),
            "src/lib.rs".to_string(),
        ];

        let build_info = parse_rustc_args(&args);

        assert_eq!(
            build_info.output_file,
            PathBuf::from("/path/to/target/debug/deps/libmylib.rlib")
        );
    }

    #[test]
    fn test_parse_bin_output_with_explicit_output() {
        let args = vec![
            "--crate-name".to_string(),
            "mybin".to_string(),
            "--crate-type".to_string(),
            "bin".to_string(),
            "-o".to_string(),
            "/path/to/target/debug/mybin".to_string(),
            "src/main.rs".to_string(),
        ];

        let build_info = parse_rustc_args(&args);

        assert_eq!(
            build_info.output_file,
            PathBuf::from("/path/to/target/debug/mybin")
        );
    }

    #[test]
    fn test_parse_collects_input_files() {
        let args = vec![
            "src/lib.rs".to_string(),
            "src/helper.rs".to_string(),
            "--crate-name".to_string(),
            "test".to_string(),
        ];

        let build_info = parse_rustc_args(&args);

        assert_eq!(build_info.input_files.len(), 2);
        assert!(build_info.input_files.contains(&PathBuf::from("src/lib.rs")));
        assert!(build_info.input_files.contains(&PathBuf::from("src/helper.rs")));
    }

    #[test]
    fn test_parse_preserves_flags() {
        let args = vec![
            "--edition".to_string(),
            "2024".to_string(),
            "-C".to_string(),
            "debuginfo=2".to_string(),
            "--crate-name".to_string(),
            "test".to_string(),
        ];

        let build_info = parse_rustc_args(&args);

        assert!(build_info.flags.contains(&"--edition".to_string()));
        assert!(build_info.flags.contains(&"2024".to_string()));
        assert!(build_info.flags.contains(&"-C".to_string()));
        assert!(build_info.flags.contains(&"debuginfo=2".to_string()));
    }
}
