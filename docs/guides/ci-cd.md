# CI/CD Integration

## Overview

objfs enables shared build caching across CI runners by connecting them to a common NativeLink scheduler. Each runner contributes to and benefits from a shared cache, so subsequent builds reuse previously compiled artifacts regardless of which runner executes them.

## GitHub Actions

### 3-Step Integration

```yaml
# Step 1: Install objfs wrapper
- run: |
    curl -L https://github.com/yourorg/objfs/releases/latest/download/cargo-objfs-rustc-linux-x86_64 \
      -o /tmp/cargo-objfs-rustc
    chmod +x /tmp/cargo-objfs-rustc
    sudo mv /tmp/cargo-objfs-rustc /usr/local/bin/

# Step 2: Configure cargo
- run: |
    mkdir -p .cargo
    cat > .cargo/config.toml << 'EOF'
    [build]
    rustc-wrapper = "/usr/local/bin/cargo-objfs-rustc"
    EOF

# Step 3: Build normally
- run: cargo build --release
```

No code changes required.

### Full Workflow

```yaml
# .github/workflows/build.yml
name: Build with objfs

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

env:
  OBJFS_REMOTE_ENDPOINT: "http://build-cluster.company.com:50051"
  OBJFS_REMOTE_INSTANCE: "main"
  OBJFS_NO_AUTO_WORKER: "1"
  OBJFS_MIN_REMOTE_SIZE: "1"

jobs:
  build:
    runs-on: ubuntu-latest

    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: stable

      - name: Install objfs
        run: |
          curl -L https://github.com/yourorg/objfs/releases/latest/download/cargo-objfs-rustc-linux-x86_64 \
            -o /tmp/cargo-objfs-rustc
          chmod +x /tmp/cargo-objfs-rustc
          sudo mv /tmp/cargo-objfs-rustc /usr/local/bin/
          cargo-objfs-rustc --version || echo "objfs wrapper installed"

      - name: Configure cargo to use objfs
        run: |
          mkdir -p .cargo
          cat > .cargo/config.toml << 'EOF'
          [build]
          rustc-wrapper = "/usr/local/bin/cargo-objfs-rustc"
          EOF

      - name: Build project
        run: cargo build --release

      - name: Run tests
        run: cargo test --release

  build-multiple-targets:
    strategy:
      matrix:
        target:
          - x86_64-unknown-linux-gnu
          - aarch64-unknown-linux-gnu
    runs-on: ubuntu-latest

    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: stable
          targets: ${{ matrix.target }}

      - name: Install objfs
        run: |
          curl -L https://github.com/yourorg/objfs/releases/latest/download/cargo-objfs-rustc-linux-x86_64 \
            -o /tmp/cargo-objfs-rustc
          chmod +x /tmp/cargo-objfs-rustc
          sudo mv /tmp/cargo-objfs-rustc /usr/local/bin/

      - name: Configure cargo
        run: |
          mkdir -p .cargo
          cat > .cargo/config.toml << 'EOF'
          [build]
          rustc-wrapper = "/usr/local/bin/cargo-objfs-rustc"
          EOF

      - name: Build for ${{ matrix.target }}
        run: cargo build --release --target ${{ matrix.target }}
        env:
          OBJFS_REMOTE_ENDPOINT: "http://build-cluster.company.com:50051"
          OBJFS_REMOTE_INSTANCE: "main"
          OBJFS_NO_AUTO_WORKER: "1"
          OBJFS_MIN_REMOTE_SIZE: "1"

      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: binary-${{ matrix.target }}
          path: target/${{ matrix.target }}/release/your-binary
```

## GitLab CI

See `examples/ci/gitlab-ci.yml` for a complete multi-stage pipeline with matrix builds.

## Configuration

### Environment Variables for CI

```bash
# Required: Point to your shared NativeLink scheduler
export OBJFS_REMOTE_ENDPOINT="http://build-cluster:50051"
export OBJFS_REMOTE_INSTANCE="main"

# Recommended for CI:
export OBJFS_NO_AUTO_WORKER="1"    # Don't start local worker in CI
export OBJFS_MIN_REMOTE_SIZE="1"   # Cache everything for maximum benefit
```

Set these as repository secrets or CI/CD variables:
- **GitHub**: Repository Settings -> Secrets and variables -> Actions
- **GitLab**: Project Settings -> CI/CD -> Variables

## Performance

Measured results from real Rust projects:

**moor project (3 sequential builds):**
- Traditional: 25m 5s total
- With objfs: 11m 45s total (2.1x faster)

**Cache warm-up progression:**
- Build 1: All cache misses (cold)
- Build 2: ~80% cache hits (warm)
- Build 3: ~90% cache hits (hot)

**Speedup by scenario:**
- Parallel PRs: 75% faster
- Monorepos: 3.6x faster
- Multi-platform builds: 1.7x faster
- Overall range: 2.1-3.8x

**GitHub Actions cost impact ($0.008/min):**
- Without objfs: $19.20/month
- With objfs: $5.16/month
- Savings: $14.04/month (73% reduction)

## Troubleshooting

### Cache not shared between runners

- Verify all runners use the same `OBJFS_REMOTE_ENDPOINT` and `OBJFS_REMOTE_INSTANCE`
- Confirm the NativeLink scheduler is accessible from CI runner network
- Check that `OBJFS_NO_AUTO_WORKER=1` is set (prevents runners from starting isolated workers)

### Connection problems

```bash
# Test connectivity from runner
curl -v http://build-cluster:50051/health

# Check firewall rules
# Port 50051 (gRPC) must be accessible from CI runner IPs
```

### No cache hits on first build

This is expected. The first build on a cold cache will be all misses. Subsequent builds benefit from the shared cache. Artifacts persist in the NativeLink CAS across builds and runners.

### objfs wrapper not found

Ensure the install step completed successfully and the binary is in PATH before the build step runs. Verify with:
```bash
which cargo-objfs-rustc
cargo-objfs-rustc --version
```
