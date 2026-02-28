# objfs Architecture

## High-Level Overview

objfs is a distributed build cache and remote execution system for Rust, built on NativeLink's Remote Execution API v2.

```
┌──────────────────────────────────────────────────────────────┐
│                     Developer Machines                        │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐             │
│  │ Dev Mac #1 │  │ Dev Mac #2 │  │ CI Runner  │             │
│  │            │  │            │  │            │             │
│  │ cargo-     │  │ cargo-     │  │ cargo-     │             │
│  │ objfs-rustc│  │ objfs-rustc│  │ objfs-rustc│             │
│  │     │      │  │     │      │  │     │      │             │
│  │     └──────┼──┼─────┘      │  │     └──────┼────────┐    │
│  └────────────┘  └────────────┘  └────────────┘        │    │
└──────────────────────────────────────────────────────────┼────┘
                                                           │
                            gRPC (RE API v2)              │
                                   │                      │
┌──────────────────────────────────┼──────────────────────┼────┐
│                    NativeLink Scheduler                  │    │
│  ┌──────────────────────────────────────────────────────┴──┐ │
│  │ • Receives build requests                               │ │
│  │ • Manages worker pool                                   │ │
│  │ • Distributes work based on platform/load               │ │
│  │ • Shared CAS (Content-Addressable Storage)              │ │
│  │ • Shared AC (Action Cache)                              │ │
│  └─────────────────────────────────────────────────────────┘ │
│                              │                               │
│                ┌─────────────┼─────────────┐                │
└────────────────┼─────────────┼─────────────┼────────────────┘
                 │             │             │
          Worker API     Worker API    Worker API
                 │             │             │
┌────────────────┴──┐ ┌────────┴────────┐ ┌─┴─────────────────┐
│ Worker: Mac #1    │ │ Worker: Mac #2  │ │ Worker: CI Runner │
│ darwin/aarch64    │ │ darwin/aarch64  │ │ linux/x86-64      │
│ • Auto-started    │ │ • Auto-started  │ │ • Auto-started    │
│ • Local cache     │ │ • Local cache   │ │ • Local cache     │
│ • Executes builds │ │ • Executes      │ │ • Executes builds │
└───────────────────┘ └─────────builds───┘ └───────────────────┘
```

## Components

### 1. cargo-objfs-rustc

Transparent wrapper around `rustc` that intercepts compilation requests.

**Responsibilities:**
- Parse rustc command-line arguments
- Compute cache keys from input files
- Check local cache first
- If miss, try remote execution
- If remote fails, fall back to local compilation
- Auto-start local worker if needed

**Flow:**
```rust
fn main() {
    // 1. Ensure local worker is running
    ensure_local_worker()?;

    // 2. Parse rustc args
    let build = parse_rustc_args(&args);

    // 3. Check cache
    if let Some(artifacts) = check_cache(&build) {
        return install_artifacts(artifacts);
    }

    // 4. Try remote execution
    if remote_enabled() {
        match try_remote_execution(&build) {
            Ok(artifacts) => return install_artifacts(artifacts),
            Err(_) => {} // Fall through to local
        }
    }

    // 5. Execute locally
    exec_rustc(&args)
}
```

### 2. Local Worker (Auto-Started)

Embedded NativeLink worker that auto-registers with the scheduler.

**Lifecycle:**
1. `cargo-objfs-rustc` starts
2. Checks if worker running (port 50062)
3. If not, generates config and spawns `nativelink`
4. Worker connects to scheduler
5. Announces platform capabilities
6. Participates in build cluster

**Configuration:**
- Generated on-the-fly in `~/.cache/objfs/worker-config.json5`
- Uses scheduler's CAS (no local CAS replication)
- 5 GiB local cache for hot artifacts
- Auto-detects platform (darwin/aarch64, linux/x86-64, etc.)

### 3. NativeLink Scheduler

Central coordinator that manages distributed builds.

**Responsibilities:**
- Accept client build requests
- Maintain worker pool registry
- Match builds to capable workers
- Manage Content-Addressable Storage (CAS)
- Manage Action Cache (AC)
- Handle retries and failover

**Platform Matching:**
```
Client requests: rustc --target aarch64-apple-darwin
    ↓
Scheduler checks workers:
    • Worker A: darwin/aarch64 ✅ MATCH
    • Worker B: linux/x86-64  ❌ NO MATCH
    • Worker C: darwin/x86_64 ❌ NO MATCH
    ↓
Assigns to Worker A
```

### 4. Content-Addressable Storage (CAS)

Immutable storage for build artifacts, indexed by SHA256 hash.

**Structure:**
```
CAS/
├── ab/cd/ef/...  (SHA256 prefix directories)
│   └── abcdef123...456  (content blob)
├── 12/34/56/...
│   └── 123456789...abc
...
```

**Properties:**
- Deduplication: Same content = same hash = stored once
- Immutable: Content never changes after upload
- Distributed: All workers read from shared CAS
- Efficient: Only transfer missing blobs

### 5. Action Cache (AC)

Maps build commands to their outputs.

**Structure:**
```json
{
  "action_digest": "sha256:abc123...",
  "result": {
    "output_files": [
      {"path": "lib.rlib", "digest": "sha256:def456..."}
    ],
    "exit_code": 0,
    "execution_metadata": {
      "worker": "darwin/aarch64",
      "execution_time": "2.3s"
    }
  }
}
```

**Benefits:**
- Instant cache hits across entire cluster
- No redundant builds (even across machines)
- Persistent across sessions

## Data Flow

### Remote Execution (Cache Miss)

```
1. Developer runs: cargo build
       ↓
2. cargo-objfs-rustc intercepts rustc invocation
       ↓
3. Computes action digest: SHA256(command + inputs)
       ↓
4. Queries AC: GET /ac/sha256:abc123
       ↓ (miss)
5. Uploads input files to CAS
       ↓
6. Sends Execute request to scheduler
       ↓
7. Scheduler selects compatible worker
       ↓
8. Worker downloads inputs from CAS
       ↓
9. Worker executes: rustc --target aarch64-apple-darwin ...
       ↓
10. Worker uploads outputs to CAS
       ↓
11. Worker returns result (exit code + output digests)
       ↓
12. Scheduler caches result in AC
       ↓
13. Client downloads outputs from CAS
       ↓
14. Installs outputs to local filesystem
```

### Cache Hit

```
1. Developer runs: cargo build
       ↓
2. cargo-objfs-rustc intercepts rustc invocation
       ↓
3. Computes action digest: SHA256(command + inputs)
       ↓
4. Queries AC: GET /ac/sha256:abc123
       ↓ (hit!)
5. Downloads outputs from CAS
       ↓
6. Installs outputs to local filesystem

Total time: ~100ms (vs 2-10s for compilation)
```

## Configuration

### Environment Variables

| Variable | Purpose | Default |
|----------|---------|---------|
| `OBJFS_REMOTE_ENDPOINT` | Scheduler URL | `http://localhost:50051` (if available) |
| `OBJFS_REMOTE_INSTANCE` | Instance name | `main` |
| `OBJFS_REMOTE_TARGETS` | Worker capabilities | Host's target triple |
| `OBJFS_MIN_REMOTE_SIZE` | Minimum file size for remote | 100 KB |
| `OBJFS_NO_AUTO_WORKER` | Disable auto-worker | (unset) |
| `OBJFS_DISABLE` | Disable objfs entirely | (unset) |

### Typical Setups

**Single Developer (Local Caching):**
```bash
# No config needed - auto-detects localhost worker
cargo build
```

**Team with Shared Scheduler:**
```bash
export OBJFS_REMOTE_ENDPOINT="http://build-cluster.company.com:50051"
cargo build
```

**CI/CD:**
```yaml
env:
  OBJFS_REMOTE_ENDPOINT: "http://build-cluster:50051"
  OBJFS_MIN_REMOTE_SIZE: "1"  # Cache everything
```

## Platform Properties

Workers announce their capabilities via platform properties:

```json
{
  "OSFamily": "darwin",       // Host OS (not target OS)
  "ISA": "aarch64",          // Instruction set
  "container-image": "rust:latest"
}
```

Scheduler matches these to build requirements:
- Darwin builds → Darwin workers
- Linux builds → Linux workers
- Cross-compilation works if worker has toolchain

## Security

**Network Security:**
- mTLS for production (scheduler ↔ worker)
- VPN/private network recommended
- No inbound ports on workers (outbound only)

**Data Security:**
- CAS content is immutable (tampering detectable via hash)
- AC results include verification metadata
- Workers sandboxed (no access to other builds)

**Resource Limits:**
- Per-worker cache limits (default 5 GiB)
- Per-build timeouts (default 30 seconds)
- Max concurrent builds per worker

## Performance Characteristics

**Cache Hit:**
- Latency: ~100ms
- Network: Download artifacts only (typically <10 MB)
- CPU: Minimal

**Cache Miss (Remote):**
- Latency: ~2-10s (compilation time)
- Network: Upload inputs + download outputs
- CPU: On remote worker (not local)

**Cache Miss (Local Fallback):**
- Latency: Normal rustc compile time
- Network: None
- CPU: On local machine

## Future Enhancements

1. **Smart worker selection** - Prefer workers with hot cache
2. **Speculative execution** - Run on multiple workers, use fastest result
3. **Incremental compilation** - Cache individual functions/modules
4. **Build analytics** - Track cache hit rates, worker utilization
5. **Cross-platform builds** - Linux workers with osxcross for Darwin targets
