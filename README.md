# objfs

Distributed build cache for Rust using NativeLink's Remote Execution API.

## Features

- **Distributed caching** -- Share build artifacts across machines
- **Remote execution** -- Compile on worker machines, link locally
- **Auto-worker registration** -- Every machine contributes to the cluster
- **C/C++ support** -- Cache GCC and Clang compilations via `objfs-cc-wrapper`

## Install

```bash
cargo build --release
sudo cp target/release/cargo-objfs-rustc /usr/local/bin/
```

objfs intercepts `rustc` through Cargo's wrapper mechanism. No code changes required.

## Usage

```bash
# Local caching (zero config)
cargo build --release

# Shared cluster
export OBJFS_REMOTE_ENDPOINT="http://build-server:50051"
cargo build
```

See [USAGE.md](USAGE.md) for configuration, CI/CD setup, and C/C++ integration.

## How It Works

```
Developer Machines --> NativeLink Scheduler --> Worker Pool
                              |
                     CAS (artifacts) + AC (cache)
```

1. `cargo-objfs-rustc` intercepts each `rustc` invocation
2. Computes a cache key from source files and flags
3. Checks the local cache, then the shared remote cache
4. On a miss, submits the build to the scheduler or compiles locally
5. Stores the result for reuse by any machine in the cluster

## Documentation

**Guides:**
- [Remote Execution](docs/guides/remote-execution.md) -- Distributed builds
- [Worker Setup](docs/guides/worker-setup.md) -- NativeLink worker configuration
- [CI/CD Integration](docs/guides/ci-cd.md) -- GitHub Actions, GitLab CI
- [C/C++ Integration](docs/guides/c-cpp-integration.md) -- CMake, Make builds

**Reference:**
- [Architecture](docs/reference/architecture.md) -- Technical details
- [Linking Strategy](docs/reference/linking-strategy.md) -- Platform-compatible linking
- [Auto-Worker Registration](docs/reference/auto-worker-registration.md) -- Zero-config clustering
- [Roadmap](docs/reference/roadmap.md) -- C/C++ feature tracking

## License

MIT
