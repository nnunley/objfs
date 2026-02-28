# Platform-Compatible Linking Strategy

## Overview

objfs intelligently routes link operations to workers based on platform compatibility, maximizing distributed execution while ensuring correct binaries.

## How It Works

### Compilation Phase

**Any worker with rust-std for the target can compile:**

```
Building aarch64-apple-darwin .rlib:
    ✅ Linux worker with aarch64-apple-darwin rust-std
    ✅ Darwin worker (native or with rust-std)
    ✅ Windows worker with aarch64-apple-darwin rust-std

    Worker platform doesn't matter - only needs:
    • rustc
    • rust-std for target
```

### Link Phase

**Only workers matching the target platform can link:**

```
Linking aarch64-apple-darwin binary:
    ✅ Darwin/aarch64 worker (native toolchain)
    ✅ Darwin/x86_64 worker (cross-platform within OS)
    ❌ Linux worker (no darwin linker)
    ❌ Windows worker (no darwin linker)

    Requires:
    • Platform-specific linker (ld, link.exe)
    • Platform SDKs (Xcode, Windows SDK)
    • Matching OS family
```

## Platform Matching Rules

objfs uses **target OS family** for link compatibility:

| Target Triple | Compatible Workers | Incompatible Workers |
|---------------|-------------------|---------------------|
| `aarch64-apple-darwin` | darwin/aarch64, darwin/x86_64 | linux/*, windows/* |
| `x86_64-unknown-linux-gnu` | linux/x86_64, linux/aarch64 | darwin/*, windows/* |
| `x86_64-pc-windows-msvc` | windows/x86_64 | darwin/*, linux/* |

**Key insight**: Architecture can differ (x86_64 vs aarch64), but OS must match (darwin, linux, windows).

## Decision Flow

```
Link operation detected for target X:
    │
    ├─ Check OBJFS_REMOTE_TARGETS
    │  (does scheduler have X-compatible workers?)
    │  │
    │  ├─ YES → Try remote execution
    │  │        └─ Scheduler assigns to compatible worker
    │  │        └─ If fails → fall back to local
    │  │
    │  └─ NO → Execute locally immediately
    │           └─ Use native toolchain
```

## Configuration

### Automatic (Recommended)

```bash
# Just set scheduler endpoint
export OBJFS_REMOTE_ENDPOINT="http://build-cluster:50051"

# objfs auto-detects:
# • Local platform (darwin, linux, windows)
# • Registers as worker with correct properties
# • Assumes workers can build for their own platform
```

### Explicit (Advanced)

```bash
# Tell objfs which platforms remote workers support
export OBJFS_REMOTE_TARGETS="aarch64-apple-darwin,x86_64-unknown-linux-gnu"

# Now objfs knows:
# • Can link darwin binaries remotely
# • Can link linux binaries remotely
# • Must link windows binaries locally (not in list)
```

## Examples

### Scenario 1: Mac Developer with Mixed Workers

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

### Scenario 2: Mac Developer with Only Linux Workers

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

### Scenario 3: Linux CI Building for Both Platforms

```bash
# Setup on Linux CI
export OBJFS_REMOTE_ENDPOINT="http://build-cluster:50051"
# Workers: 2 darwin, 3 linux

# Build for Linux (native)
cargo build --target x86_64-unknown-linux-gnu

# Compilation:
[objfs] remote execution: libfoo.rlib (worker: linux/x86-64)

# Link (needs linux, available):
[objfs] link operation for x86_64-unknown-linux-gnu - trying platform-compatible remote worker
[objfs] remote link succeeded (worker: linux/x86-64)

# Build for macOS (cross-platform)
cargo build --target aarch64-apple-darwin

# Compilation:
[objfs] remote execution: libfoo.rlib (worker: linux/x86-64)

# Link (needs darwin, available):
[objfs] link operation for aarch64-apple-darwin - trying platform-compatible remote worker
[objfs] remote link succeeded (worker: darwin/aarch64)
```

## Benefits

### ✅ Maximized Parallelism

Links can run remotely when compatible workers exist, not forced local.

### ✅ Automatic Fallback

No compatible workers? Seamlessly falls back to local linking.

### ✅ Platform-Correct Binaries

Platform matching ensures binaries get correct linker and SDKs.

### ✅ Simple Configuration

Auto-detection works for common setups, explicit config for advanced.

## Platform Properties

NativeLink workers announce their platform via properties:

```json
{
  "OSFamily": "darwin",    // or "linux", "windows"
  "ISA": "aarch64",       // or "x86_64"
  "container-image": "rust:latest"
}
```

Scheduler matches these to build requirements:
- darwin binaries → darwin workers (any ISA)
- linux binaries → linux workers (any ISA)
- windows binaries → windows workers (any ISA)

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
# darwin workers → OSFamily: "darwin"
# linux workers → OSFamily: "linux"
```

### Slower Link Performance Than Expected

**Problem**: Remote linking slower than local.

**Analysis**: Network transfer overhead + remote execution time > local link time.

**Solution**: For small links, local is faster. For large projects with many .rlib files, remote can be faster due to:
- Faster worker hardware
- Cached .rlib files on remote CAS
- Parallel link on multiple workers (multiple binaries)

## Future Enhancements

1. **Smart worker selection** - Prefer workers with hot .rlib cache
2. **Link time estimation** - Choose remote vs local based on predicted time
3. **Parallel linking** - Link multiple binaries concurrently on different workers
4. **Platform SDK caching** - Cache platform SDKs in CAS for worker setup
