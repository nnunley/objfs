# objfs Quick Start

Get distributed build caching and remote execution working in 5 minutes.

## Installation

```bash
cd objfs
cargo build --release
sudo cp target/release/cargo-objfs-rustc /usr/local/bin/
```

## Option 1: Local Caching Only (Zero Config)

Use objfs for local caching with automatic worker:

```bash
# Just use it - auto-starts local worker
cargo build --release
```

That's it! objfs will:
- Auto-start a local NativeLink worker
- Cache build artifacts locally
- Reuse cached builds instantly

## Option 2: Shared Build Cluster

### Step 1: Set Up Scheduler (One Machine)

On your build server or a shared machine:

```bash
# Install NativeLink
cargo install nativelink

# Create config directory
sudo mkdir -p /etc/nativelink

# Create scheduler config
cat > /tmp/scheduler-config.json5 << 'EOF'
{
  stores: [
    {
      name: "CAS_MAIN",
      filesystem: {
        content_path: "/var/lib/nativelink/cas/content",
        temp_path: "/var/lib/nativelink/cas/tmp",
        eviction_policy: { max_bytes: 107374182400 },  // 100 GiB
      },
    },
    {
      name: "AC_MAIN",
      filesystem: {
        content_path: "/var/lib/nativelink/ac/content",
        temp_path: "/var/lib/nativelink/ac/tmp",
        eviction_policy: { max_bytes: 10737418240 },  // 10 GiB
      },
    },
  ],

  schedulers: [{
    name: "MAIN_SCHEDULER",
    simple: {
      supported_platform_properties: {
        OSFamily: "exact",
        ISA: "exact",
        "container-image": "exact",
      },
      max_job_retries: 3,
    },
  }],

  workers: [],  // Workers auto-register

  servers: [{
    listener: {
      http: { socket_address: "0.0.0.0:50051" },
    },
    services: {
      cas: { main: { cas_store: "CAS_MAIN" } },
      ac: { main: { ac_store: "AC_MAIN" } },
      execution: {
        main: {
          cas_store: "CAS_MAIN",
          scheduler: "MAIN_SCHEDULER",
        },
      },
      capabilities: {
        main: {
          remote_execution: { scheduler: "MAIN_SCHEDULER" },
        },
      },
      bytestream: { cas_stores: { main: "CAS_MAIN" } },
    },
  }, {
    listener: {
      http: { socket_address: "0.0.0.0:50061" },
    },
    services: {
      worker_api: { scheduler: "MAIN_SCHEDULER" },
    },
  }],
}
EOF

sudo mv /tmp/scheduler-config.json5 /etc/nativelink/config.json5
sudo mkdir -p /var/lib/nativelink

# Start scheduler
sudo nativelink /etc/nativelink/config.json5
```

### Step 2: Configure Developer Machines

On every dev machine:

```bash
# Point to the scheduler
export OBJFS_REMOTE_ENDPOINT="http://build-server:50051"
export OBJFS_REMOTE_INSTANCE="main"

# Add to ~/.bashrc or ~/.zshrc to persist
echo 'export OBJFS_REMOTE_ENDPOINT="http://build-server:50051"' >> ~/.zshrc
echo 'export OBJFS_REMOTE_INSTANCE="main"' >> ~/.zshrc

# Build anything
cargo build
```

That's it! Each dev machine will:
1. Auto-register as a worker with the scheduler
2. Participate in distributed builds
3. Share cache with all other developers

## Verify It's Working

```bash
# Build something
cd /tmp
cargo init --bin hello
cd hello

# First build (cold cache)
time cargo build --release
# Takes ~2-5 seconds

# Clean and rebuild (hot cache)
cargo clean
time cargo build --release
# Takes ~100ms (instant!)
```

Check cache hit:
```bash
cargo build --release 2>&1 | grep objfs
# [objfs] cache hit: hello
```

## Configuration Options

### Disable for Specific Builds

```bash
# Temporarily disable
OBJFS_DISABLE=1 cargo build

# Or exclude this project
echo 'OBJFS_DISABLE=1' > .cargo/config.toml
```

### Adjust Cache Behavior

```bash
# Only cache files larger than 1 MB
export OBJFS_MIN_REMOTE_SIZE=1048576

# Cache everything (including small files)
export OBJFS_MIN_REMOTE_SIZE=1
```

### Client-Only Mode (No Auto-Worker)

```bash
# Don't start local worker, only use remote workers
export OBJFS_NO_AUTO_WORKER=1
cargo build
```

## Monitoring

### Check Scheduler Status

```bash
# On scheduler machine
curl http://localhost:50051/health

# Check CAS size
du -sh /var/lib/nativelink/cas

# Check AC size
du -sh /var/lib/nativelink/ac
```

### Check Worker Registration

```bash
# On scheduler machine - check logs
journalctl -u nativelink -f

# Should see:
# Worker registered: darwin/aarch64 from 10.0.1.10
# Worker registered: linux/x86-64 from 10.0.1.11
```

### Monitor Cache Hits

```bash
# In your project
cargo clean
cargo build 2>&1 | grep -E "objfs|cache"

# Output:
# [objfs] cache hit: libfoo.rlib
# [objfs] cache hit: main.o
# [objfs] cache hit: myapp
```

## Troubleshooting

### "Connection refused" Error

Scheduler not reachable:
```bash
# Test connectivity
curl -v http://build-server:50051/health

# Check firewall
sudo ufw allow 50051/tcp
sudo ufw allow 50061/tcp
```

### Builds Not Cached

Check configuration:
```bash
# Verify endpoint
echo $OBJFS_REMOTE_ENDPOINT

# Test manually
cargo-objfs-rustc rustc --version
# Should see worker start message
```

### Worker Won't Start

Check if `nativelink` is installed:
```bash
which nativelink
# If not found:
cargo install nativelink
```

### Cache Consuming Too Much Disk

Reduce cache size:
```bash
# On scheduler
# Edit /etc/nativelink/config.json5
# Change max_bytes to smaller value

# Or clean manually
rm -rf /var/lib/nativelink/cas/*
rm -rf /var/lib/nativelink/ac/*
```

## Next Steps

- Read [ARCHITECTURE.md](ARCHITECTURE.md) for technical details
- See [MULTI_WORKER_SETUP.md](MULTI_WORKER_SETUP.md) for advanced configuration
- Check [AUTO_WORKER_REGISTRATION.md](AUTO_WORKER_REGISTRATION.md) for worker details

## Performance Tips

1. **Use fast storage for CAS** - SSD recommended
2. **Increase cache size** - More cache = higher hit rates
3. **Collocate scheduler and primary workers** - Reduces network latency
4. **Use GbE or faster network** - Large artifacts transfer faster
5. **Enable on CI/CD** - Massive speedup from shared cache

## Example Speedups

**Typical Rust project:**
- Cold build: 45 seconds
- Hot build (local cache): 100ms
- Hit rate after warmup: >90%

**Large monorepo:**
- Cold build: 15 minutes
- Hot build: 2-3 seconds
- CI builds: 80% faster with shared cache
