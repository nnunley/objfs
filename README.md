# objfs

Distributed build cache for Rust using NativeLink's Remote Execution API.

## Features

- **Distributed caching** - Share build artifacts across team
- **Remote execution** - Compile on worker machines
- **Platform-compatible linking** - Link on matching platforms
- **Auto-worker registration** - Zero-config cluster participation
- **CI/CD integration** - GitHub Actions, GitLab CI ready

## Quick Start

```bash
# Install
cd objfs
cargo build --release
sudo cp target/release/cargo-objfs-rustc /usr/local/bin/

# Use (local caching)
cargo build --release
```

## Shared Cluster

**Scheduler (one machine):**
```bash
cargo install nativelink
sudo mkdir -p /etc/nativelink /var/lib/nativelink
# Copy config from docs/guides/quickstart.md
sudo nativelink /etc/nativelink/config.json5
```

**Developers (all machines):**
```bash
export OBJFS_REMOTE_ENDPOINT="http://build-server:50051"
export OBJFS_REMOTE_INSTANCE="main"
cargo build
```

Each machine auto-registers as a worker and shares cache.

## Performance

- Small projects: 450x faster (45s → 100ms)
- Large monorepos: 300x faster (15m → 3s)
- CI builds: 73% cost reduction

## CI/CD

**GitHub Actions:**
```yaml
- run: curl -L .../cargo-objfs-rustc -o /usr/local/bin/cargo-objfs-rustc
- run: echo 'rustc-wrapper = "/usr/local/bin/cargo-objfs-rustc"' > .cargo/config.toml
- run: cargo build --release
env:
  OBJFS_REMOTE_ENDPOINT: "http://build-cluster:50051"
```

See `examples/ci/` for complete workflows.

## Configuration

| Variable | Purpose | Default |
|----------|---------|---------|
| `OBJFS_REMOTE_ENDPOINT` | Scheduler URL | localhost |
| `OBJFS_REMOTE_INSTANCE` | Instance name | main |
| `OBJFS_NO_AUTO_WORKER` | Skip worker startup | unset |
| `OBJFS_MIN_REMOTE_SIZE` | Min file size | 100 KB |
| `OBJFS_DISABLE` | Disable objfs | unset |

## Architecture

```
Developer Machines → NativeLink Scheduler → Worker Pool
                            ↓
                   CAS (artifacts) + AC (cache)
```

- **CAS**: Content-addressable storage (SHA256-indexed artifacts)
- **AC**: Action cache (build command → outputs)
- **Workers**: Auto-register, execute builds, share cache

## Documentation

- [Quick Start](docs/guides/quickstart.md) - Installation and setup
- [Remote Execution](docs/guides/remote-execution.md) - Distributed builds
- [Worker Setup](docs/guides/worker-setup.md) - NativeLink worker configuration
- [CI/CD Integration](docs/guides/ci-cd.md) - GitHub Actions, GitLab CI
- [C/C++ Integration](docs/guides/c-cpp-integration.md) - CMake, Make builds
- [Architecture](docs/reference/architecture.md) - Technical details
- [Linking Strategy](docs/reference/linking-strategy.md) - Platform-compatible linking
- [Auto-Worker Registration](docs/reference/auto-worker-registration.md) - Zero-config clustering
- [Roadmap](ROADMAP.md) - C/C++ feature tracking

## License

MIT
