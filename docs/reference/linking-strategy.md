# Linking Strategy

## Overview

objfs uses a hybrid approach: compile `.rlib` files on any worker, link final binaries on platform-compatible workers. Compilation units distribute across the entire cluster for maximum parallelism, while link operations route only to workers whose OS family matches the target platform. When no compatible remote worker is available, linking falls back to the local machine's native toolchain.

## Compilation vs Linking

### Can Execute Remotely

- **Library compilation**: `.rs` -> `.rlib`
- **Object file generation**: `.rs` -> `.o`
- **Metadata generation**: `.rs` -> `.rmeta`
- **Proc macro compilation**: `.rs` -> `.so`

These operations only need `rustc` and `rust-std` for the target. They do not require a platform linker and can run on any OS with the matching target stdlib.

### Needs Platform Matching

- **Binary linking**: `.rlib` -> executable
- **Dynamic library linking**: `.rlib` -> `.so`/`.dylib`
- **Static library archiving**: `.o` -> `.a`

These operations require a platform-specific linker (`ld`, `link.exe`) and may need platform SDKs. They can run remotely if a worker's platform matches the target, and must run locally if no compatible remote workers are available.

### Build Process

```
cargo build --target aarch64-apple-darwin
    |
Generates multiple rustc invocations:
    |-- rustc lib1.rs -o lib1.rlib        [Compilation]
    |-- rustc lib2.rs -o lib2.rlib        [Compilation]
    |-- rustc lib3.rs -o lib3.rlib        [Compilation]
    +-- rustc main.rs -o myapp            [Link]
         (links: lib1.rlib + lib2.rlib + lib3.rlib)
```

## Platform Matching Rules

objfs uses the target OS family for link compatibility:

| Target Triple | Compatible Workers | Incompatible Workers |
|---------------|-------------------|---------------------|
| `aarch64-apple-darwin` | darwin/aarch64, darwin/x86_64 | linux/\*, windows/\* |
| `x86_64-unknown-linux-gnu` | linux/x86_64, linux/aarch64 | darwin/\*, windows/\* |
| `x86_64-pc-windows-msvc` | windows/x86_64 | darwin/\*, linux/\* |

**Key insight**: Architecture can differ (x86_64 vs aarch64), but OS must match (darwin, linux, windows).

Any worker with `rust-std` for the target can compile `.rlib` files regardless of its own platform. But only workers matching the target OS family can link, because linking requires the platform-specific linker and SDKs:

```
Building aarch64-apple-darwin .rlib:
    Linux worker with aarch64-apple-darwin rust-std   -> OK
    Darwin worker (native or with rust-std)           -> OK
    Windows worker with aarch64-apple-darwin rust-std  -> OK

Linking aarch64-apple-darwin binary:
    Darwin/aarch64 worker (native toolchain)          -> OK
    Darwin/x86_64 worker (cross-platform within OS)   -> OK
    Linux worker (no darwin linker)                    -> INCOMPATIBLE
    Windows worker (no darwin linker)                  -> INCOMPATIBLE
```

## Decision Flow

```
Link operation detected for target X:
    |
    +-- Check OBJFS_REMOTE_TARGETS
    |   (does scheduler have X-compatible workers?)
    |   |
    |   +-- YES -> Try remote execution
    |   |          +-- Scheduler assigns to compatible worker
    |   |          +-- If fails -> fall back to local
    |   |
    |   +-- NO -> Execute locally immediately
    |              +-- Use native toolchain
```

### Detection and Routing Logic

objfs detects link operations and routes them appropriately:

```rust
// 1. Detect if operation is a link
fn is_link_operation(build_info: &BuildInfo) -> bool {
    // All inputs are pre-compiled (`.rlib`, `.a`, etc.)
    let all_precompiled = build_info.input_files.iter().all(|f| {
        matches!(f.extension(), "rlib" | "rmeta" | "a" | "so" | "dylib")
    });

    // Output is a binary (not a library)
    let output_is_binary = !matches!(
        build_info.output_file.extension(),
        "rlib" | "rmeta" | "a"
    );

    all_precompiled && output_is_binary
}

// 2. Route link operations based on worker availability
if is_link_operation(&build_info) {
    if remote_config.can_build_target(target_triple) {
        // Platform-compatible worker available - try remote
        try_remote_execution(&remote_config, &build_info, &args)
            .or_else(|_| execute_locally(&build_info, &args))
    } else {
        // No compatible workers - must execute locally
        execute_locally(&build_info, &args)
    }
}
```

## Examples

### Mac Developer with Mixed Workers

```bash
# Setup
export OBJFS_REMOTE_ENDPOINT="http://build-cluster:50051"
# Workers: 2 darwin, 3 linux

# Build for macOS
cargo build --target aarch64-apple-darwin

# Compilation (.rlib):
[objfs] remote execution: libfoo.rlib (worker: linux/x86-64)
[objfs] remote execution: libbar.rlib (worker: darwin/aarch64)

# Link (needs darwin):
[objfs] link operation for aarch64-apple-darwin - trying platform-compatible remote worker
[objfs] remote link succeeded (worker: darwin/aarch64)
```

### Mac Developer with Only Linux Workers

```bash
# Setup
export OBJFS_REMOTE_ENDPOINT="http://build-cluster:50051"
# Workers: 5 linux, 0 darwin

# Build for macOS
cargo build --target aarch64-apple-darwin

# Compilation (.rlib):
[objfs] remote execution: libfoo.rlib (worker: linux/x86-64)
[objfs] remote execution: libbar.rlib (worker: linux/x86-64)

# Link (needs darwin, none available):
[objfs] link operation for aarch64-apple-darwin - no compatible remote workers, executing locally
    Linking myapp v0.1.0
```

### Cross-Platform Team

**Team Setup:**
- Alice (Mac M1) - Developer
- Bob (Linux x86_64) - Developer
- CI (Linux x86_64) - Automation

**Shared scheduler:** `build-cluster:50051`

**Alice builds for Mac:**

```bash
cargo build --target aarch64-apple-darwin

# Compiles .rlib:
#   - Worker: Bob's Linux machine
#   - Worker: CI server
# Links binary:
#   - Local: Alice's Mac with Xcode
```

**Bob builds for Linux:**

```bash
cargo build --target x86_64-unknown-linux-gnu

# Compiles .rlib:
#   - Worker: Alice's Mac
#   - Worker: CI server
# Links binary:
#   - Local: Bob's Linux with ld
```

**CI builds for both:**

```bash
cargo build --target aarch64-apple-darwin
cargo build --target x86_64-unknown-linux-gnu

# Compiles .rlib:
#   - Worker: Alice's Mac
#   - Worker: Bob's Linux
# Links binaries:
#   - aarch64: FAILS (CI is Linux, can't link macOS)
#   - x86_64: Local
```

**Solution for CI**: Install target linker or use a Mac worker for Darwin builds.

## Configuration

### Automatic (Recommended)

```bash
# Just set scheduler endpoint
export OBJFS_REMOTE_ENDPOINT="http://build-cluster:50051"

# objfs auto-detects:
# - Local platform (darwin, linux, windows)
# - Registers as worker with correct properties
# - Assumes workers can build for their own platform
```

### Explicit (Advanced)

```bash
# Tell objfs which platforms remote workers support
export OBJFS_REMOTE_TARGETS="aarch64-apple-darwin,x86_64-unknown-linux-gnu"

# Now objfs knows:
# - Can link darwin binaries remotely
# - Can link linux binaries remotely
# - Must link windows binaries locally (not in list)
```

### Force Everything Local

```bash
# Disable remote execution entirely
OBJFS_DISABLE=1 cargo build
```

## Performance

### Cold Build (No Cache)

```
Sequential (local only):
    lib1: 5s ------>
    lib2:      5s ------>
    lib3:           5s ------>
    link:                1s ->
    Total: 16s

Parallel (distributed compilation):
    lib1: 5s ------>   (Worker A)
    lib2: 5s ------>   (Worker B)
    lib3: 5s ------>   (Worker C)
    link:      1s ->   (Local)
    Total: 6s (2.7x faster)
```

### Warm Cache

```
    lib1: cache hit (100ms)
    lib2: cache hit (100ms)
    lib3: cache hit (100ms)
    link: cache hit (100ms)
    Total: 400ms (40x faster than cold)
```

## Troubleshooting

### "No compatible remote workers" for Every Link

**Problem**: All links execute locally even though remote workers exist.

**Cause**: `OBJFS_REMOTE_TARGETS` doesn't include your target platform.

**Solution**:
```bash
# Check current config
echo $OBJFS_REMOTE_TARGETS

# Add your target
export OBJFS_REMOTE_TARGETS="aarch64-apple-darwin,x86_64-unknown-linux-gnu"

# Or let auto-detection work
unset OBJFS_REMOTE_TARGETS
```

### Links Fail with "Linker Not Found"

**Problem**: Remote worker tries to link but doesn't have toolchain.

**Cause**: Worker platform doesn't match target platform.

**Solution**: Verify worker platform properties match target OS family.

```bash
# Worker should announce correct OSFamily
# darwin workers -> OSFamily: "darwin"
# linux workers -> OSFamily: "linux"
```

### Slower Link Performance Than Expected

**Problem**: Remote linking slower than local.

**Analysis**: Network transfer overhead + remote execution time > local link time.

**Solution**: For small links, local is faster. For large projects with many `.rlib` files, remote can be faster due to:
- Faster worker hardware
- Cached `.rlib` files on remote CAS
- Parallel link on multiple workers (multiple binaries)
