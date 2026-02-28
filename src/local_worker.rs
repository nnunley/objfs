// Local worker management
// Ensures a local NativeLink worker is running and available

use std::process::{Command, Child};
use std::net::TcpStream;
use std::time::Duration;
use std::path::PathBuf;

/// Check if a local worker is already running
pub fn is_worker_running() -> bool {
    TcpStream::connect_timeout(
        &"127.0.0.1:50062".parse().unwrap(),
        Duration::from_millis(100)
    ).is_ok()
}

/// Start a local worker process in the background
/// Returns the child process handle that should be kept alive
pub fn start_local_worker() -> std::io::Result<Child> {
    // Generate a minimal worker config on-the-fly
    let config = generate_worker_config()?;

    // Start nativelink worker process
    let child = Command::new("nativelink")
        .arg(&config)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;

    // Give it a moment to start
    std::thread::sleep(Duration::from_millis(500));

    Ok(child)
}

/// Generate a minimal worker configuration
fn generate_worker_config() -> std::io::Result<PathBuf> {
    use std::io::Write;

    let config_dir = dirs::cache_dir()
        .ok_or_else(|| std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not find cache directory"
        ))?
        .join("objfs");

    std::fs::create_dir_all(&config_dir)?;

    let config_path = config_dir.join("worker-config.json5");

    // Get scheduler endpoint from environment or default to localhost
    let scheduler_endpoint = std::env::var("OBJFS_REMOTE_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:50051".to_string());

    let cache_dir = dirs::cache_dir()
        .unwrap()
        .join("objfs/worker");

    std::fs::create_dir_all(&cache_dir)?;

    let config = format!(r#"{{
  stores: [
    {{
      name: "CAS_MAIN",
      grpc: {{
        instance_name: "main",
        endpoints: [{{ uri: "{}" }}],
        store_type: "cas",
      }},
    }},
    {{
      name: "WORKER_FAST",
      filesystem: {{
        content_path: "{}/fast/content",
        temp_path: "{}/fast/tmp",
        eviction_policy: {{ max_bytes: 5368709120 }},
      }},
    }},
    {{
      name: "WORKER_CAS_FAST_SLOW",
      fast_slow: {{
        fast: {{ ref_store: {{ name: "WORKER_FAST" }} }},
        slow: {{ ref_store: {{ name: "CAS_MAIN" }} }},
      }},
    }},
  ],

  workers: [{{
    local: {{
      worker_api_endpoint: {{ uri: "grpc://{}/worker" }},
      cas_fast_slow_store: "WORKER_CAS_FAST_SLOW",
      upload_action_result: {{ ac_store: "CAS_MAIN" }},
      work_directory: "{}/work",
      platform_properties: {{
        OSFamily: {{ values: ["{}"] }},
        ISA: {{ values: ["{}"] }},
        "container-image": {{ values: ["rust:latest"] }},
      }},
    }},
  }}],

  servers: [],
}}"#,
        scheduler_endpoint,
        cache_dir.display(),
        cache_dir.display(),
        scheduler_endpoint.replace("http://", "").replace("https://", ""),
        cache_dir.display(),
        get_os_family(),
        get_isa(),
    );

    let mut file = std::fs::File::create(&config_path)?;
    file.write_all(config.as_bytes())?;

    Ok(config_path)
}

/// Get the OS family for platform properties
fn get_os_family() -> &'static str {
    #[cfg(target_os = "macos")]
    { "darwin" }
    #[cfg(target_os = "linux")]
    { "linux" }
    #[cfg(target_os = "windows")]
    { "windows" }
}

/// Get the ISA (instruction set architecture) for platform properties
fn get_isa() -> &'static str {
    #[cfg(target_arch = "aarch64")]
    { "aarch64" }
    #[cfg(target_arch = "x86_64")]
    { "x86-64" }
    #[cfg(target_arch = "x86")]
    { "x86" }
}

/// Ensure a local worker is running
/// Returns a process handle that should be kept alive for the lifetime of the program
pub fn ensure_local_worker() -> std::io::Result<Option<Child>> {
    if is_worker_running() {
        // Worker already running (system service or previous instance)
        Ok(None)
    } else {
        // Try to start a local worker
        match start_local_worker() {
            Ok(child) => {
                // Verify it started
                std::thread::sleep(Duration::from_millis(500));
                if is_worker_running() {
                    Ok(Some(child))
                } else {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Worker process started but not responding"
                    ))
                }
            }
            Err(e) => {
                eprintln!("[objfs] Warning: Could not start local worker: {}", e);
                eprintln!("[objfs] Falling back to direct compilation");
                Ok(None)
            }
        }
    }
}
