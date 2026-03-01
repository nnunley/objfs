// objfs C/C++ compiler wrapper for CMake integration
// Works as CMAKE_C_COMPILER_LAUNCHER / CMAKE_CXX_COMPILER_LAUNCHER

use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::io;
use sha2::{Sha256, Digest};

use objfs::cas::Cas;

fn main() {
    if let Err(e) = run() {
        eprintln!("objfs-cc-wrapper error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("Usage: objfs-cc-wrapper <compiler> [args...]");
        eprintln!("Example: objfs-cc-wrapper gcc -c foo.c -o foo.o");
        std::process::exit(1);
    }

    // First arg is the actual compiler
    let compiler = &args[0];
    let compiler_args = &args[1..];

    // Future: load ObjfsConfig for remote C/C++ execution support
    // let _config = objfs::config::ObjfsConfig::load();

    // If CAS is disabled, just pass through
    if env::var("OBJFS_DISABLE").is_ok() {
        exec_compiler(compiler, compiler_args);
        return Ok(());
    }

    // Parse compilation command
    let build_info = match parse_compiler_args(compiler, compiler_args) {
        Some(info) => info,
        None => {
            // Not a compilation command - pass through
            exec_compiler(compiler, compiler_args);
            return Ok(());
        }
    };

    // Compute cache key from inputs
    let cache_key = compute_cache_key(&build_info)?;

    // Check CAS for cached result
    let cas = Cas::new(Cas::default_location())?;

    if let Some(output_hash) = check_cache(&cas, &cache_key)? {
        // Cache hit! Restore output
        if let Ok(()) = restore_output(&cas, &output_hash, &build_info.output) {
            eprintln!("[objfs-cc] cache hit: {}", build_info.output.display());
            return Ok(());
        }
    }

    // Cache miss
    eprintln!("[objfs-cc] cache miss: {}", build_info.output.display());

    // Compile
    exec_compiler(compiler, compiler_args);

    // Store result in CAS
    if build_info.output.exists() {
        if let Ok(hash) = cas.store_file(&build_info.output) {
            store_cache_entry(&cas, &cache_key, &hash)?;
            eprintln!("[objfs-cc] cached: {} -> {}", build_info.output.display(), &hash[..8]);
        }
    }

    Ok(())
}

#[derive(Debug)]
struct CompilationInfo {
    input: PathBuf,
    output: PathBuf,
    compiler: String,
    flags: Vec<String>,
}

fn parse_compiler_args(compiler: &str, args: &[String]) -> Option<CompilationInfo> {
    let mut input = None;
    let mut output = None;
    let mut flags = Vec::new();
    let mut is_compilation = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            // Compilation flag
            "-c" => {
                is_compilation = true;
                flags.push(args[i].clone());
            }
            // Output file
            "-o" => {
                if i + 1 < args.len() {
                    output = Some(PathBuf::from(&args[i + 1]));
                    flags.push(args[i].clone());
                    flags.push(args[i + 1].clone());
                    i += 1;
                }
            }
            // Skip linking/preprocessing-only modes
            "-E" | "-S" | "-M" | "-MM" => {
                // Preprocessing or assembly only - don't cache
                return None;
            }
            // Input file (ends with .c, .cc, .cpp, .cxx, .C)
            arg if arg.ends_with(".c") ||
                   arg.ends_with(".cc") ||
                   arg.ends_with(".cpp") ||
                   arg.ends_with(".cxx") ||
                   arg.ends_with(".C") => {
                input = Some(PathBuf::from(arg));
                // Don't add to flags - input is positional
            }
            // Other flags
            arg => {
                flags.push(arg.to_string());
            }
        }
        i += 1;
    }

    // Only cache if this is a compilation command with input/output
    if is_compilation && input.is_some() && output.is_some() {
        Some(CompilationInfo {
            input: input.unwrap(),
            output: output.unwrap(),
            compiler: compiler.to_string(),
            flags,
        })
    } else {
        None
    }
}

fn compute_cache_key(build_info: &CompilationInfo) -> io::Result<String> {
    let mut hasher = Sha256::new();

    // Hash compiler executable (name only, not path)
    hasher.update(build_info.compiler.as_bytes());

    // Hash input file contents
    let input_data = std::fs::read(&build_info.input)?;
    hasher.update(&input_data);

    // Hash compilation flags (affects output)
    for flag in &build_info.flags {
        hasher.update(flag.as_bytes());
        hasher.update(b"\0"); // Separator
    }

    Ok(hex::encode(hasher.finalize()))
}

fn check_cache(cas: &Cas, cache_key: &str) -> io::Result<Option<String>> {
    let cache_file = cas.root().join("cache").join(cache_key);
    if cache_file.exists() {
        let hash = std::fs::read_to_string(cache_file)?;
        Ok(Some(hash.trim().to_string()))
    } else {
        Ok(None)
    }
}

fn store_cache_entry(cas: &Cas, cache_key: &str, output_hash: &str) -> io::Result<()> {
    let cache_dir = cas.root().join("cache");
    std::fs::create_dir_all(&cache_dir)?;
    let cache_file = cache_dir.join(cache_key);
    std::fs::write(cache_file, output_hash)?;
    Ok(())
}

fn restore_output(cas: &Cas, output_hash: &String, output_path: &PathBuf) -> io::Result<()> {
    cas.get_to_file(output_hash, output_path)
}

fn exec_compiler(compiler: &str, args: &[String]) {
    let status = Command::new(compiler)
        .args(args)
        .status()
        .unwrap_or_else(|e| {
            eprintln!("Failed to execute {}: {}", compiler, e);
            std::process::exit(1);
        });

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_compilation() {
        let args = vec![
            "-c".to_string(),
            "foo.c".to_string(),
            "-o".to_string(),
            "foo.o".to_string(),
        ];

        let info = parse_compiler_args("gcc", &args).unwrap();
        assert_eq!(info.input, PathBuf::from("foo.c"));
        assert_eq!(info.output, PathBuf::from("foo.o"));
        assert_eq!(info.compiler, "gcc");
    }

    #[test]
    fn test_parse_with_flags() {
        let args = vec![
            "-c".to_string(),
            "-O2".to_string(),
            "-Wall".to_string(),
            "foo.cpp".to_string(),
            "-o".to_string(),
            "foo.o".to_string(),
        ];

        let info = parse_compiler_args("g++", &args).unwrap();
        assert_eq!(info.input, PathBuf::from("foo.cpp"));
        assert_eq!(info.output, PathBuf::from("foo.o"));
        assert!(info.flags.contains(&"-O2".to_string()));
        assert!(info.flags.contains(&"-Wall".to_string()));
    }

    #[test]
    fn test_preprocessing_not_cached() {
        let args = vec![
            "-E".to_string(),
            "foo.c".to_string(),
        ];

        assert!(parse_compiler_args("gcc", &args).is_none());
    }

    #[test]
    fn test_linking_not_cached() {
        let args = vec![
            "foo.o".to_string(),
            "bar.o".to_string(),
            "-o".to_string(),
            "myapp".to_string(),
        ];

        assert!(parse_compiler_args("gcc", &args).is_none());
    }
}
