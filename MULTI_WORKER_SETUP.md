# Multi-Worker NativeLink Setup

## Architecture

Instead of running separate NativeLink instances, run one scheduler that manages multiple workers:

```
objfs client (Mac)
    ↓ gRPC
Scheduler (can run anywhere)
    ├→ Worker 1: Mac (localhost for Darwin builds)
    └→ Worker 2: Linux (scheduler-host for cross-platform builds)
```

## Option A: Scheduler on Mac, Workers Connect to It

Run the scheduler on your Mac, and have remote workers connect to it.

### Mac Configuration (Scheduler + Local Worker)

`/usr/local/etc/nativelink/config.json5`:

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
      name: "MULTI_SCHEDULER",
      simple: {
        supported_platform_properties: {
          OSFamily: "exact",    // Matches worker platform
          ISA: "exact",
          "container-image": "exact",
        },
        max_job_retries: 3,
      },
    },
  ],

  // LOCAL WORKER (Mac)
  workers: [{
    local: {
      worker_api_endpoint: {
        uri: "grpc://127.0.0.1:50062",  // Local worker endpoint
      },
      cas_fast_slow_store: "WORKER_CAS_FAST_SLOW",
      upload_action_result: {
        ac_store: "AC_MAIN",
      },
      work_directory: "$HOME/.local/share/nativelink/work",
      platform_properties: {
        OSFamily: {
          values: ["darwin"],  // Mac can build Darwin targets
        },
        ISA: {
          values: ["aarch64"],  // ARM64
        },
        "container-image": {
          values: ["rust:latest"],
        },
      },
    },
  }],

  servers: [
    // Client-facing endpoint (scheduler)
    {
      listener: {
        http: {
          socket_address: "0.0.0.0:50051",  // Listen on all interfaces for remote clients
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
            scheduler: "MULTI_SCHEDULER",
          },
        },
        capabilities: {
          main: {
            remote_execution: {
              scheduler: "MULTI_SCHEDULER",
            },
          },
        },
        bytestream: {
          cas_stores: {
            main: "CAS_MAIN",
          },
        },
      },
    },
    // Worker API endpoint (for remote workers to connect)
    {
      listener: {
        http: {
          socket_address: "0.0.0.0:50061",  // Remote workers connect here
        },
      },
      services: {
        worker_api: {
          scheduler: "MULTI_SCHEDULER",
        },
      },
    },
    // Local worker endpoint
    {
      listener: {
        http: {
          socket_address: "127.0.0.1:50062",
        },
      },
      services: {
        worker_api: {
          scheduler: "MULTI_SCHEDULER",
        },
      },
    },
  ],
}
```

### Linux Worker Configuration (Remote Worker Mode)

On the Linux container, configure it to connect to the Mac scheduler:

`/etc/nativelink/config.json5`:

```json5
{
  stores: [
    {
      name: "CAS_MAIN",
      grpc: {
        instance_name: "main",
        endpoints: [
          {
            uri: "grpc://10.0.1.1:50051",  // Mac's IP
          },
        ],
        store_type: "cas",
      },
    },
    {
      name: "WORKER_FAST",
      filesystem: {
        content_path: "/var/lib/nativelink/worker-fast/content",
        temp_path: "/var/lib/nativelink/worker-fast/tmp",
        eviction_policy: {
          max_bytes: 10737418240,  // 10 GiB
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
            name: "CAS_MAIN",  // Remote CAS on Mac
          },
        },
      },
    },
  ],

  schedulers: [],  // No scheduler, this is just a worker

  workers: [{
    local: {
      worker_api_endpoint: {
        uri: "grpc://10.0.1.1:50061",  // Connect to Mac's worker API
      },
      cas_fast_slow_store: "WORKER_CAS_FAST_SLOW",
      upload_action_result: {
        ac_store: "CAS_MAIN",  // Upload results to Mac's AC
      },
      work_directory: "/var/lib/nativelink/work",
      platform_properties: {
        OSFamily: {
          values: ["linux"],  // Linux worker
        },
        ISA: {
          values: ["x86-64"],
        },
        "container-image": {
          values: ["rust:latest"],
        },
      },
    },
  }],

  servers: [],  // Worker-only, no server endpoints
}
```

## Option B: Scheduler on Linux, Mac Worker Connects

Alternatively, keep the scheduler on the Linux container and have your Mac connect as a worker.

## Usage

With this setup:

```bash
# objfs connects to the scheduler (Mac in Option A)
export OBJFS_REMOTE_ENDPOINT="http://localhost:50051"  # or Mac's IP from other machines
export OBJFS_REMOTE_INSTANCE="main"

# Scheduler will automatically:
# - Send Darwin builds to Mac worker
# - Send Linux builds to Linux worker
# - Use whichever worker is available and matches platform
```

The scheduler handles work distribution automatically based on platform properties!

## Benefits

✅ **Automatic worker selection** - Scheduler picks the right worker
✅ **Load balancing** - Multiple workers for same platform can share work
✅ **Unified CAS** - All workers share the same content-addressable storage
✅ **Shared cache** - AC (Action Cache) benefits all workers
✅ **Fault tolerance** - If one worker fails, others can continue

## Testing

```bash
# Compile for Darwin - should use Mac worker
cargo build --target aarch64-apple-darwin

# Compile for Linux - should use Linux worker
cargo build --target x86_64-unknown-linux-gnu
```

Check NativeLink logs to see which worker handled each build:
```bash
# Mac:
tail -f $HOME/.local/share/nativelink/nativelink.log

# Linux:
incus exec nativelink-worker -- journalctl -u nativelink -f
```
