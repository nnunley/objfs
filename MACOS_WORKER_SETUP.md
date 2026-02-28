# Setting Up macOS as NativeLink Worker

## Overview

Run a NativeLink worker on your Mac to handle Darwin compilation natively, avoiding cross-compilation complexity.

## Installation

### 1. Install NativeLink via Homebrew (if available) or Build from Source

**Option A: Homebrew (check if available)**
```bash
brew install nativelink
```

**Option B: Build from source**
```bash
# Install Rust if not already installed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build NativeLink
cd your-projects
git clone https://github.com/TraceMachina/nativelink.git
cd nativelink
cargo build --release

# Copy binary to PATH
sudo cp target/release/nativelink /usr/local/bin/
```

### 2. Create Worker Configuration

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
        content_path: "$HOME/.local/share/nativelink/cas/content",
        temp_path: "$HOME/.local/share/nativelink/cas/tmp",
        eviction_policy: {
          max_bytes: 21474836480,  // 20 GiB
        },
      },
    },
    {
      name: "AC_MAIN",
      filesystem: {
        content_path: "$HOME/.local/share/nativelink/ac/content",
        temp_path: "$HOME/.local/share/nativelink/ac/tmp",
        eviction_policy: {
          max_bytes: 2147483648,  // 2 GiB
        },
      },
    },
    {
      name: "WORKER_FAST",
      filesystem: {
        content_path: "$HOME/.local/share/nativelink/worker-fast/content",
        temp_path: "$HOME/.local/share/nativelink/worker-fast/tmp",
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
        uri: "grpc://127.0.0.1:50062",  // Different port than Linux worker
      },
      cas_fast_slow_store: "WORKER_CAS_FAST_SLOW",
      upload_action_result: {
        ac_store: "AC_MAIN",
      },
      work_directory: "$HOME/.local/share/nativelink/work",
      platform_properties: {
        OSFamily: {
          values: ["darwin"],  // macOS
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
        socket_address: "127.0.0.1:50051",  // Same as Linux worker for client compatibility
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

### 3. Create Launch Daemon (auto-start on boot)

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
    <string>$HOME/.local/share/nativelink/nativelink.log</string>
    <key>StandardErrorPath</key>
    <string>$HOME/.local/share/nativelink/nativelink.error.log</string>
</dict>
</plist>
```

Load the daemon:
```bash
sudo launchctl load /Library/LaunchDaemons/com.tracemachina.nativelink.plist
```

### 4. Update objfs Configuration

In `src/platform.rs`, the client already sends platform properties. The scheduler will match based on:
- `OSFamily=darwin` for macOS builds
- `OSFamily=linux` for Linux builds (if you keep the Linux worker)

### 5. Configure objfs to Use Local Worker

```bash
# For local macOS worker:
export OBJFS_REMOTE_ENDPOINT="http://localhost:50051"
export OBJFS_REMOTE_INSTANCE="main"
export OBJFS_REMOTE_TARGETS="aarch64-apple-darwin"
export OBJFS_MIN_REMOTE_SIZE=1
```

## Testing

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

## Advantages

✅ **No cross-compilation** - Native Darwin builds
✅ **Uses Xcode toolchain** - Proper Apple SDK and tools
✅ **Faster** - No network latency to remote container
✅ **Simpler** - No osxcross, no wrapper conflicts
✅ **Reliable** - Battle-tested Apple tooling

## Optional: Multi-Worker Setup

You can run both workers simultaneously:
- **macOS worker** (localhost:50051) - Handles Darwin builds
- **Linux worker** (scheduler-host:50051) - Handles Linux builds

Update objfs to select worker based on target platform.
