use std::env;
use std::path::{Path, PathBuf};
use std::io;

use objfs::cas::Cas;
use objfs::config::{ObjfsConfig, detect_project_type};

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        print_usage();
        return Ok(());
    }

    match args[0].as_str() {
        "init" => cmd_init(&args[1..]),
        "stats" => cmd_stats(),
        "clear" => cmd_clear(),
        "enable" => cmd_enable(),
        "disable" => cmd_disable(),
        "evict" => {
            let days = if args.len() > 1 {
                args[1].parse().unwrap_or(30)
            } else {
                30
            };
            cmd_evict(days)
        }
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        _ => {
            eprintln!("Unknown command: {}", args[0]);
            print_usage();
            std::process::exit(1);
        }
    }
}

fn print_usage() {
    println!("objfs - Object Filesystem for Build Artifacts");
    println!();
    println!("USAGE:");
    println!("    objfs <COMMAND>");
    println!();
    println!("COMMANDS:");
    println!("    init          Initialize objfs for current project");
    println!("    init --config <file>    Apply shared config (TOML or JSON5)");
    println!("    init --export-config    Export current config to objfs.toml");
    println!("    stats         Show CAS statistics");
    println!("    clear         Clear all cached objects");
    println!("    evict [days]  Evict objects older than N days (default: 30)");
    println!("    enable        Enable objfs for current project");
    println!("    disable       Disable objfs for current project");
    println!("    help          Print this help message");
    println!();
    println!("ENVIRONMENT:");
    println!("    OBJFS_DISABLE=1    Disable caching (pass through to rustc)");
    println!();
    println!("To enable for Cargo builds, add to .cargo/config.toml:");
    println!("    [build]");
    println!("    rustc-wrapper = \"cargo-objfs-rustc\"");
}

fn cmd_stats() -> io::Result<()> {
    let cas = Cas::new(Cas::default_location())?;
    let stats = cas.stats()?;

    println!("CAS Statistics");
    println!("==============");
    println!("Location:      {}", Cas::default_location().display());
    println!("Objects:       {}", stats.object_count);
    println!("Total size:    {} MB", stats.total_size / 1_000_000);
    println!("Avg size:      {} KB",
             if stats.object_count > 0 {
                 (stats.total_size / stats.object_count) / 1000
             } else {
                 0
             });

    Ok(())
}

fn cmd_clear() -> io::Result<()> {
    let cas_dir = Cas::default_location();

    println!("Clearing CAS at: {}", cas_dir.display());

    if cas_dir.exists() {
        std::fs::remove_dir_all(&cas_dir)?;
        println!("CAS cleared successfully");
    } else {
        println!("CAS directory does not exist");
    }

    Ok(())
}

fn cmd_enable() -> io::Result<()> {
    let config_dir = PathBuf::from(".cargo");
    let config_file = config_dir.join("config.toml");

    std::fs::create_dir_all(&config_dir)?;

    let config_content = r#"[build]
rustc-wrapper = "cargo-objfs-rustc"
"#;

    if config_file.exists() {
        println!("Warning: .cargo/config.toml already exists");
        println!("Please manually add:");
        println!("{}", config_content);
    } else {
        std::fs::write(&config_file, config_content)?;
        println!("objfs enabled for this project");
        println!("Created: {}", config_file.display());
    }

    Ok(())
}

fn cmd_disable() -> io::Result<()> {
    let config_file = PathBuf::from(".cargo/config.toml");

    if !config_file.exists() {
        println!("No .cargo/config.toml found");
        return Ok(());
    }

    let content = std::fs::read_to_string(&config_file)?;
    let new_content = content
        .lines()
        .filter(|line| !line.contains("rustc-wrapper"))
        .collect::<Vec<_>>()
        .join("\n");

    std::fs::write(&config_file, new_content)?;
    println!("objfs disabled for this project");

    Ok(())
}

fn cmd_init(args: &[String]) -> io::Result<()> {
    let cwd = env::current_dir()?;

    // Handle --export-config
    if args.iter().any(|a| a.starts_with("--export-config")) {
        return cmd_export_config(args);
    }

    // Load base config: from --config file, existing objfs.toml, or defaults
    let mut config = if let Some(pos) = args.iter().position(|a| a == "--config") {
        let path = args.get(pos + 1)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "--config requires a file path"))?;
        let path = PathBuf::from(path);
        if !path.is_file() {
            return Err(io::Error::new(io::ErrorKind::NotFound, format!("Config file not found: {}", path.display())));
        }
        println!("Loading config from: {}", path.display());
        ObjfsConfig::from_file(&path)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?
    } else if let Some(path) = ObjfsConfig::find_in_ancestors(&cwd) {
        println!("Found existing config: {}", path.display());
        ObjfsConfig::from_file(&path)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?
    } else {
        ObjfsConfig::default()
    };

    // Auto-detect project type if not set
    if config.project.project_type.is_empty() {
        config.project.project_type = detect_project_type(&cwd).to_string();
        println!("Detected project type: {}", config.project.project_type);
    }

    // Prompt for remote endpoint if not set and no --config was given
    if config.remote.endpoint.is_none() && !args.iter().any(|a| a == "--config") {
        eprint!("Remote endpoint (empty for local-only): ");
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        if !input.is_empty() {
            config.remote.endpoint = Some(input.to_string());
        }
    }

    // Write objfs.toml
    let config_path = cwd.join("objfs.toml");
    config.write_to_file(&config_path)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    println!("Wrote: {}", config_path.display());

    // Configure build system integration
    match config.project.project_type.as_str() {
        "rust" | "mixed" => configure_rust_project(&cwd)?,
        "cmake" => print_cmake_instructions(),
        "make" => print_make_instructions(),
        _ => eprintln!("Warning: Unknown project type; skipping build system configuration"),
    }

    // Verify remote connectivity
    if let Some(ref endpoint) = config.remote.endpoint {
        verify_endpoint(endpoint);
    }

    println!();
    println!("objfs initialized. Configuration saved to objfs.toml.");

    Ok(())
}

fn cmd_export_config(args: &[String]) -> io::Result<()> {
    let config = ObjfsConfig::load()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    let export_arg = args.iter().find(|a| a.starts_with("--export-config")).unwrap();

    let path = if export_arg.contains("=json5") {
        PathBuf::from("objfs.json5")
    } else {
        PathBuf::from("objfs.toml")
    };

    config.write_to_file(&path)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    println!("Exported config to: {}", path.display());
    Ok(())
}

fn configure_rust_project(project_dir: &Path) -> io::Result<()> {
    let config_dir = project_dir.join(".cargo");
    let config_file = config_dir.join("config.toml");

    std::fs::create_dir_all(&config_dir)?;

    let wrapper_line = "rustc-wrapper = \"cargo-objfs-rustc\"";

    if config_file.exists() {
        let content = std::fs::read_to_string(&config_file)?;
        if content.contains("cargo-objfs-rustc") {
            println!(".cargo/config.toml already configured");
            return Ok(());
        }
        if content.contains("rustc-wrapper") {
            eprintln!("Warning: .cargo/config.toml has a different rustc-wrapper");
            eprintln!("  Please update it manually to:");
            eprintln!("  {}", wrapper_line);
            return Ok(());
        }
        // Append to existing file
        let mut new_content = content;
        if !new_content.contains("[build]") {
            new_content.push_str("\n[build]\n");
        }
        new_content.push_str(&format!("{}\n", wrapper_line));
        std::fs::write(&config_file, new_content)?;
    } else {
        std::fs::write(&config_file, format!("[build]\n{}\n", wrapper_line))?;
    }
    println!("Configured: {}", config_file.display());

    Ok(())
}

fn print_cmake_instructions() {
    println!();
    println!("For CMake, add to your build command:");
    println!("  cmake .. \\");
    println!("    -DCMAKE_C_COMPILER_LAUNCHER=/usr/local/bin/objfs-cc-wrapper \\");
    println!("    -DCMAKE_CXX_COMPILER_LAUNCHER=/usr/local/bin/objfs-cc-wrapper");
}

fn print_make_instructions() {
    println!();
    println!("For Make, set CC and CXX:");
    println!("  CC=\"objfs-cc-wrapper gcc\" CXX=\"objfs-cc-wrapper g++\" make");
}

fn verify_endpoint(endpoint: &str) {
    let health_url = format!("{}/health", endpoint.trim_end_matches('/'));
    print!("Checking {}... ", health_url);

    match reqwest::blocking::Client::new()
        .get(&health_url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
    {
        Ok(resp) if resp.status().is_success() => println!("OK"),
        Ok(resp) => eprintln!("Warning: server returned {}", resp.status()),
        Err(e) => eprintln!("Warning: unreachable ({})", e),
    }
}

fn cmd_evict(days: u64) -> io::Result<()> {
    use std::time::Duration;

    let cas = Cas::new(Cas::default_location())?;
    let ttl = Duration::from_secs(86400 * days);

    println!("Evicting objects older than {} days...", days);

    let stats = cas.evict_expired_objects(ttl)?;

    println!("Evicted {} objects", stats.objects_evicted);
    println!("Freed {} MB", stats.bytes_freed / 1_000_000);

    Ok(())
}
