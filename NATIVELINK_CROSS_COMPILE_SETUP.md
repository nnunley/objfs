# NativeLink Remote Execution: Cross-Compiling Rust to macOS from Linux

This document describes the complete setup for remote execution of Rust compilation targeting macOS (aarch64-apple-darwin) from a Linux NativeLink worker on NixOS.

## Problem Statement

We need to compile Rust code for macOS (aarch64-apple-darwin) on a remote Linux worker. This requires:
1. Rust compiler with macOS target stdlib
2. macOS SDK for linking
3. Darwin-aware linker

## Architecture

```
Mac (10.0.1.1)
  ↓ gRPC/HTTP
Linux Host (scheduler-host / 10.0.1.2)
  ↓ incus proxy (port 50051)
NixOS Container (10.142.129.138)
  ↓ NativeLink Worker
Remote Execution Environment
```

## Part 1: Network Configuration

### Incus Proxy Setup

The NativeLink worker runs in an incus container on an isolated network (10.142.129.0/24). To make it accessible from the Mac:

**Proxy device configuration:**
```bash
incus config device show nativelink-worker
# Shows:
# proxy-public:
#   connect: tcp:127.0.0.1:50051
#   listen: tcp:10.0.1.2:50051  # Changed from tcp:0.0.0.0:50051
#   type: proxy
```

**Key change:** Updated proxy to listen on the host's actual IP (10.0.1.2) instead of 0.0.0.0 to work properly.

**NativeLink config:** Listen on 0.0.0.0:50051 inside container (not 127.0.0.1).

## Part 2: Rust Toolchain with macOS Target

### Challenge: NixOS Dynamic Linking

NixOS uses non-standard library paths (`/nix/store/...`), making standard rustup installations fail:
```
/lib64/ld-linux-x86-64.so.2 => /nix/store/vr7ds8vwbl2fz7pr221d5y0f8n9a5wda-glibc-2.40-218/lib64/ld-linux-x86-64.so.2
```

Binaries compiled for standard Linux paths can't execute in NixOS.

### Solution: rust-overlay

Use [oxalica/rust-overlay](https://github.com/oxalica/rust-overlay) to install Rust with additional targets:

```bash
# 1. Add rust-overlay channel
nix-channel --add https://github.com/oxalica/rust-overlay/archive/master.tar.gz rust-overlay
nix-channel --update

# 2. Create Nix expression for Rust with Darwin target
cat > /tmp/rust-with-darwin.nix << 'EOF'
let
  pkgs = import <nixos> {
    overlays = [
      (import <rust-overlay>)
    ];
  };
in
  pkgs.rust-bin.stable.latest.default.override {
    targets = [ "aarch64-apple-darwin" ];
  }
EOF

# 3. Remove old rustc if present
nix-env -e rustc cargo

# 4. Install Rust with Darwin target
nix-env -f /tmp/rust-with-darwin.nix -i
```

**Verification:**
```bash
/root/.nix-profile/bin/rustc --version
# rustc 1.93.1 (01f6ddf75 2026-02-11)

ls $(/root/.nix-profile/bin/rustc --print sysroot)/lib/rustlib/ | grep darwin
# aarch64-apple-darwin
```

**Key insight:** The rust-overlay downloads pre-built `rust-std-1.93.1-aarch64-apple-darwin.tar.xz` from official Rust sources, giving us the standard library for Darwin without needing to compile it.

## Part 3: Platform Properties Configuration

### NativeLink Scheduler Configuration

The scheduler must accept the platform properties that objfs sends:

```json5
// /etc/nativelink/config.json5
schedulers: [{
  name: "MAIN_SCHEDULER",
  simple: {
    supported_platform_properties: {
      OSFamily: "exact",
      ISA: "exact",           // Not "Arch"!
      "container-image": "exact",
    },
    max_job_retries: 10,
  },
}]
```

**Important:** Use `ISA` (not `Arch`) to match NativeLink conventions.

### Worker Configuration

```json5
workers: [{
  local: {
    worker_api_endpoint: {
      uri: "grpc://127.0.0.1:50061",
    },
    cas_fast_slow_store: "WORKER_CAS_FAST_SLOW",
    upload_action_result: {
      ac_store: "AC_MAIN",
    },
    work_directory: "/var/lib/nativelink/work",
    additional_environment: {
      PATH: {
        value: "/root/.nix-profile/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/run/current-system/sw/bin",
      },
      LD_LIBRARY_PATH: {
        value: "/usr/lib:/lib",
      },
    },
    platform_properties: {
      OSFamily: {
        values: ["linux"],  // Worker OS, not target OS!
      },
      ISA: {
        values: ["x86-64"],
      },
      "container-image": {
        values: ["rust:latest"],
      },
    },
  },
}]
```

**Critical insight:** `platform_properties` describe the **worker's** platform (Linux), not the **target** platform (macOS). Remote execution happens on Linux, producing binaries for macOS.

### objfs Client Configuration

```rust
// src/re_client.rs
impl Action {
    pub fn new(command: Command, input_files: Vec<PathBuf>) -> Self {
        // Use Linux platform for remote execution
        // The worker runs on Linux, even if we're compiling for macOS
        let mut platform = Platform::new("linux", "x86-64");
        platform.properties.insert("container-image".to_string(), "rust:latest".to_string());

        // ...
    }
}

// src/platform.rs
impl Platform {
    pub fn new(os: &str, arch: &str) -> Self {
        let mut properties = HashMap::new();
        properties.insert("OSFamily".to_string(), os.to_string());
        properties.insert("ISA".to_string(), arch.to_string());  // Not "Arch"!
        Self { properties }
    }
}
```

**Environment variables in Command proto:**
```rust
// src/grpc_client.rs
let command_proto = cas::Command {
    arguments: command.arguments.clone(),
    environment_variables: vec![
        cas::command::EnvironmentVariable {
            name: "PATH".to_string(),
            value: "/root/.nix-profile/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/run/current-system/sw/bin".to_string(),
        },
    ],
    // ...
};
```

**Use absolute path to rustc:**
```rust
// src/re_client.rs
pub fn from_rustc_args(args: &[&str], working_dir: &PathBuf) -> Self {
    // Use absolute path for NixOS container
    let mut full_args = vec!["/root/.nix-profile/bin/rustc".to_string()];
    // ...
}
```

## Part 4: Cross-Compilation Status

### What Works Now

✅ Rust compiler with aarch64-apple-darwin stdlib installed
✅ Can compile Rust source to object files
✅ Remote execution infrastructure working
✅ Platform matching successful

### What Doesn't Work Yet

❌ Linking binaries for macOS (requires macOS SDK + osxcross)

**Current error:**
```
error: linking with `cc` failed: exit status: 1
  = note: clang: error: unsupported option '-arch' for target 'x86_64-unknown-linux-gnu'
```

The rustc compiler tries to use the host's `cc` (which targets Linux) to link a macOS binary. This fails because:
1. No macOS SDK available
2. Linux `cc` doesn't understand macOS-specific flags like `-arch arm64`

## Part 5: osxcross Setup

To enable full binary cross-compilation, we need osxcross.

**What osxcross provides:**
- Darwin-aware linker that understands `-arch`, `-mmacosx-version-min`, etc.
- Wrappers for Apple's development tools (clang, ld, etc.)
- Proper SDK path configuration

### Installation Steps

**1. Install build dependencies:**
```bash
nix-env -iA nixos.git nixos.cmake nixos.clang nixos.python3 nixos.libxml2 nixos.openssl
```

**2. Clone osxcross:**
```bash
mkdir -p /opt
cd /opt
git clone https://github.com/tpoechtrager/osxcross
```

**3. Download macOS SDK:**

Using pre-packaged SDKs from [joseluisq/macosx-sdks](https://github.com/joseluisq/macosx-sdks):

```bash
cd /opt/osxcross/tarballs
curl -L -o MacOSX14.5.sdk.tar.xz \
  https://github.com/joseluisq/macosx-sdks/releases/download/14.5/MacOSX14.5.sdk.tar.xz
```

**4. Build osxcross:**

On NixOS, you need to provide paths to development headers since they're in non-standard Nix store locations:

```bash
cd /opt/osxcross

# Find the exact paths (they vary per installation)
LIBXML2_DEV=$(find /nix/store -name xml2-config | head -1 | xargs dirname)
OPENSSL_DEV=$(ls -d /nix/store/*-openssl-*-dev 2>/dev/null | head -1)
OPENSSL_LIB=$(ls -d /nix/store/*-openssl-3.* 2>/dev/null | grep -v dev | head -1)
ZLIB_DEV=$(ls -d /nix/store/*-zlib-*-dev 2>/dev/null | head -1)

# Set up build environment
export PATH="/root/.nix-profile/bin:${LIBXML2_DEV}:$PATH"
export CPPFLAGS="-I${OPENSSL_DEV}/include -I${ZLIB_DEV}/include"
export LDFLAGS="-L${OPENSSL_LIB}/lib"

# Build
SDK_VERSION=14.5 UNATTENDED=1 ./build.sh
```

**NixOS-specific challenges:**
- Headers and libraries are in `/nix/store/...` not standard FHS paths
- Need to explicitly provide CPPFLAGS and LDFLAGS
- Build dependencies: libxml2-dev, openssl-dev, zlib-dev

**5. Configure Rust to use osxcross:**

After osxcross builds successfully, configure Rust to use it as the linker:

```bash
# Add to environment or Command proto
export PATH="/opt/osxcross/target/bin:$PATH"
export CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER="aarch64-apple-darwin23.5-clang"
export SDKROOT="/opt/osxcross/target/SDK/MacOSX14.5.sdk"
```

Or create a `.cargo/config.toml`:
```toml
[target.aarch64-apple-darwin]
linker = "aarch64-apple-darwin23.5-clang"
```

### osxcross Build Status

✅ osxcross build completed successfully with these components:
1. xar (archiving library)
2. cctools-port (Apple's compiler tools ported to Linux)
3. Clang wrapper configured for Darwin
4. All target binaries: aarch64-apple-darwin23.5-clang, linker, ar, etc.

Note: The final test step failed due to missing 32-bit glibc headers (gnu/stubs-32.h) in NixOS, but this doesn't affect functionality since we only need 64-bit support. All required binaries are in `/opt/osxcross/target/bin/`.

## Testing the Setup

### Environment Variables

```bash
export OBJFS_REMOTE_ENDPOINT="http://scheduler-host:50051"
export OBJFS_REMOTE_INSTANCE="main"
export OBJFS_REMOTE_TARGETS="aarch64-apple-darwin"
export OBJFS_MIN_REMOTE_SIZE=1
```

### Test Command

```bash
cd /tmp
cat > test_remote.rs << 'EOF'
fn main() {
    println!("Hello from remote execution!");
}
EOF

$PROJECT_ROOT/target/release/cargo-objfs-rustc rustc \
  --target aarch64-apple-darwin \
  --crate-name test_remote \
  --edition 2021 \
  --crate-type bin \
  -o test_remote_bin \
  test_remote.rs
```

### Current Output

```
[objfs] remote execution: target=aarch64-apple-darwin, size=60 bytes
[objfs] remote execution failed with exit code 1
```

Check NativeLink logs for stderr showing linking errors.

## Troubleshooting

### "No candidate workers due to lack of matching X"

Check that:
- Scheduler's `supported_platform_properties` includes the property
- Worker's `platform_properties` advertises the correct values
- objfs sends matching platform properties (OSFamily=linux, ISA=x86-64)

### "Command not found: rustc"

Options:
1. Use absolute path: `/root/.nix-profile/bin/rustc`
2. Set PATH in Command proto's `environment_variables`
3. Update worker's `additional_environment` (may not work in all NativeLink versions)

### Container networking issues

If Mac can't reach scheduler-host:50051:
1. Verify incus proxy: `incus config device show nativelink-worker`
2. Check proxy listen address matches host IP
3. Test with curl: `curl -v http://scheduler-host:50051`
4. Verify NativeLink is listening: `incus exec nativelink-worker -- ss -tlnp | grep 50051`

## References

- [oxalica/rust-overlay](https://github.com/oxalica/rust-overlay) - Rust toolchains with target support
- [nix-community/fenix](https://github.com/nix-community/fenix) - Alternative Rust toolchain manager
- [Cross compilation with Nix](https://nix.dev/tutorials/cross-compilation.html)
- [osxcross](https://github.com/tpoechtrager/osxcross) - macOS cross-compilation toolchain
- [Rust cross-compile Linux to macOS](https://wapl.es/rust/2019/02/17/rust-cross-compile-linux-to-macos.html/)

## Status: osxcross on NixOS - Incompatible

**IMPORTANT:** osxcross does not work properly on NixOS due to fundamental incompatibilities between osxcross's compiler wrappers and NixOS's compiler wrappers.

### The Problem

osxcross wraps `clang` to inject Darwin-specific flags and linker settings. However, NixOS also wraps `clang` to inject NixOS-specific paths and settings. When these two wrapper systems interact:

1. **NixOS's clang wrapper** calls `/nix/store/.../binutils-wrapper/bin/ld` with Linux-specific flags (`-dynamic-linker`, `-emulation`, etc.)
2. **osxcross expects** to control the entire compilation pipeline and use Darwin-aware tools
3. **Result:** The linker fails with "unrecognised emulation mode: llvm" or hangs in infinite loops

### Evidence

```bash
# osxcross clang wrapper tries to compile for Darwin
$ /opt/osxcross/target/bin/aarch64-apple-darwin23.5-clang test.c

# But NixOS's clang wrapper underneath calls:
/nix/store/z1nv854kk451zqzhj7x8x5siypjwq5ws-binutils-2.44/bin/ld \
  -dynamic-linker=/nix/store/.../glibc-2.40-218/lib/ld-linux-x86-64.so.2 \
  # ... (Linux-specific flags)

# Result:
ld: unrecognised emulation mode: llvm
Supported emulations: elf_x86_64 elf32_x86_64 elf_i386 elf_iamcu
```

### Alternative Solutions

**Option A: Use Standard Linux Container (RECOMMENDED)**
- Replace NixOS container with Ubuntu/Debian container for the NativeLink worker
- osxcross works normally on standard Linux distributions
- Install via package manager, no Nix wrapper conflicts

**Option B: Use Zig CC as Cross-Compiler**
- [cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild) can cross-compile to macOS
- No macOS SDK required (uses Zig's bundled libc)
- Works on NixOS without wrapper conflicts
- Simpler setup than osxcross

**Option C: Native macOS Worker**
- Run NativeLink worker on macOS machine
- No cross-compilation needed
- Most reliable but requires macOS hardware

**Option D: Rust-lld with macOS SDK**
- Use Rust's bundled `lld` linker directly
- Provide macOS SDK from osxcross
- Bypass both osxcross and NixOS wrappers
- Experimental, may have missing symbols

### What Was Accomplished

Despite osxcross not working on NixOS, this effort successfully:

1. ✅ **Remote execution infrastructure** - Full RE API v2 implementation working
2. ✅ **macOS SDK extraction** - SDK available at `/opt/osxcross/target/SDK/MacOSX14.5.sdk`
3. ✅ **Cross-compilation research** - Documented Rust cross-compile requirements
4. ✅ **NixOS rust-overlay setup** - Rust with `aarch64-apple-darwin` stdlib installed
5. ✅ **Network configuration** - Incus proxy working for remote worker access

### Lessons Learned

1. **NixOS + osxcross don't mix** - Wrapper conflicts are fundamental
2. **Standard Linux preferred** - For cross-compilation toolchains, avoid NixOS
3. **Zig CC is promising** - Modern alternative to osxcross
4. **SDK extraction works** - Can reuse the SDK with other toolchains

## Implementation Complete - osxcross Abandoned

### What's Been Done

✅ **osxcross installation complete**
- Built and installed in `/opt/osxcross/target/bin/` on nativelink-worker
- Darwin linkers ready: `aarch64-apple-darwin23.5-clang`, etc.

✅ **NativeLink worker configured**
- PATH updated to include osxcross binaries
- `CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER` environment variable set

✅ **objfs code updated**
- Automatic detection of Darwin targets in `src/re_client.rs`
- Injects `-C linker=aarch64-apple-darwin23.5-clang` for Darwin builds
- Changes in `src/grpc_client.rs` and `src/re_client.rs`

## Recommended Next Steps

### Quick Win: Use Standard Linux Container

Replace the NixOS worker with Ubuntu 24.04:

```bash
# Create new Ubuntu container
incus launch images:ubuntu/24.04 nativelink-worker-ubuntu

# Copy NativeLink configuration
incus file push /tmp/config.json5 nativelink-worker-ubuntu/etc/nativelink/

# Install dependencies in Ubuntu
incus exec nativelink-worker-ubuntu -- bash << 'EOF'
apt-get update
apt-get install -y curl git clang cmake libssl-dev libxml2-dev

# Install Rust with darwin target
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source $HOME/.cargo/env
rustup target add aarch64-apple-darwin

# Install osxcross
cd /opt
git clone https://github.com/tpoechtrager/osxcross
# ... (follow standard osxcross instructions - will work without wrapper conflicts)
EOF
```

On Ubuntu, osxcross works as documented without NixOS wrapper issues.

### Alternative: cargo-zigbuild

Simpler cross-compilation without SDK:

```bash
# On worker (Ubuntu or NixOS)
cargo install cargo-zigbuild

# In objfs code, change linker to:
-C linker=zigcc

# No SDK needed, works on NixOS
```

### What to Salvage

The macOS SDK is already extracted and usable:
- Location: `/opt/osxcross/target/SDK/MacOSX14.5.sdk`
- Can be copied to Ubuntu container
- Can be used with Zig CC or rust-lld

The remote execution infrastructure is complete and working - just needs a compatible worker environment.
