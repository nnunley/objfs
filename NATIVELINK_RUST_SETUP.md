# Setting Up NativeLink Worker with Rust Toolchain

This guide shows how to configure a NativeLink worker to compile Rust code remotely.

## Option 1: Using ADDITIONAL_SETUP_WORKER_CMD (Recommended)

This method installs Rust directly in the NativeLink worker container.

### 1. Set Environment Variable

Create a `.env` file in `nativelink/deployment-examples/docker-compose/`:

```bash
# Install Rust toolchain in worker container
ADDITIONAL_SETUP_WORKER_CMD="curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && \
  . \$HOME/.cargo/env && \
  rustup target add aarch64-apple-darwin && \
  rustup target add x86_64-apple-darwin"
```

### 2. Update worker.json5

Edit `nativelink/deployment-examples/docker-compose/worker.json5`:

```json5
platform_properties: {
  cpu_count: {
    query_cmd: "nproc",
  },
  OSFamily: {
    values: [
      "Linux",  // Changed from ""
    ],
  },
  "container-image": {
    values: [
      "rust:latest",  // Changed from ""
    ],
  },
  ISA: {
    values: [
      "x86-64",
    ],
  },
},
```

### 3. Rebuild and Restart

```bash
cd nativelink/deployment-examples/docker-compose
docker-compose down
docker-compose build --no-cache nativelink_executor
docker-compose up -d
```

### 4. Verify Installation

Check that Rust is installed in the worker:

```bash
docker-compose exec nativelink_executor bash -c ". ~/.cargo/env && rustc --version"
```

Expected output:
```
rustc 1.75.0 (or later)
```

## Option 2: Custom Dockerfile (More Control)

Create a custom Dockerfile based on NativeLink's:

### 1. Create Dockerfile.rust

```dockerfile
# Start from NativeLink's base
FROM trace_machina/nativelink:latest

# Install Rust toolchain
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Install macOS cross-compilation targets
RUN rustup target add aarch64-apple-darwin && \
    rustup target add x86_64-apple-darwin

# Install osxcross for actual macOS linking (optional, complex)
# This requires macOS SDK which has licensing restrictions
# For now, we can only compile, not link macOS binaries on Linux

EXPOSE 50051/tcp 50052/tcp
CMD ["nativelink"]
```

### 2. Update docker-compose.yml

```yaml
nativelink_executor:
  build:
    context: .
    dockerfile: Dockerfile.rust
  # ... rest of configuration
```

## Option 3: Use Official Rust Docker Image (Simplest)

Modify `docker-compose.yml` to use Rust base image:

```yaml
nativelink_executor:
  image: rust:latest  # Use official Rust image instead
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

**Note**: This option installs NativeLink on every container start, which is slow. Better to build a custom image.

## Testing Remote Compilation

Once the worker is configured, test with objfs:

```bash
cd /tmp
cat > test.rs << 'EOF'
fn main() {
    println!("Hello from remote execution!");
}
EOF

# Configure objfs to use remote execution
export OBJFS_REMOTE_ENDPOINT="http://scheduler-host:50051"
export OBJFS_REMOTE_INSTANCE="main"
export OBJFS_REMOTE_TARGETS="aarch64-apple-darwin"
export OBJFS_MIN_REMOTE_SIZE=1

# Compile remotely
objfs/target/release/cargo-objfs-rustc rustc \
  --target aarch64-apple-darwin \
  --crate-name test \
  --edition 2021 \
  --crate-type bin \
  -o test_bin \
  test.rs
```

### Expected Output (Success)

```
[objfs] remote execution: target=aarch64-apple-darwin, size=60 bytes
[objfs] remote execution succeeded
[objfs] cached bundle: 1 files -> abc123
```

### Expected Output (Before Configuration)

```
[objfs] remote execution: target=aarch64-apple-darwin, size=60 bytes
[objfs] remote execution failed: Remote execution timed out after 30 seconds.
Worker may be missing required toolchain (rustc). Check NativeLink worker
configuration., falling back to local
[objfs] cache miss: test_bin
```

## Cross-Compilation Limitations

### What Works
- ✅ Compiling Rust code on Linux for Linux targets
- ✅ Compiling Rust code (rustc) for macOS targets
- ✅ Generating `.rlib` files for macOS

### What Doesn't Work (Without Extra Setup)
- ❌ **Linking** final macOS binaries on Linux (requires macOS SDK + osxcross)
- ❌ Creating macOS executables (needs Apple linker)

### Workarounds

**For Libraries (`.rlib`)**: Works perfectly! Just compile with `--crate-type lib`.

**For Binaries**: You need osxcross:
1. Install osxcross in the worker (requires macOS SDK - legal gray area)
2. Set `CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER` to osxcross linker
3. Very complex setup, not recommended

**Better Approach**: Use remote execution for library compilation only, link locally.

## Recommended Configuration

For objfs use case (caching Rust compilation):

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
  // Add these for Rust-specific capabilities:
  "rust-version": { values: ["1.75.0"] },
  "rust-targets": { values: ["aarch64-apple-darwin", "x86_64-apple-darwin"] },
}
```

## Troubleshooting

### Worker doesn't start
```bash
docker-compose logs nativelink_executor
```

### Rust not found in worker
```bash
docker-compose exec nativelink_executor which rustc
docker-compose exec nativelink_executor bash -c ". ~/.cargo/env && rustc --version"
```

### Remote execution still times out
1. Check worker logs: `docker-compose logs -f nativelink_executor`
2. Verify platform properties match in worker.json5
3. Ensure OBJFS_REMOTE_TARGETS includes your target
4. Test rustc directly in container:
   ```bash
   docker-compose exec nativelink_executor bash -c ". ~/.cargo/env && rustc --version"
   ```

## Next Steps

Once configured:
1. Test with simple Rust file (shown above)
2. Test with moor project: `cargo build` with RUSTC_WRAPPER
3. Monitor cache hit rates in objfs
4. Tune worker count for parallelism

## References

- NativeLink docs: https://github.com/TraceMachina/nativelink
- Rustup installation: https://rustup.rs/
- Docker Compose: https://docs.docker.com/compose/
