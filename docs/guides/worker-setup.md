# NativeLink Worker Setup

## Overview

NativeLink workers execute build tasks dispatched by the scheduler. This guide covers setting up workers on macOS, configuring multi-worker architectures, running workers in Docker containers, and cross-compiling Rust to macOS from Linux.

## macOS Worker

Run a NativeLink worker on your Mac to handle Darwin compilation natively, avoiding cross-compilation complexity.

### Installation

**Option A: Homebrew (if available)**
```bash
brew install nativelink
```

**Option B: Build from source**
```bash
# Install Rust if not already installed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build NativeLink
git clone https://github.com/TraceMachina/nativelink.git
cd nativelink
cargo build --release

# Copy binary to PATH
sudo cp target/release/nativelink /usr/local/bin/
```

### Worker Configuration

```bash
sudo mkdir -p /usr/local/etc/nativelink
```

Create `/usr/local/etc/nativelink/config.json5`:

```json5
{
  stores: [
    {
      name: "CAS_MAIN",
      filesystem: {
        content_path: "$HOME/Library/Application Support/nativelink/cas/content",
        temp_path: "$HOME/Library/Application Support/nativelink/cas/tmp",
        eviction_policy: {
          max_bytes: 21474836480,  // 20 GiB
        },
      },
    },
    {
      name: "AC_MAIN",
      filesystem: {
        content_path: "$HOME/Library/Application Support/nativelink/ac/content",
        temp_path: "$HOME/Library/Application Support/nativelink/ac/tmp",
        eviction_policy: {
          max_bytes: 2147483648,  // 2 GiB
        },
      },
    },
    {
      name: "WORKER_FAST",
      filesystem: {
        content_path: "$HOME/Library/Application Support/nativelink/worker-fast/content",
        temp_path: "$HOME/Library/Application Support/nativelink/worker-fast/tmp",
        eviction_policy: {
          max_bytes: 5368709120,  // 5 GiB
        },
      },
    },
    {
      name: "WORKER_CAS_FAST_SLOW",
      fast_slow: {
        fast: {
          ref_store: {
            name: "WORKER_FAST",
          },
        },
        slow: {
          ref_store: {
            name: "CAS_MAIN",
          },
        },
      },
    },
  ],

  schedulers: [
    {
      name: "DARWIN_SCHEDULER",
      simple: {
        supported_platform_properties: {
          OSFamily: "exact",
          ISA: "exact",
          "container-image": "exact",
        },
        max_job_retries: 3,
      },
    },
  ],

  workers: [{
    local: {
      worker_api_endpoint: {
        uri: "grpc://127.0.0.1:50062",
      },
      cas_fast_slow_store: "WORKER_CAS_FAST_SLOW",
      upload_action_result: {
        ac_store: "AC_MAIN",
      },
      work_directory: "$HOME/Library/Application Support/nativelink/work",
      platform_properties: {
        OSFamily: {
          values: ["darwin"],
        },
        ISA: {
          values: ["aarch64"],  // ARM64 (M1/M2/M3)
        },
        "container-image": {
          values: ["rust:latest"],
        },
      },
    },
  }],

  servers: [{
    listener: {
      http: {
        socket_address: "127.0.0.1:50051",
      },
    },
    services: {
      cas: {
        main: {
          cas_store: "CAS_MAIN",
        },
      },
      ac: {
        main: {
          ac_store: "AC_MAIN",
        },
      },
      execution: {
        main: {
          cas_store: "CAS_MAIN",
          scheduler: "DARWIN_SCHEDULER",
        },
      },
      capabilities: {
        main: {
          remote_execution: {
            scheduler: "DARWIN_SCHEDULER",
          },
        },
      },
      bytestream: {
        cas_stores: {
          main: "CAS_MAIN",
        },
      },
    },
  }, {
    listener: {
      http: {
        socket_address: "127.0.0.1:50062",
      },
    },
    services: {
      worker_api: {
        scheduler: "DARWIN_SCHEDULER",
      },
    },
  }],
}
```

### Launch Daemon (auto-start on boot)

Create `/Library/LaunchDaemons/com.tracemachina.nativelink.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.tracemachina.nativelink</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/nativelink</string>
        <string>/usr/local/etc/nativelink/config.json5</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>$HOME/Library/Logs/nativelink.log</string>
    <key>StandardErrorPath</key>
    <string>$HOME/Library/Logs/nativelink.error.log</string>
</dict>
</plist>
```

Load the daemon:
```bash
sudo launchctl load /Library/LaunchDaemons/com.tracemachina.nativelink.plist
```

### Configure objfs

```bash
export OBJFS_REMOTE_ENDPOINT="http://localhost:50051"
export OBJFS_REMOTE_INSTANCE="main"
export OBJFS_REMOTE_TARGETS="aarch64-apple-darwin"
export OBJFS_MIN_REMOTE_SIZE=1
```

### Testing

```bash
cd /tmp
cat > test.rs << 'EOF'
fn main() {
    println!("Hello from macOS worker!");
}
EOF

cargo-objfs-rustc rustc \
  --target aarch64-apple-darwin \
  --crate-name test \
  --edition 2021 \
  --crate-type bin \
  -o test_bin \
  test.rs

# Should compile using local macOS worker
file test_bin
# Mach-O 64-bit executable arm64

./test_bin
# Hello from macOS worker!
```

### Advantages

- **No cross-compilation** -- Native Darwin builds
- **Uses Xcode toolchain** -- Proper Apple SDK and tools
- **Faster** -- No network latency to remote container
- **Simpler** -- No osxcross, no wrapper conflicts
- **Reliable** -- Battle-tested Apple tooling

## Multi-Worker Architecture

Instead of running separate NativeLink instances, run one scheduler that manages multiple workers:

```
objfs client (Mac)
    | gRPC
Scheduler (can run anywhere)
    |-> Worker 1: Mac (localhost for Darwin builds)
    |-> Worker 2: Linux (scheduler-host for cross-platform builds)
```

### Option A: Scheduler on Mac, Workers Connect to It

Run the scheduler on your Mac, and have remote workers connect to it.

#### Mac Configuration (Scheduler + Local Worker)

`/usr/local/etc/nativelink/config.json5`:

```json5
{
  stores: [
    {
      name: "CAS_MAIN",
      filesystem: {
        content_path: "$HOME/Library/Application Support/nativelink/cas/content",
        temp_path: "$HOME/Library/Application Support/nativelink/cas/tmp",
        eviction_policy: {
          max_bytes: 21474836480,  // 20 GiB
        },
      },
    },
    {
      name: "AC_MAIN",
      filesystem: {
        content_path: "$HOME/Library/Application Support/nativelink/ac/content",
        temp_path: "$HOME/Library/Application Support/nativelink/ac/tmp",
        eviction_policy: {
          max_bytes: 2147483648,  // 2 GiB
        },
      },
    },
    {
      name: "WORKER_FAST",
      filesystem: {
        content_path: "$HOME/Library/Application Support/nativelink/worker-fast/content",
        temp_path: "$HOME/Library/Application Support/nativelink/worker-fast/tmp",
        eviction_policy: {
          max_bytes: 5368709120,  // 5 GiB
        },
      },
    },
    {
      name: "WORKER_CAS_FAST_SLOW",
      fast_slow: {
        fast: {
          ref_store: {
            name: "WORKER_FAST",
          },
        },
        slow: {
          ref_store: {
            name: "CAS_MAIN",
          },
        },
      },
    },
  ],

  schedulers: [
    {
      name: "MULTI_SCHEDULER",
      simple: {
        supported_platform_properties: {
          OSFamily: "exact",
          ISA: "exact",
          "container-image": "exact",
        },
        max_job_retries: 3,
      },
    },
  ],

  // LOCAL WORKER (Mac)
  workers: [{
    local: {
      worker_api_endpoint: {
        uri: "grpc://127.0.0.1:50062",
      },
      cas_fast_slow_store: "WORKER_CAS_FAST_SLOW",
      upload_action_result: {
        ac_store: "AC_MAIN",
      },
      work_directory: "$HOME/Library/Application Support/nativelink/work",
      platform_properties: {
        OSFamily: {
          values: ["darwin"],
        },
        ISA: {
          values: ["aarch64"],
        },
        "container-image": {
          values: ["rust:latest"],
        },
      },
    },
  }],

  servers: [
    // Client-facing endpoint (scheduler)
    {
      listener: {
        http: {
          socket_address: "0.0.0.0:50051",
        },
      },
      services: {
        cas: {
          main: {
            cas_store: "CAS_MAIN",
          },
        },
        ac: {
          main: {
            ac_store: "AC_MAIN",
          },
        },
        execution: {
          main: {
            cas_store: "CAS_MAIN",
            scheduler: "MULTI_SCHEDULER",
          },
        },
        capabilities: {
          main: {
            remote_execution: {
              scheduler: "MULTI_SCHEDULER",
            },
          },
        },
        bytestream: {
          cas_stores: {
            main: "CAS_MAIN",
          },
        },
      },
    },
    // Worker API endpoint (for remote workers to connect)
    {
      listener: {
        http: {
          socket_address: "0.0.0.0:50061",
        },
      },
      services: {
        worker_api: {
          scheduler: "MULTI_SCHEDULER",
        },
      },
    },
    // Local worker endpoint
    {
      listener: {
        http: {
          socket_address: "127.0.0.1:50062",
        },
      },
      services: {
        worker_api: {
          scheduler: "MULTI_SCHEDULER",
        },
      },
    },
  ],
}
```

#### Linux Worker Configuration (Remote Worker Mode)

On the Linux container, configure it to connect to the Mac scheduler.

`/etc/nativelink/config.json5`:

```json5
{
  stores: [
    {
      name: "CAS_MAIN",
      grpc: {
        instance_name: "main",
        endpoints: [
          {
            uri: "grpc://10.0.1.1:50051",  // Mac's IP
          },
        ],
        store_type: "cas",
      },
    },
    {
      name: "WORKER_FAST",
      filesystem: {
        content_path: "/var/lib/nativelink/worker-fast/content",
        temp_path: "/var/lib/nativelink/worker-fast/tmp",
        eviction_policy: {
          max_bytes: 10737418240,  // 10 GiB
        },
      },
    },
    {
      name: "WORKER_CAS_FAST_SLOW",
      fast_slow: {
        fast: {
          ref_store: {
            name: "WORKER_FAST",
          },
        },
        slow: {
          ref_store: {
            name: "CAS_MAIN",
          },
        },
      },
    },
  ],

  schedulers: [],  // No scheduler, this is just a worker

  workers: [{
    local: {
      worker_api_endpoint: {
        uri: "grpc://10.0.1.1:50061",  // Connect to Mac's worker API
      },
      cas_fast_slow_store: "WORKER_CAS_FAST_SLOW",
      upload_action_result: {
        ac_store: "CAS_MAIN",
      },
      work_directory: "/var/lib/nativelink/work",
      platform_properties: {
        OSFamily: {
          values: ["linux"],
        },
        ISA: {
          values: ["x86-64"],
        },
        "container-image": {
          values: ["rust:latest"],
        },
      },
    },
  }],

  servers: [],  // Worker-only, no server endpoints
}
```

### Option B: Scheduler on Linux, Mac Worker Connects

Alternatively, keep the scheduler on the Linux container and have your Mac connect as a worker.

### Usage

With this setup:

```bash
# objfs connects to the scheduler (Mac in Option A)
export OBJFS_REMOTE_ENDPOINT="http://localhost:50051"  # or Mac's IP from other machines
export OBJFS_REMOTE_INSTANCE="main"

# Scheduler will automatically:
# - Send Darwin builds to Mac worker
# - Send Linux builds to Linux worker
# - Use whichever worker is available and matches platform
```

The scheduler handles work distribution automatically based on platform properties.

### Benefits

- **Automatic worker selection** -- Scheduler picks the right worker
- **Load balancing** -- Multiple workers for same platform can share work
- **Unified CAS** -- All workers share the same content-addressable storage
- **Shared cache** -- AC (Action Cache) benefits all workers
- **Fault tolerance** -- If one worker fails, others can continue

### Verification

```bash
# Compile for Darwin - should use Mac worker
cargo build --target aarch64-apple-darwin

# Compile for Linux - should use Linux worker
cargo build --target x86_64-unknown-linux-gnu
```

Check NativeLink logs to see which worker handled each build:
```bash
# Mac:
tail -f $HOME/Library/Logs/nativelink.log

# Linux:
incus exec nativelink-worker -- journalctl -u nativelink -f
```

## Docker/Container Workers

This section covers configuring NativeLink workers in Docker containers to compile Rust code remotely.

### Option 1: Using ADDITIONAL_SETUP_WORKER_CMD (Recommended)

Install Rust directly in the NativeLink worker container.

Set environment variable in `nativelink/deployment-examples/docker-compose/.env` (in your NativeLink source directory):

```bash
ADDITIONAL_SETUP_WORKER_CMD="curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && \
  . \$HOME/.cargo/env && \
  rustup target add aarch64-apple-darwin && \
  rustup target add x86_64-apple-darwin"
```

Update `worker.json5` platform properties:

```json5
platform_properties: {
  cpu_count: {
    query_cmd: "nproc",
  },
  OSFamily: {
    values: ["Linux"],
  },
  "container-image": {
    values: ["rust:latest"],
  },
  ISA: {
    values: ["x86-64"],
  },
},
```

Rebuild and restart:

```bash
cd nativelink/deployment-examples/docker-compose
docker-compose down
docker-compose build --no-cache nativelink_executor
docker-compose up -d
```

Verify Rust is installed:

```bash
docker-compose exec nativelink_executor bash -c ". ~/.cargo/env && rustc --version"
```

### Option 2: Custom Dockerfile

Create `Dockerfile.rust`:

```dockerfile
# Start from NativeLink's base
FROM trace_machina/nativelink:latest

# Install Rust toolchain
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Install macOS cross-compilation targets
RUN rustup target add aarch64-apple-darwin && \
    rustup target add x86_64-apple-darwin

EXPOSE 50051/tcp 50052/tcp
CMD ["nativelink"]
```

Update `docker-compose.yml`:

```yaml
nativelink_executor:
  build:
    context: .
    dockerfile: Dockerfile.rust
  # ... rest of configuration
```

### Option 3: Official Rust Docker Image

Modify `docker-compose.yml` to use the Rust base image:

```yaml
nativelink_executor:
  image: rust:latest
  volumes:
    - ${NATIVELINK_DIR:-~/.cache/nativelink}:/root/.cache/nativelink
    - type: bind
      source: .
      target: /root
  environment:
    RUST_LOG: ${RUST_LOG:-warn}
    CAS_ENDPOINT: nativelink_local_cas
    SCHEDULER_ENDPOINT: nativelink_scheduler
  command: |
    # Install NativeLink in the Rust container
    cargo install --git https://github.com/TraceMachina/nativelink nativelink && \
    nativelink /root/worker.json5
  depends_on:
    - nativelink_local_cas
    - nativelink_scheduler
```

**Note**: This option installs NativeLink on every container start, which is slow. Building a custom image is preferred.

### Testing Remote Compilation

```bash
cd /tmp
cat > test.rs << 'EOF'
fn main() {
    println!("Hello from remote execution!");
}
EOF

export OBJFS_REMOTE_ENDPOINT="http://scheduler-host:50051"
export OBJFS_REMOTE_INSTANCE="main"
export OBJFS_REMOTE_TARGETS="aarch64-apple-darwin"
export OBJFS_MIN_REMOTE_SIZE=1

cargo-objfs-rustc rustc \
  --target aarch64-apple-darwin \
  --crate-name test \
  --edition 2021 \
  --crate-type bin \
  -o test_bin \
  test.rs
```

### Cross-Compilation Limitations

**What works:**
- Compiling Rust code on Linux for Linux targets
- Compiling Rust code (rustc) for macOS targets
- Generating `.rlib` files for macOS

**What does not work without extra setup:**
- **Linking** final macOS binaries on Linux (requires macOS SDK + osxcross)
- Creating macOS executables (needs Apple linker)

**Workarounds:**
- **For libraries (`.rlib`)**: Works without extra setup. Compile with `--crate-type lib`.
- **For binaries**: Use remote execution for library compilation only, link locally.

### Recommended Configuration

```bash
# .env file
ADDITIONAL_SETUP_WORKER_CMD="curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && \
  . \$HOME/.cargo/env && \
  rustup target add aarch64-apple-darwin && \
  rustup target add x86_64-apple-darwin && \
  rustup component add rust-src"
```

```json5
// worker.json5
platform_properties: {
  OSFamily: { values: ["Linux"] },
  "container-image": { values: ["rust:latest"] },
  ISA: { values: ["x86-64"] },
  "rust-version": { values: ["1.75.0"] },
  "rust-targets": { values: ["aarch64-apple-darwin", "x86_64-apple-darwin"] },
}
```

## Cross-Compilation from Linux

### Problem Statement

Compiling Rust code for macOS (aarch64-apple-darwin) on a remote Linux worker requires:
1. Rust compiler with macOS target stdlib
2. macOS SDK for linking
3. Darwin-aware linker

### Network Configuration

If the NativeLink worker runs in an incus container on an isolated network, use proxy devices to make it accessible:

```bash
incus config device show nativelink-worker
# proxy-public:
#   connect: tcp:127.0.0.1:50051
#   listen: tcp:10.0.1.2:50051
#   type: proxy
```

The proxy should listen on the host's actual IP rather than 0.0.0.0 to work properly. NativeLink inside the container should listen on 0.0.0.0:50051.

### Rust Toolchain with rust-overlay (NixOS)

NixOS uses non-standard library paths (`/nix/store/...`), making standard rustup installations fail. Use [oxalica/rust-overlay](https://github.com/oxalica/rust-overlay) instead:

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

The rust-overlay downloads pre-built `rust-std` for `aarch64-apple-darwin` from official Rust sources, providing the standard library for Darwin without needing to compile it.

### Platform Properties

The scheduler must accept the platform properties that objfs sends:

```json5
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

Use `ISA` (not `Arch`) to match NativeLink conventions.

Worker `platform_properties` describe the **worker's** platform (Linux), not the **target** platform (macOS). Remote execution happens on Linux, producing binaries for macOS.

### WARNING: osxcross Does Not Work on NixOS

osxcross wraps `clang` to inject Darwin-specific flags. NixOS also wraps `clang` to inject NixOS-specific paths. When these two wrapper systems interact, the linker fails:

```
ld: unrecognised emulation mode: llvm
Supported emulations: elf_x86_64 elf32_x86_64 elf_i386 elf_iamcu
```

This is a fundamental incompatibility between osxcross's compiler wrappers and NixOS's compiler wrappers.

### Alternative Solutions

**Option A: Use Standard Linux Container (Recommended)**
- Replace NixOS container with Ubuntu/Debian for the NativeLink worker
- osxcross works normally on standard Linux distributions

**Option B: Use Zig CC as Cross-Compiler**
- [cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild) can cross-compile to macOS
- No macOS SDK required (uses Zig's bundled libc)
- Works on NixOS without wrapper conflicts

**Option C: Native macOS Worker**
- Run NativeLink worker on macOS machine
- No cross-compilation needed; most reliable but requires macOS hardware

**Option D: Rust-lld with macOS SDK**
- Use Rust's bundled `lld` linker directly
- Provide macOS SDK from osxcross
- Bypass both osxcross and NixOS wrappers; experimental

### Lessons Learned

1. **NixOS + osxcross do not mix** -- Wrapper conflicts are fundamental
2. **Standard Linux preferred** -- For cross-compilation toolchains, avoid NixOS
3. **Zig CC is promising** -- Modern alternative to osxcross
4. **SDK extraction works** -- The macOS SDK can be reused with other toolchains

## Troubleshooting

### Worker does not start

```bash
# Docker workers:
docker-compose logs nativelink_executor

# macOS Launch Daemon:
sudo launchctl list | grep nativelink
cat $HOME/Library/Logs/nativelink.error.log
```

### Rust not found in worker

```bash
docker-compose exec nativelink_executor which rustc
docker-compose exec nativelink_executor bash -c ". ~/.cargo/env && rustc --version"
```

### Remote execution times out

1. Check worker logs: `docker-compose logs -f nativelink_executor`
2. Verify platform properties match in worker.json5
3. Ensure OBJFS_REMOTE_TARGETS includes your target
4. Test rustc directly in the container

### "No candidate workers due to lack of matching X"

Check that:
- Scheduler's `supported_platform_properties` includes the property
- Worker's `platform_properties` advertises the correct values
- objfs sends matching platform properties (OSFamily=linux, ISA=x86-64)

### "Command not found: rustc"

Options:
1. Use absolute path: `/root/.nix-profile/bin/rustc`
2. Set PATH in Command proto's `environment_variables`
3. Update worker's `additional_environment`

### Container networking issues

If Mac cannot reach scheduler-host:50051:
1. Verify incus proxy: `incus config device show nativelink-worker`
2. Check proxy listen address matches host IP
3. Test with curl: `curl -v http://scheduler-host:50051`
4. Verify NativeLink is listening: `incus exec nativelink-worker -- ss -tlnp | grep 50051`
