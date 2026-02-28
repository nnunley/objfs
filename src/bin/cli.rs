use std::env;
use std::path::PathBuf;
use std::io;

use objfs::cas::Cas;

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
