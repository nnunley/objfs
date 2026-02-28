# Distributed Compilation with Platform-Compatible Linking

## Strategy

objfs uses a **hybrid approach** to maximize the benefits of distributed builds while ensuring platform-correct binaries:

1. **Compile `.rlib` files remotely** - Can run on any worker with the target's rust-std
2. **Link final binaries on platform-compatible workers** - Uses platform-specific linker

This gives you:
✅ Parallelized compilation across the cluster
✅ Platform-correct binaries without cross-compilation
✅ Links can run remotely if workers match target platform
✅ Automatic fallback to local linking when needed

## How It Works

### Build Process

```
cargo build --target aarch64-apple-darwin
    ↓
Generates multiple rustc invocations:
    ├─ rustc lib1.rs -o lib1.rlib        [Compilation]
    ├─ rustc lib2.rs -o lib2.rlib        [Compilation]
    ├─ rustc lib3.rs -o lib3.rlib        [Compilation]
    └─ rustc main.rs -o myapp            [Link]
         (links: lib1.rlib + lib2.rlib + lib3.rlib)
```

### objfs Decision Tree

```
For each rustc invocation:
    │
    ├─ Is this a link operation?
    │  (inputs are all .rlib/.a files)
    │  │
    │  ├─ YES → Check platform compatibility
    │  │        │
    │  │        ├─ Platform-compatible worker available?
    │  │        │  (darwin→darwin, linux→linux)
    │  │        │  │
    │  │        │  ├─ YES → Try remote execution on compatible worker
    │  │        │  │
    │  │        │  └─ NO → Execute LOCALLY
    │  │        │         (uses your Mac's ld/Xcode)
    │  │
    │  └─ NO → Compilation unit (can use any remote worker)
    │           ├─ Check cache
    │           ├─ Try remote worker
    │           └─ Fallback to local
```

### Example Session

```bash
$ cargo build --release --target aarch64-apple-darwin
   Compiling myproject v0.1.0

# Compilation units (can run on any worker with rust-std):
[objfs] remote execution: libfoo.rlib (worker: linux/x86-64)
[objfs] remote execution: libbar.rlib (worker: darwin/aarch64)
[objfs] remote execution: libmain.rlib (worker: linux/x86-64)

# Link operation (needs darwin worker or local):
[objfs] link operation for aarch64-apple-darwin - trying platform-compatible remote worker
[objfs] remote link succeeded (worker: darwin/aarch64)
   Linking myproject v0.1.0
```

### Example Session (No Compatible Workers)

```bash
$ cargo build --release --target aarch64-apple-darwin
   # (On a Mac with only Linux workers available)

[objfs] remote execution: libfoo.rlib (worker: linux/x86-64)
[objfs] remote execution: libbar.rlib (worker: linux/x86-64)
[objfs] remote execution: libmain.rlib (worker: linux/x86-64)

# Link operation (no darwin workers, must use local):
[objfs] link operation for aarch64-apple-darwin - no compatible remote workers, executing locally
   Linking myproject v0.1.0
```

## Benefits

### 1. No Cross-Compilation Issues

**Problem we avoid:**
```
Linux worker tries to link Darwin binary:
    rustc main.rs -o myapp --target aarch64-apple-darwin
    ↓
    error: linking with `cc` failed
    = note: linker not found for aarch64-apple-darwin
```

**Our solution:**
```
Linux worker compiles libraries:
    rustc lib.rs -o lib.rlib --target aarch64-apple-darwin ✅
    (Just needs rust-std for target, no linker)

Mac links locally:
    rustc main.rs -L deps -o myapp --target aarch64-apple-darwin ✅
    (Uses native ld/Xcode)
```

### 2. Parallelized Compilation

Multiple `.rlib` files can compile on different workers simultaneously:

```
Time: 0s ────────────> 10s

Worker A: ████████░░ libfoo.rlib
Worker B: ██████████ libbar.rlib
Worker C: ████░░░░░░ libutils.rlib
Local:    ░░░░░░░░██ link (fast)

Total: 10s (vs 30s sequential)
```

### 3. Correct Platform Binaries

Each platform links with its native toolchain:

```
Developer on Mac:
    • .rlib files: from remote Linux workers ✅
    • Final binary: linked on Mac with Xcode ✅
    • Result: Mach-O aarch64 binary ✅

Developer on Linux:
    • .rlib files: from remote Mac workers ✅
    • Final binary: linked on Linux with ld ✅
    • Result: ELF x86_64 binary ✅
```

## What Gets Distributed

### Can Execute Remotely ✅

- **Library compilation**: `.rs` → `.rlib`
- **Object file generation**: `.rs` → `.o`
- **Metadata generation**: `.rs` → `.rmeta`
- **Proc macro compilation**: `.rs` → `.so`

These operations:
- Only need `rustc` and `rust-std` for target
- Don't require platform linker
- Can run on any OS with matching target stdlib

### May Execute Locally (Platform-Dependent) ⚠️

- **Binary linking**: `.rlib` → executable
- **Dynamic library linking**: `.rlib` → `.so`/`.dylib`
- **Static library archiving**: `.o` → `.a`

These operations:
- Require platform-specific linker (`ld`, `link.exe`)
- May need platform SDKs
- **Can run remotely if worker platform matches target platform**
- **Must run locally if no compatible remote workers available**

Examples:
- Building `aarch64-apple-darwin` binary → Can run on any darwin worker (local or remote)
- Building `x86_64-unknown-linux-gnu` binary → Can run on any linux worker (local or remote)
- Building `aarch64-apple-darwin` on Mac with only Linux workers → Must run locally

## Detection and Routing Logic

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

## Configuration

### Default Behavior (Automatic)

```bash
# Just works - no config needed
cargo build

# Compilations: remote (if available)
# Links: always local
```

### Force Everything Local

```bash
# Disable remote execution entirely
OBJFS_DISABLE=1 cargo build
```

### Force Everything Remote (Not Recommended)

```bash
# Override link detection (may fail on cross-platform)
OBJFS_FORCE_REMOTE_LINK=1 cargo build
```

## Performance Characteristics

### Cold Build (No Cache)

```
Sequential (local only):
    lib1: 5s ──────>
    lib2:      5s ──────>
    lib3:           5s ──────>
    link:                1s ─>
    Total: 16s

Parallel (distributed compilation):
    lib1: 5s ──────>   (Worker A)
    lib2: 5s ──────>   (Worker B)
    lib3: 5s ──────>   (Worker C)
    link:      1s ─>   (Local)
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

## Compatibility

### Supported Scenarios ✅

| Developer Platform | Target Platform | Workers Needed | Works? |
|--------------------|-----------------|----------------|--------|
| Mac | aarch64-apple-darwin | Any OS | ✅ |
| Mac | x86_64-unknown-linux-gnu | Linux workers | ✅ |
| Linux | x86_64-unknown-linux-gnu | Any OS | ✅ |
| Linux | aarch64-apple-darwin | Any OS (with rust-std) | ✅ |
| Windows | x86_64-pc-windows-msvc | Any OS (with rust-std) | ✅ |

**Key insight**: Workers just need `rust-std` for the target platform. Linking happens locally with proper toolchain.

### Unsupported (Would Require Remote Linking) ❌

- Cross-compiling on machine without target stdlib
- Distributed linking (not just compilation)

## Example: Cross-Platform Team

**Team Setup:**
- Alice (Mac M1) - Developer
- Bob (Linux x86_64) - Developer
- CI (Linux x86_64) - Automation

**Shared scheduler:** `build-cluster:50051`

### Alice builds for Mac

```bash
cargo build --target aarch64-apple-darwin

# Compiles .rlib:
#   - Worker: Bob's Linux machine ✅
#   - Worker: CI server ✅
# Links binary:
#   - Local: Alice's Mac with Xcode ✅
```

### Bob builds for Linux

```bash
cargo build --target x86_64-unknown-linux-gnu

# Compiles .rlib:
#   - Worker: Alice's Mac ✅
#   - Worker: CI server ✅
# Links binary:
#   - Local: Bob's Linux with ld ✅
```

### CI builds for both

```bash
cargo build --target aarch64-apple-darwin
cargo build --target x86_64-unknown-linux-gnu

# Compiles .rlib:
#   - Worker: Alice's Mac ✅
#   - Worker: Bob's Linux ✅
# Links binaries:
#   - aarch64: FAILS (CI is Linux, can't link macOS) ❌
#   - x86_64: Local ✅
```

**Solution for CI**: Install target linker or use a Mac worker for Darwin builds.

## Monitoring

Check what's being distributed:

```bash
cargo build 2>&1 | grep objfs

# Output:
# [objfs] remote execution: libfoo.rlib (worker: linux/x86-64)
# [objfs] remote execution: libbar.rlib (worker: darwin/aarch64)
# [objfs] link operation detected - executing locally
# [objfs] cached bundle: 1 files -> a3b5c7d9
```

## Future Enhancements

1. **Parallel linking** - Multiple binaries link simultaneously
2. **LTO (Link-Time Optimization)** - Smart distribution of LTO units
3. **Incremental linking** - Cache link graph, relink only changed
4. **Remote linking with SDK** - Distribute linker + SDK together

## Summary

**Current architecture:**
- ✅ Compile libraries anywhere (distributed)
- ✅ Link binaries on native platform (local)
- ✅ No cross-compilation toolchain complexity
- ✅ Parallelized builds across cluster
- ✅ Platform-correct binaries

This gives you 80% of the benefit of full distributed builds with 20% of the complexity.
