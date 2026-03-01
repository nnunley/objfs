# objfs Quick Start

Distributed build caching in 5 minutes.

## Installation

```bash
cd objfs
cargo build --release
sudo cp target/release/cargo-objfs-rustc /usr/local/bin/
```

## Local Caching (Zero Config)

```bash
cargo build --release
```

objfs auto-starts a worker, caches artifacts, and reuses them instantly.

## Shared Build Cluster

### Step 1: Set Up Scheduler

On your build server:

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

### Step 2: Configure Developers

On each machine:

```bash
export OBJFS_REMOTE_ENDPOINT="http://build-server:50051"
export OBJFS_REMOTE_INSTANCE="main"
echo 'export OBJFS_REMOTE_ENDPOINT="http://build-server:50051"' >> ~/.zshrc
echo 'export OBJFS_REMOTE_INSTANCE="main"' >> ~/.zshrc
cargo build
```

Each machine auto-registers as a worker and shares cache with the cluster.

## Verify

```bash
cd /tmp && cargo init --bin hello && cd hello
time cargo build --release  # 2-5 seconds (cold)
cargo clean
time cargo build --release  # ~100ms (hot)
cargo build --release 2>&1 | grep objfs  # Shows cache hits
```

## Configuration

**Disable objfs:**
```bash
OBJFS_DISABLE=1 cargo build
```

**Adjust cache threshold:**
```bash
export OBJFS_MIN_REMOTE_SIZE=1048576  # 1 MB minimum
export OBJFS_MIN_REMOTE_SIZE=1        # Cache everything
```

**Client-only mode:**
```bash
export OBJFS_NO_AUTO_WORKER=1
```

## Monitoring

**Scheduler status:**
```bash
curl http://localhost:50051/health
du -sh /var/lib/nativelink/{cas,ac}
```

**Worker registration:**
```bash
journalctl -u nativelink -f
```

**Cache hits:**
```bash
cargo build 2>&1 | grep objfs
```

## Troubleshooting

**Connection refused:**
```bash
curl -v http://build-server:50051/health
sudo ufw allow 50051/tcp 50061/tcp
```

**No cache hits:**
```bash
echo $OBJFS_REMOTE_ENDPOINT
cargo-objfs-rustc rustc --version
```

**Worker won't start:**
```bash
cargo install nativelink
```

**Cache too large:**
Edit `/etc/nativelink/config.json5` and reduce `max_bytes`, or:
```bash
rm -rf /var/lib/nativelink/{cas,ac}/*
```

## Performance

Use SSD storage, increase cache size, and place scheduler near workers for best results.

**Typical speedups:**
- Small project: 45s → 100ms (450x)
- Large monorepo: 15m → 3s (300x)
- CI builds: 80% faster

## Further Reading

- [ARCHITECTURE.md](ARCHITECTURE.md) - Technical details
- [examples/ci/](examples/ci/) - CI/CD integration
- [AUTO_WORKER_REGISTRATION.md](AUTO_WORKER_REGISTRATION.md) - Worker setup
