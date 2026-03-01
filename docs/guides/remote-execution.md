# Remote Execution with NativeLink

This document explains how to configure objfs to use remote execution with your x86_64-linux NativeLink instance while developing on aarch64-macos.

## Architecture

```
Your Mac (aarch64-macos)
    |
objfs detects: Need x86_64-linux binary
    |
Decision: Can I build this locally?
    - If --target x86_64-unknown-linux-gnu: NO -> Use remote
    - If native build: YES -> Use local
    |
NativeLink Workers (x86_64-linux @ scheduler-host)
    |
Compiled binary stored in CAS
    |
Downloaded to your Mac
```

## Configuration

### 1. Enable Remote Execution

Add to your shell profile (`~/.zshrc` or `~/.bashrc`):

```bash
# NativeLink remote execution
export OBJFS_REMOTE_ENDPOINT="http://scheduler-host:50051"
export OBJFS_REMOTE_PLATFORM="x86_64-unknown-linux-gnu"
export OBJFS_REMOTE_INSTANCE="main"

# Optional: Minimum size to use remote (default: 100KB)
export OBJFS_MIN_REMOTE_SIZE=102400
```

###  2. Cross-Compile for Linux

```bash
# Add Linux target if you don't have it
rustup target add x86_64-unknown-linux-gnu

# Build for Linux (will use remote execution)
cargo build --target x86_64-unknown-linux-gnu
```

## How It Works

### Platform Detection

```rust
// objfs automatically detects:
Host: aarch64-macos (your Mac)
Target: x86_64-linux (from --target flag)

// Decision:
if target != host && remote_platform == target:
    use_remote_execution()
else:
    compile_locally()
```

### Cache Keys Include Platform

```
macOS build:
  cache_key = SHA256("Arch=aarch64,OSFamily=macos" + sources)

Linux build:
  cache_key = SHA256("Arch=x86_64,OSFamily=linux" + sources)

Different keys -> No collision -> Correct binary for each platform
```

## Example Workflow

### Scenario: Build a Linux binary from your Mac

```bash
# 1. Configure remote execution
export OBJFS_REMOTE_ENDPOINT="http://scheduler-host:50051"
export OBJFS_REMOTE_PLATFORM="x86_64-unknown-linux-gnu"

# 2. Build for Linux
cd my-rust-project
cargo build --target x86_64-unknown-linux-gnu

# What happens:
# - objfs sees: target=x86_64-linux, remote=x86_64-linux -> MATCH
# - Checks local cache (miss)
# - Checks remote NativeLink cache (miss)
# - Submits build to NativeLink workers
# - Workers compile on x86_64-linux
# - Binary stored in remote CAS
# - Binary downloaded to your Mac
# - Cached locally for next time
```

### Scenario: Build a macOS binary (native)

```bash
# Normal cargo build (no --target)
cargo build

# What happens:
# - objfs sees: target=aarch64-macos, remote=x86_64-linux -> MISMATCH
# - Compiles locally on your Mac
# - Stores in local CAS
```

## Platform Matching Rules

| Build Target | Remote Platform | Decision | Reason |
|--------------|-----------------|----------|---------|
| x86_64-linux | x86_64-linux | **Remote** | Exact match |
| aarch64-macos | x86_64-linux | **Local** | Platform mismatch |
| aarch64-linux | x86_64-linux | **Local** | Arch mismatch |
| x86_64-linux-musl | x86_64-linux-gnu | **Local** | Different ABI |

## Hierarchical CAS Example

With remote execution enabled, your CAS hierarchy becomes:

```rust
HybridCas {
    backends: [
        Local(~/.cache/objfs/cas),              // Check local first
        Remote(http://scheduler-host:50051/main),    // Then remote NativeLink
    ]
}
```

**Benefits:**
- Local cache is fast (no network)
- Remote cache is shared (team members can reuse)
- Remote execution offloads work to powerful Linux box

## Troubleshooting

### Remote execution not working?

```bash
# Check configuration
echo $OBJFS_REMOTE_ENDPOINT
echo $OBJFS_REMOTE_PLATFORM

# Test connectivity
curl -I http://scheduler-host:50051

# Verify NativeLink instance name
# (should match OBJFS_REMOTE_INSTANCE)
```

### Getting wrong binary architecture?

```bash
# Check cache keys include platform
objfs stats

# Clear cache if needed
objfs clear

# Rebuild
cargo clean && cargo build --target x86_64-unknown-linux-gnu
```

### Build not using remote?

Common reasons:
1. **Target doesn't match**: `--target aarch64-apple-darwin` won't use Linux workers
2. **File too small**: Default threshold is 100KB
3. **Remote not configured**: Check environment variables
4. **NativeLink unreachable**: Check `http://scheduler-host:50051`
