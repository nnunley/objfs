# Replace nativelink-proto with Self-Generated Protos

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove the `nativelink-proto` path dependency so objfs can be installed via `cargo install --git` and eventually published to crates.io.

**Architecture:** Fetch upstream `.proto` source files from `bazelbuild/remote-apis` and `googleapis/googleapis`, compile them with `tonic-build` in `build.rs`, and update the two consuming files (`grpc_client.rs`, `directory_tree.rs`) to use the new generated module paths. Remove the entire `proto/` directory (nativelink's vendored copy) and replace it with a minimal set of only the proto files objfs actually needs. Update Cargo.toml metadata for distribution.

**Tech Stack:** tonic-build 0.13, prost 0.13, protobuf, Remote Execution API v2

---

### Task 1: Fetch upstream proto source files

**Files:**
- Delete: `proto/` (entire directory — nativelink's vendored protos)
- Create: `proto/build/bazel/remote/execution/v2/remote_execution.proto`
- Create: `proto/build/bazel/semver/semver.proto`
- Create: `proto/google/longrunning/operations.proto`
- Create: `proto/google/rpc/status.proto`
- Create: `proto/google/api/annotations.proto`
- Create: `proto/google/api/http.proto`
- Create: `proto/google/api/client.proto`
- Create: `proto/google/api/field_behavior.proto`
- Create: `proto/google/api/launch_stage.proto`

These are the transitive closure of imports required by `remote_execution.proto` and `operations.proto`. The well-known types (`google/protobuf/*`) are bundled with prost and do not need vendoring.

**Step 1: Delete nativelink's vendored proto directory**

```bash
rm -rf proto/
```

**Step 2: Download upstream proto files**

From `bazelbuild/remote-apis` (tag `v2.2.0` or latest on main):
```bash
mkdir -p proto/build/bazel/remote/execution/v2
mkdir -p proto/build/bazel/semver
curl -sL 'https://raw.githubusercontent.com/bazelbuild/remote-apis/main/build/bazel/remote/execution/v2/remote_execution.proto' \
  -o proto/build/bazel/remote/execution/v2/remote_execution.proto
curl -sL 'https://raw.githubusercontent.com/bazelbuild/remote-apis/main/build/bazel/semver/semver.proto' \
  -o proto/build/bazel/semver/semver.proto
```

From `googleapis/googleapis`:
```bash
mkdir -p proto/google/longrunning proto/google/rpc proto/google/api
curl -sL 'https://raw.githubusercontent.com/googleapis/googleapis/master/google/longrunning/operations.proto' \
  -o proto/google/longrunning/operations.proto
curl -sL 'https://raw.githubusercontent.com/googleapis/googleapis/master/google/rpc/status.proto' \
  -o proto/google/rpc/status.proto
curl -sL 'https://raw.githubusercontent.com/googleapis/googleapis/master/google/api/annotations.proto' \
  -o proto/google/api/annotations.proto
curl -sL 'https://raw.githubusercontent.com/googleapis/googleapis/master/google/api/http.proto' \
  -o proto/google/api/http.proto
curl -sL 'https://raw.githubusercontent.com/googleapis/googleapis/master/google/api/client.proto' \
  -o proto/google/api/client.proto
curl -sL 'https://raw.githubusercontent.com/googleapis/googleapis/master/google/api/field_behavior.proto' \
  -o proto/google/api/field_behavior.proto
curl -sL 'https://raw.githubusercontent.com/googleapis/googleapis/master/google/api/launch_stage.proto' \
  -o proto/google/api/launch_stage.proto
```

**Step 3: Verify proto files downloaded**

```bash
find proto/ -name '*.proto' | sort
```

Expected output:
```
proto/build/bazel/remote/execution/v2/remote_execution.proto
proto/build/bazel/semver/semver.proto
proto/google/api/annotations.proto
proto/google/api/client.proto
proto/google/api/field_behavior.proto
proto/google/api/http.proto
proto/google/api/launch_stage.proto
proto/google/longrunning/operations.proto
proto/google/rpc/status.proto
```

**Step 4: Commit**

```bash
jj new -m 'feat: vendor upstream RE API v2 proto files

Replace nativelink-proto vendored protos with upstream sources from
bazelbuild/remote-apis and googleapis/googleapis.'
jj bookmark set main
```

---

### Task 2: Configure tonic-build in build.rs

**Files:**
- Modify: `Cargo.toml` (update tonic-build version, remove nativelink-proto)
- Modify: `build.rs`

**Step 1: Update Cargo.toml**

Remove the `nativelink-proto` dependency (line 21):
```
nativelink-proto = { path = "../nativelink/nativelink-proto" }
```

Update `tonic-build` from `0.12` to `0.13` (must match tonic version):
```toml
[build-dependencies]
tonic-build = "0.13"
```

Add package metadata for distribution:
```toml
[package]
name = "objfs"
version = "0.1.0"
edition = "2024"
description = "Distributed build cache using Remote Execution API v2"
license = "MIT"
repository = "https://github.com/nnunley/objfs"
readme = "README.md"
keywords = ["build-cache", "remote-execution", "compilation", "rust"]
categories = ["development-tools::build-utils", "caching"]
```

**Step 2: Write build.rs**

Replace `build.rs` with:
```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(false)
        .compile_protos(
            &[
                "proto/build/bazel/remote/execution/v2/remote_execution.proto",
                "proto/google/longrunning/operations.proto",
            ],
            &["proto/"],
        )?;
    Ok(())
}
```

Key points:
- `build_server(false)` — we only need client stubs, not server
- Two proto files compiled; their transitive imports resolve via the `&["proto/"]` include path
- Generated code lands in `OUT_DIR` and gets `include!`'d at compile time

**Step 3: Verify it compiles**

```bash
cargo check 2>&1
```

Expected: compilation errors in `grpc_client.rs` and `directory_tree.rs` because they still import `nativelink_proto::`. That is correct — we fix those in Task 3.

**Step 4: Commit**

```bash
jj new -m 'build: configure tonic-build to compile RE API v2 protos

Remove nativelink-proto path dependency and generate proto types
directly from vendored upstream .proto files. Add Cargo.toml
metadata for distribution.'
jj bookmark set main
```

---

### Task 3: Create proto module and update imports

**Files:**
- Create: `src/proto.rs`
- Modify: `src/lib.rs` (add `pub mod proto;`)
- Modify: `src/grpc_client.rs` (change imports)
- Modify: `src/directory_tree.rs` (change imports)

**Step 1: Create `src/proto.rs`**

This module re-exports the tonic-build generated code:

```rust
pub mod google {
    pub mod longrunning {
        tonic::include_proto!("google.longrunning");
    }
    pub mod rpc {
        tonic::include_proto!("google.rpc");
    }
    pub mod api {
        // google.api types are pulled in transitively; no separate include needed
    }
}

pub mod build {
    pub mod bazel {
        pub mod remote {
            pub mod execution {
                pub mod v2 {
                    tonic::include_proto!("build.bazel.remote.execution.v2");
                }
            }
        }
    }
}
```

Note: `tonic::include_proto!` expands to `include!(concat!(env!("OUT_DIR"), "/build.bazel.remote.execution.v2.rs"))`. Each call brings in one generated `.rs` file.

**Step 2: Add module to lib.rs**

Add to `src/lib.rs`:
```rust
pub mod proto;
```

**Step 3: Update `src/grpc_client.rs` imports**

Replace line 7-8:
```rust
// Use NativeLink's proto definitions
use nativelink_proto::build::bazel::remote::execution::v2 as cas;
```
With:
```rust
use crate::proto::build::bazel::remote::execution::v2 as cas;
```

Replace line 390:
```rust
use nativelink_proto::google::longrunning::operation;
```
With:
```rust
use crate::proto::google::longrunning::operation;
```

Also update the file-level comment (line 2):
```rust
// Implements secure communication over gRPC with RE API v2 servers
```

**Step 4: Update `src/directory_tree.rs` imports**

Replace line 4:
```rust
use nativelink_proto::build::bazel::remote::execution::v2 as cas;
```
With:
```rust
use crate::proto::build::bazel::remote::execution::v2 as cas;
```

**Step 5: Verify compilation**

```bash
cargo check 2>&1
```

Expected: clean compilation with no errors.

If there are missing types, the proto module may need adjustment. Common issues:
- `google.rpc.Status` is pulled in transitively by longrunning — may not need separate include
- Nested enum/message paths may differ slightly between nativelink-proto and tonic-build output

**Step 6: Run tests**

```bash
cargo test 2>&1
```

Expected: all non-ignored tests pass. The ignored tests (`#[ignore]`) require a running RE API server and are expected to be skipped.

**Step 7: Commit**

```bash
jj new -m 'refactor: use self-generated proto types instead of nativelink-proto

Update grpc_client.rs and directory_tree.rs to import from
crate::proto instead of nativelink_proto. No behavioral changes.'
jj bookmark set main
```

---

### Task 4: Clean up empty src/proto directory

**Files:**
- Delete: `src/proto/` (empty directory left over from earlier work)

**Step 1: Remove empty directory**

```bash
rm -rf src/proto/
```

Verify `src/proto.rs` (the module file) still exists — that is the correct file.

**Step 2: Commit**

```bash
jj new -m 'chore: remove empty src/proto/ directory'
jj bookmark set main
```

---

### Task 5: Update documentation

**Files:**
- Modify: `README.md` (update description, add install instructions)
- Modify: `USAGE.md` (update install section)
- Modify: `src/grpc_client.rs:380` (update error message)
- Modify: `src/re_client.rs:3` (update comment)
- Modify: `build.rs` comment (already done in Task 2)
- Modify: `docs/reference/architecture.md:5` (update description)

**Step 1: Update code comments referencing NativeLink**

In `src/grpc_client.rs` line 380, change:
```
"Remote execution timed out after 30 seconds. Worker may be missing required toolchain (rustc). Check NativeLink worker configuration."
```
To:
```
"Remote execution timed out after 30 seconds. Worker may be missing required toolchain (rustc). Check worker configuration."
```

In `src/re_client.rs` line 3, the comment already says:
```
// like NativeLink, BuildBarn, etc.
```
This is accurate — leave it. NativeLink is one compatible server.

**Step 2: Update README.md**

Change line 3 from:
```
Distributed build cache for Rust using NativeLink's Remote Execution API.
```
To:
```
Distributed build cache for Rust using the Remote Execution API v2.
```

Add installation section after the existing usage section:
```markdown
## Installation

```bash
cargo install --git https://github.com/nnunley/objfs
```

This installs three binaries: `objfs`, `cargo-objfs-rustc`, and `objfs-cc-wrapper`.
```

**Step 3: Update USAGE.md install section**

If there are `sudo cp` instructions, replace them with `cargo install` instructions.

**Step 4: Update docs/reference/architecture.md line 5**

Change:
```
objfs is a distributed build cache and remote execution system for Rust, built on NativeLink's Remote Execution API v2.
```
To:
```
objfs is a distributed build cache and remote execution system for Rust, built on the Remote Execution API v2.
```

**Step 5: Commit**

```bash
jj new -m 'docs: update references to use generic RE API v2 terminology

NativeLink is one compatible server, not a hard dependency.
Update install instructions to use cargo install.'
jj bookmark set main
```

---

### Task 6: Final verification and push

**Step 1: Full test suite**

```bash
cargo test 2>&1
cargo check 2>&1
```

**Step 2: Verify cargo install works from local path**

```bash
cargo install --path . --force 2>&1
```

All three binaries should install:
- `objfs`
- `cargo-objfs-rustc`
- `objfs-cc-wrapper`

**Step 3: Verify no nativelink-proto references remain in source**

```bash
grep -r 'nativelink.proto\|nativelink_proto' src/ build.rs Cargo.toml
```

Expected: no output (zero matches).

**Step 4: Push**

```bash
jj git push
```

Push to both remotes if configured.
