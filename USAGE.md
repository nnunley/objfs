# Usage

## Installation

Build from source:

```bash
cargo build --release
sudo cp target/release/cargo-objfs-rustc /usr/local/bin/
```

## Local Caching

objfs caches build artifacts with no configuration. Run any Cargo build:

```bash
cargo build --release
```

On the first build, objfs starts a local worker and stores artifacts in a
content-addressed cache. Subsequent builds reuse cached artifacts, including
after `cargo clean`.

## Shared Cluster

A shared NativeLink scheduler lets multiple machines share cached artifacts.
One machine runs the scheduler; all others connect to it.

### Scheduler Setup

```bash
cargo install nativelink
sudo mkdir -p /etc/nativelink /var/lib/nativelink

# Write scheduler config (see docs/guides/quickstart.md for full config)
sudo nativelink /etc/nativelink/config.json5
```

### Developer Setup

Point each machine at the scheduler:

```bash
export OBJFS_REMOTE_ENDPOINT="http://build-server:50051"
export OBJFS_REMOTE_INSTANCE="main"
```

Add these to your shell profile (`~/.zshrc` or `~/.bashrc`) to persist them.
Each machine auto-registers as a worker and contributes to the shared cache.

## Configuration

| Variable | Purpose | Default |
|----------|---------|---------|
| `OBJFS_REMOTE_ENDPOINT` | Scheduler URL | localhost |
| `OBJFS_REMOTE_INSTANCE` | Instance name | main |
| `OBJFS_NO_AUTO_WORKER` | Skip worker startup | unset |
| `OBJFS_MIN_REMOTE_SIZE` | Min file size for remote cache | 100 KB |
| `OBJFS_REMOTE_TARGETS` | Worker platform capabilities | host triple |
| `OBJFS_DISABLE` | Disable objfs entirely | unset |

### Common Settings

```bash
# Disable objfs for one build
OBJFS_DISABLE=1 cargo build

# Cache everything (useful in CI)
export OBJFS_MIN_REMOTE_SIZE=1

# Client-only mode (don't start a local worker)
export OBJFS_NO_AUTO_WORKER=1
```

## CI/CD

### GitHub Actions

```yaml
env:
  OBJFS_REMOTE_ENDPOINT: "http://build-cluster:50051"
  OBJFS_NO_AUTO_WORKER: "1"
  OBJFS_MIN_REMOTE_SIZE: "1"

steps:
  - name: Install objfs
    run: |
      curl -L https://github.com/yourorg/objfs/releases/latest/download/cargo-objfs-rustc-linux-x86_64 \
        -o /tmp/cargo-objfs-rustc
      chmod +x /tmp/cargo-objfs-rustc
      sudo mv /tmp/cargo-objfs-rustc /usr/local/bin/

  - name: Configure cargo
    run: |
      mkdir -p .cargo
      echo '[build]' > .cargo/config.toml
      echo 'rustc-wrapper = "/usr/local/bin/cargo-objfs-rustc"' >> .cargo/config.toml

  - name: Build
    run: cargo build --release
```

Set `OBJFS_NO_AUTO_WORKER=1` in CI to prevent runners from starting isolated
workers. All runners should share the same scheduler endpoint.

See [CI/CD Integration](docs/guides/ci-cd.md) for GitLab CI, multi-target
builds, and performance benchmarks.

## C/C++ Integration

objfs caches C/C++ compilation through `objfs-cc-wrapper`:

```bash
cargo build --release --bin objfs-cc-wrapper
sudo cp target/release/objfs-cc-wrapper /usr/local/bin/
```

### CMake

```bash
cmake .. \
  -DCMAKE_C_COMPILER_LAUNCHER=/usr/local/bin/objfs-cc-wrapper \
  -DCMAKE_CXX_COMPILER_LAUNCHER=/usr/local/bin/objfs-cc-wrapper
```

### Make

```bash
CC="objfs-cc-wrapper gcc" CXX="objfs-cc-wrapper g++" make
```

See [C/C++ Integration](docs/guides/c-cpp-integration.md) for Autotools,
direct usage, and limitations.

## Verify

```bash
cd /tmp && cargo init --bin hello && cd hello
time cargo build --release    # cold
cargo clean
time cargo build --release    # hot (should be ~100ms)
cargo build --release 2>&1 | grep objfs
```

## Monitoring

```bash
# Scheduler health
curl http://build-server:50051/health

# CAS disk usage
du -sh /var/lib/nativelink/{cas,ac}

# Cache hits
cargo build 2>&1 | grep objfs
```

## Troubleshooting

**Connection refused:**
Verify the scheduler is running and the endpoint is reachable:
```bash
curl -v http://build-server:50051/health
```

**No cache hits:**
Check that `OBJFS_REMOTE_ENDPOINT` and `OBJFS_REMOTE_INSTANCE` match the
scheduler configuration.

**Worker won't start:**
Ensure `nativelink` is in PATH:
```bash
which nativelink
cargo install nativelink
```

**Cache too large:**
Edit the scheduler config to reduce `max_bytes`, or clear the cache:
```bash
rm -rf /var/lib/nativelink/{cas,ac}/*
```
