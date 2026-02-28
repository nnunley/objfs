# Automatic Worker Registration

## Overview

objfs now automatically registers the local machine as a worker in the NativeLink cluster. Every machine running objfs contributes to the shared build capacity.

## How It Works

```
┌─────────────────────────────────────────────┐
│ Machine A (Dev's Laptop)                    │
├─────────────────────────────────────────────┤
│ cargo-objfs-rustc                           │
│   ├─ Checks if local worker running         │
│   ├─ If not: starts embedded worker         │
│   │   └─ Worker connects to scheduler       │
│   └─ Sends build request to scheduler       │
└─────────────────────────────────────────────┘
                    │
                    ↓ gRPC
┌─────────────────────────────────────────────┐
│ Scheduler (scheduler-host:50051)                 │
├─────────────────────────────────────────────┤
│ Receives build request                      │
│ Checks registered workers:                  │
│  • Machine A (darwin/aarch64) - AVAILABLE   │
│  • Machine B (linux/x86-64) - AVAILABLE     │
│  • Machine C (darwin/aarch64) - AVAILABLE   │
│ Selects best worker based on:              │
│  • Platform match                           │
│  • Load balancing                           │
│  • Network proximity                        │
└─────────────────────────────────────────────┘
                    │
                    ↓ Assigned to Machine B
┌─────────────────────────────────────────────┐
│ Machine B (Build Server)                    │
├─────────────────────────────────────────────┤
│ Local worker executes build                 │
│ Uploads artifacts to CAS                    │
│ Returns result to scheduler                 │
└─────────────────────────────────────────────┘
```

## Configuration

### Minimal Setup (Auto-Worker)

```bash
# Just point to a scheduler - worker auto-registers
export OBJFS_REMOTE_ENDPOINT="http://scheduler-host:50051"
export OBJFS_REMOTE_INSTANCE="main"

# Build anything - local machine auto-contributes
cargo build
```

objfs will:
1. Check if `nativelink` binary is available
2. Generate a minimal worker config
3. Start worker process in background
4. Worker auto-registers with scheduler
5. Participate in builds

### Advanced Configuration

Override auto-worker behavior:

```bash
# Disable auto-worker (client-only mode)
export OBJFS_NO_AUTO_WORKER=1

# Use specific worker port
export OBJFS_WORKER_PORT=50062

# Custom worker cache location
export OBJFS_WORKER_CACHE="/path/to/cache"
```

## Worker Config Generation

The auto-generated worker config:

```json5
{
  stores: [
    {
      name: "CAS_MAIN",
      grpc: {
        instance_name: "main",
        endpoints: [{ uri: "http://scheduler-host:50051" }],  // Scheduler CAS
        store_type: "cas",
      },
    },
    {
      name: "WORKER_FAST",
      filesystem: {
        content_path: "~/.cache/objfs/worker/fast/content",
        temp_path: "~/.cache/objfs/worker/fast/tmp",
        eviction_policy: { max_bytes: 5368709120 },  // 5 GiB local cache
      },
    },
    {
      name: "WORKER_CAS_FAST_SLOW",
      fast_slow: {
        fast: { ref_store: { name: "WORKER_FAST" } },
        slow: { ref_store: { name: "CAS_MAIN" } },      // Remote CAS
      },
    },
  ],

  workers: [{
    local: {
      worker_api_endpoint: {
        uri: "grpc://scheduler-host:50051/worker",  // Scheduler worker API
      },
      cas_fast_slow_store: "WORKER_CAS_FAST_SLOW",
      upload_action_result: { ac_store: "CAS_MAIN" },
      work_directory: "~/.cache/objfs/worker/work",
      platform_properties: {
        OSFamily: { values: ["darwin"] },      // Auto-detected
        ISA: { values: ["aarch64"] },          // Auto-detected
        "container-image": { values: ["rust:latest"] },
      },
    },
  }],

  servers: [],  // Worker-only, no server endpoints
}
```

## Benefits

✅ **Zero-config clustering** - Every dev machine contributes automatically
✅ **Dynamic capacity** - Worker pool grows as devs join builds
✅ **Platform diversity** - Mix of Linux/macOS/Windows workers
✅ **Load balancing** - Scheduler distributes work optimally
✅ **Shared cache** - All workers benefit from shared CAS/AC
✅ **Fault tolerance** - Workers come and go gracefully

## Examples

### Scenario 1: Personal Dev Machine

```bash
# Developer on Mac
export OBJFS_REMOTE_ENDPOINT="http://build-cluster:50051"

# First cargo build:
# - Auto-starts local worker
# - Registers as darwin/aarch64 worker
# - Participates in cluster
# - Builds can execute locally OR on other workers

cargo build --release
```

### Scenario 2: Build Server

```bash
# Dedicated Linux build server
export OBJFS_REMOTE_ENDPOINT="http://build-cluster:50051"

# Run as daemon
while true; do
  cargo-objfs-rustc --version  # Keeps worker alive
  sleep 3600
done
```

### Scenario 3: CI/CD Pipeline

```yaml
# .github/workflows/build.yml
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Setup objfs
        run: |
          export OBJFS_REMOTE_ENDPOINT="http://build-cluster:50051"
          cargo build --release
        # CI runner auto-registers as worker, contributes to build
```

## Monitoring

Check active workers:

```bash
# On scheduler machine
nativelink-admin list-workers

# Output:
# Worker ID: abc123
# Platform: darwin/aarch64
# Status: ACTIVE
# Location: 10.0.1.10
# Capacity: 8 cores
#
# Worker ID: def456
# Platform: linux/x86-64
# Status: ACTIVE
# Location: 10.0.1.11
# Capacity: 16 cores
```

## Security Considerations

**Worker Authentication:**
- Workers connect to scheduler's worker API endpoint
- Consider mTLS for production deployments
- Network isolation (VPN/private network recommended)

**Resource Limits:**
- Auto-workers use 5 GiB local cache by default
- Work directory cleaned after builds
- No persistent storage of source code

**Firewall Rules:**
- Scheduler needs port 50051 (client API) and 50061 (worker API) open
- Workers only need outbound connections to scheduler
- No inbound ports required on worker machines

## Troubleshooting

**Worker won't start:**
```bash
# Check if nativelink binary is in PATH
which nativelink

# Manual worker start for debugging
nativelink ~/.cache/objfs/worker-config.json5
```

**Worker not registered:**
```bash
# Check worker logs
tail -f ~/.cache/objfs/worker.log

# Verify connectivity to scheduler
curl -v http://scheduler-host:50051/health
```

**Worker consuming too much disk:**
```bash
# Reduce cache size
export OBJFS_WORKER_CACHE_SIZE=2147483648  # 2 GiB

# Or disable auto-worker
export OBJFS_NO_AUTO_WORKER=1
```

## Implementation Details

The auto-worker feature:
1. Runs in `cargo-objfs-rustc` before each build
2. Checks if local worker already running (port 50062)
3. If not, generates config and spawns `nativelink` process
4. Worker process persists for duration of build
5. Multiple builds reuse same worker (if still running)
6. Worker exits when no builds active (idle timeout)

Cache location:
- **macOS**: `~/Library/Caches/objfs/worker/`
- **Linux**: `~/.cache/objfs/worker/`
- **Windows**: `%LOCALAPPDATA%\objfs\worker\`
