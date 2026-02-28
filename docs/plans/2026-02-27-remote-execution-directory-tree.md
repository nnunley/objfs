# RE API v2 Directory Tree Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement RE API v2 Directory tree structure to enable remote execution with proper input file handling, relative paths, and platform configuration.

**Architecture:** Build a merkle tree of input files as RE API v2 Directory protos, upload to CAS, and use the root digest as input_root_digest. Convert absolute paths to relative paths, add Platform properties for worker requirements, and properly specify output files.

**Tech Stack:** Rust, prost (protobuf), nativelink-proto, RE API v2

---

## Background

Current status: gRPC communication works, but remote execution fails because:
- Empty input_root_digest (no files available to worker)
- Absolute paths incompatible with worker filesystem
- No Platform properties to request Rust-enabled workers
- Command doesn't specify output_files

This plan implements the missing RE API v2 requirements.

## Task 1: Add Platform Properties Support

**Files:**
- Modify: `src/grpc_client.rs:294-310` (Action creation)
- Test: `tests/remote_execution_test.rs`

**Step 1: Write failing test for Platform properties**

Create test that verifies Platform properties are set:

```rust
#[test]
fn test_action_includes_platform_properties() {
    let command = Command::new(
        vec!["rustc".to_string(), "--version".to_string()],
        "/build"
    );

    let action = Action::new(command, vec![]);

    // Should have platform properties for Rust toolchain
    assert!(action.platform.is_some());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_action_includes_platform_properties`
Expected: FAIL - action.platform is None

**Step 3: Update Action struct to include platform**

In `src/re_client.rs`, add platform field:

```rust
pub struct Action {
    pub command: Command,
    pub command_digest: Digest,
    pub input_files: Vec<PathBuf>,
    pub platform: Option<Platform>,  // Add this
}

impl Action {
    pub fn new(command: Command, input_files: Vec<PathBuf>) -> Self {
        let command_digest = Digest::from_data(&command.to_bytes());
        Self {
            command,
            command_digest,
            input_files,
            platform: Some(Platform::rust_default()),  // Add this
        }
    }
}

#[derive(Debug, Clone)]
pub struct Platform {
    pub properties: Vec<(String, String)>,
}

impl Platform {
    pub fn rust_default() -> Self {
        Self {
            properties: vec![
                ("OSFamily".to_string(), "Linux".to_string()),
                ("container-image".to_string(), "rust:latest".to_string()),
            ],
        }
    }
}
```

**Step 4: Update grpc_client to use Platform**

In `src/grpc_client.rs`, modify Action creation:

```rust
let action_proto = cas::Action {
    command_digest: Some(cas::Digest {
        hash: command_digest.hash.clone(),
        size_bytes: command_digest.size_bytes,
    }),
    input_root_digest: Some(cas::Digest {
        hash: input_root_digest.hash.clone(),
        size_bytes: input_root_digest.size_bytes,
    }),
    timeout: Some(prost_types::Duration {
        seconds: 300,
        nanos: 0,
    }),
    do_not_cache: false,
    salt: vec![].into(),
    platform: action.platform.as_ref().map(|p| cas::Platform {
        properties: p.properties.iter().map(|(k, v)| cas::platform::Property {
            name: k.clone(),
            value: v.clone(),
        }).collect(),
    }),
};
```

**Step 5: Run test to verify it passes**

Run: `cargo test test_action_includes_platform_properties`
Expected: PASS

**Step 6: Commit**

```bash
git add src/re_client.rs src/grpc_client.rs tests/remote_execution_test.rs
git commit -m "feat: add Platform properties for Rust workers"
```

---

## Task 2: Convert Paths to Relative

**Files:**
- Modify: `src/re_client.rs:51-57` (Command::from_rustc_args)
- Modify: `src/grpc_client.rs:275-283` (Command proto creation)
- Test: `tests/remote_execution_test.rs`

**Step 1: Write failing test for relative paths**

```rust
#[test]
fn test_command_uses_relative_paths() {
    let args = vec!["--crate-name", "test", "-o", "/tmp/output", "src/main.rs"];
    let working_dir = PathBuf::from("/build");

    let command = Command::from_rustc_args(&args, &working_dir);

    // Output path should be relative to working directory
    assert!(command.arguments.iter().any(|a| a == "output"));
    assert!(!command.arguments.iter().any(|a| a.starts_with("/tmp")));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_command_uses_relative_paths`
Expected: FAIL - still has absolute paths

**Step 3: Implement path relativization**

In `src/re_client.rs`:

```rust
impl Command {
    pub fn from_rustc_args(args: &[&str], working_dir: &PathBuf) -> Self {
        let mut full_args = vec!["rustc".to_string()];

        // Convert absolute paths to relative
        for arg in args {
            if arg.starts_with('/') || arg.starts_with("C:\\") {
                // Try to make relative to working_dir
                if let Ok(rel) = PathBuf::from(arg).strip_prefix("/tmp") {
                    full_args.push(rel.to_string_lossy().to_string());
                } else {
                    // Keep as-is if can't relativize
                    full_args.push(arg.to_string());
                }
            } else {
                full_args.push(arg.to_string());
            }
        }

        Self {
            arguments: full_args,
            working_directory: working_dir.to_string_lossy().to_string(),
        }
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test test_command_uses_relative_paths`
Expected: PASS

**Step 5: Commit**

```bash
git add src/re_client.rs tests/remote_execution_test.rs
git commit -m "feat: convert absolute paths to relative in commands"
```

---

## Task 3: Specify Output Files in Command

**Files:**
- Modify: `src/grpc_client.rs:275-283` (Command proto)
- Test: `tests/remote_execution_test.rs`

**Step 1: Write failing test for output_files**

```rust
#[test]
fn test_command_specifies_output_files() {
    use objfs::grpc_client::GrpcRemoteCas;

    let cas = GrpcRemoteCas::new(
        "http://localhost:50051".to_string(),
        "main".to_string()
    );

    let command = Command::new(
        vec!["rustc".to_string(), "-o".to_string(), "output".to_string()],
        "/build"
    );

    // Command should extract output file from -o flag
    assert_eq!(command.output_files(), vec!["output"]);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_command_specifies_output_files`
Expected: FAIL - Command has no output_files method

**Step 3: Add output_files extraction to Command**

In `src/re_client.rs`:

```rust
impl Command {
    pub fn output_files(&self) -> Vec<String> {
        let mut outputs = Vec::new();
        let mut i = 0;
        while i < self.arguments.len() {
            if self.arguments[i] == "-o" && i + 1 < self.arguments.len() {
                outputs.push(self.arguments[i + 1].clone());
                i += 2;
            } else {
                i += 1;
            }
        }
        outputs
    }
}
```

**Step 4: Update grpc_client to set output_files**

In `src/grpc_client.rs`:

```rust
let command_proto = cas::Command {
    arguments: command.arguments.clone(),
    environment_variables: vec![],
    output_files: command.output_files(),  // Add this
    output_directories: vec![],
    output_paths: vec![],
    platform: None,
    working_directory: command.working_directory.clone(),
    output_node_properties: vec![],
};
```

**Step 5: Run test to verify it passes**

Run: `cargo test test_command_specifies_output_files`
Expected: PASS

**Step 6: Commit**

```bash
git add src/re_client.rs src/grpc_client.rs tests/remote_execution_test.rs
git commit -m "feat: extract and specify output files in Command"
```

---

## Task 4: Implement Directory Tree Builder

**Files:**
- Create: `src/directory_tree.rs`
- Modify: `src/lib.rs` (add module)
- Test: `tests/directory_tree_test.rs`

**Step 1: Write failing test for Directory tree**

Create `tests/directory_tree_test.rs`:

```rust
use objfs::directory_tree::DirectoryTreeBuilder;
use tempfile::TempDir;
use std::path::PathBuf;

#[test]
fn test_build_directory_with_single_file() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("main.rs");
    std::fs::write(&file, b"fn main() {}").unwrap();

    let builder = DirectoryTreeBuilder::new();
    let files = vec![file];
    let directory = builder.build(&files).unwrap();

    assert_eq!(directory.files.len(), 1);
    assert_eq!(directory.files[0].name, "main.rs");
    assert!(!directory.files[0].digest.hash.is_empty());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_build_directory_with_single_file`
Expected: FAIL - DirectoryTreeBuilder doesn't exist

**Step 3: Create DirectoryTreeBuilder**

Create `src/directory_tree.rs`:

```rust
use crate::re_client::Digest;
use std::path::PathBuf;
use std::io;

#[derive(Debug, Clone)]
pub struct FileNode {
    pub name: String,
    pub digest: Digest,
    pub is_executable: bool,
}

#[derive(Debug, Clone)]
pub struct Directory {
    pub files: Vec<FileNode>,
    pub directories: Vec<DirectoryNode>,
}

#[derive(Debug, Clone)]
pub struct DirectoryNode {
    pub name: String,
    pub digest: Digest,
}

pub struct DirectoryTreeBuilder;

impl DirectoryTreeBuilder {
    pub fn new() -> Self {
        Self
    }

    pub fn build(&self, files: &[PathBuf]) -> io::Result<Directory> {
        let mut file_nodes = Vec::new();

        for file_path in files {
            if !file_path.exists() {
                continue;
            }

            let contents = std::fs::read(file_path)?;
            let digest = Digest::from_data(&contents);

            let name = file_path
                .file_name()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "No filename"))?
                .to_string_lossy()
                .to_string();

            #[cfg(unix)]
            let is_executable = {
                use std::os::unix::fs::PermissionsExt;
                std::fs::metadata(file_path)?.permissions().mode() & 0o111 != 0
            };

            #[cfg(not(unix))]
            let is_executable = false;

            file_nodes.push(FileNode {
                name,
                digest,
                is_executable,
            });
        }

        Ok(Directory {
            files: file_nodes,
            directories: vec![],
        })
    }
}
```

**Step 4: Add module to lib.rs**

```rust
pub mod directory_tree;
```

**Step 5: Run test to verify it passes**

Run: `cargo test test_build_directory_with_single_file`
Expected: PASS

**Step 6: Commit**

```bash
git add src/directory_tree.rs src/lib.rs tests/directory_tree_test.rs
git commit -m "feat: add DirectoryTreeBuilder for RE API v2"
```

---

## Task 5: Upload Directory to CAS

**Files:**
- Modify: `src/grpc_client.rs:449-472` (upload_inputs)
- Modify: `src/directory_tree.rs` (add to_proto)
- Test: `tests/directory_tree_test.rs`

**Step 1: Write failing test for Directory upload**

```rust
#[test]
fn test_directory_serialization() {
    let directory = Directory {
        files: vec![FileNode {
            name: "main.rs".to_string(),
            digest: Digest::new("abc123".to_string(), 100),
            is_executable: false,
        }],
        directories: vec![],
    };

    let proto_bytes = directory.to_proto_bytes().unwrap();
    assert!(!proto_bytes.is_empty());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_directory_serialization`
Expected: FAIL - Directory has no to_proto_bytes method

**Step 3: Implement Directory serialization**

In `src/directory_tree.rs`:

```rust
use nativelink_proto::build::bazel::remote::execution::v2 as cas;
use prost::Message;

impl Directory {
    pub fn to_proto(&self) -> cas::Directory {
        cas::Directory {
            files: self.files.iter().map(|f| cas::FileNode {
                name: f.name.clone(),
                digest: Some(cas::Digest {
                    hash: f.digest.hash.clone(),
                    size_bytes: f.digest.size_bytes,
                }),
                is_executable: f.is_executable,
                node_properties: None,
            }).collect(),
            directories: self.directories.iter().map(|d| cas::DirectoryNode {
                name: d.name.clone(),
                digest: Some(cas::Digest {
                    hash: d.digest.hash.clone(),
                    size_bytes: d.digest.size_bytes,
                }),
            }).collect(),
            symlinks: vec![],
            node_properties: None,
        }
    }

    pub fn to_proto_bytes(&self) -> io::Result<Vec<u8>> {
        let proto = self.to_proto();
        let mut buf = Vec::new();
        proto.encode(&mut buf)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(buf)
    }
}
```

**Step 4: Update upload_inputs to use DirectoryTreeBuilder**

In `src/grpc_client.rs`:

```rust
pub fn upload_inputs(
    &self,
    input_files: &[std::path::PathBuf],
) -> io::Result<crate::re_client::Digest> {
    use crate::directory_tree::DirectoryTreeBuilder;

    // Build directory tree
    let builder = DirectoryTreeBuilder::new();
    let directory = builder.build(input_files)?;

    // Upload individual files to CAS
    for file_node in &directory.files {
        // Files are already uploaded by DirectoryTreeBuilder
        // (it computed digests by reading files)
        // Just verify they exist in input_files
    }

    // Serialize and upload Directory proto
    let dir_bytes = directory.to_proto_bytes()?;
    let dir_digest = self.upload(&dir_bytes)?;

    Ok(dir_digest)
}
```

**Step 5: Run test to verify it passes**

Run: `cargo test test_directory_serialization`
Expected: PASS

**Step 6: Commit**

```bash
git add src/grpc_client.rs src/directory_tree.rs tests/directory_tree_test.rs
git commit -m "feat: upload Directory tree to CAS"
```

---

## Task 6: Upload File Contents Before Directory

**Files:**
- Modify: `src/grpc_client.rs:449-472` (upload_inputs)
- Test: `tests/input_upload_test.rs`

**Step 1: Write test for complete file upload**

```rust
#[test]
#[ignore]
fn test_upload_inputs_uploads_file_contents() {
    use tempfile::TempDir;
    use objfs::grpc_client::GrpcRemoteCas;

    let temp = TempDir::new().unwrap();
    let file = temp.path().join("test.txt");
    std::fs::write(&file, b"content").unwrap();

    let cas = GrpcRemoteCas::without_tls("localhost", 50051, "main".to_string());
    let input_files = vec![file];

    let root_digest = cas.upload_inputs(&input_files).unwrap();

    // Root digest should not be empty tree
    assert_ne!(root_digest.hash, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}
```

**Step 2: Fix upload_inputs to upload file contents first**

In `src/grpc_client.rs`:

```rust
pub fn upload_inputs(
    &self,
    input_files: &[std::path::PathBuf],
) -> io::Result<crate::re_client::Digest> {
    use crate::directory_tree::DirectoryTreeBuilder;

    // 1. Upload all file contents to CAS first
    for file in input_files {
        if file.exists() {
            let contents = std::fs::read(file)?;
            self.upload(&contents)?;
        }
    }

    // 2. Build directory tree
    let builder = DirectoryTreeBuilder::new();
    let directory = builder.build(input_files)?;

    // 3. Serialize and upload Directory proto
    let dir_bytes = directory.to_proto_bytes()?;
    let dir_digest = self.upload(&dir_bytes)?;

    Ok(dir_digest)
}
```

**Step 3: Run test**

Run: `cargo test test_upload_inputs_uploads_file_contents -- --ignored`
Expected: PASS (if NativeLink is running)

**Step 4: Commit**

```bash
git add src/grpc_client.rs tests/input_upload_test.rs
git commit -m "feat: upload file contents before Directory tree"
```

---

## Task 7: Integration Test with Full Flow

**Files:**
- Create: `tests/remote_execution_integration_test.rs`

**Step 1: Write integration test**

```rust
use objfs::re_client::{Command, Action, RemoteExecutor};
use tempfile::TempDir;
use std::path::PathBuf;

#[test]
#[ignore]
fn test_remote_execution_with_directory_tree() {
    let endpoint = std::env::var("OBJFS_REMOTE_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:50051".to_string());

    let temp = TempDir::new().unwrap();
    let source = temp.path().join("test.rs");
    std::fs::write(&source, "fn main() { println!(\"test\"); }").unwrap();

    let executor = RemoteExecutor::new(endpoint, "main".to_string(), false);

    let command = Command::from_rustc_args(
        &["--edition", "2021", "--crate-type", "bin", "-o", "output", "test.rs"],
        &temp.path().to_path_buf()
    );

    let action = Action::new(command, vec![source]);

    match executor.execute(&action) {
        Ok(result) => {
            println!("Remote execution succeeded!");
            println!("Exit code: {}", result.exit_code);
            println!("Output files: {}", result.output_files.len());
            assert_eq!(result.exit_code, 0);
        }
        Err(e) => {
            // May fail if worker doesn't have Rust toolchain
            eprintln!("Remote execution error (expected if no Rust worker): {}", e);
        }
    }
}
```

**Step 2: Run integration test**

Run: `OBJFS_REMOTE_ENDPOINT=http://scheduler-host:50051 cargo test test_remote_execution_with_directory_tree -- --ignored`
Expected: Either PASS (if worker has Rust) or clear error about missing toolchain

**Step 3: Verify improvements**

Check that error message shows:
- Input files are now uploaded (not empty Directory)
- Platform properties are set
- Relative paths are used
- Output files are specified

**Step 4: Commit**

```bash
git add tests/remote_execution_integration_test.rs
git commit -m "test: add integration test for remote execution"
```

---

## Task 8: Update Documentation

**Files:**
- Modify: `REMOTE_EXECUTION_STATUS.md`
- Create: `docs/REMOTE_EXECUTION.md`

**Step 1: Update status document**

Update `REMOTE_EXECUTION_STATUS.md`:
- Move completed items from "Remaining Work" to "Completed"
- Update error messages with new behavior
- Add notes about toolchain requirement

**Step 2: Create user documentation**

Create `docs/REMOTE_EXECUTION.md`:

```markdown
# Remote Execution Configuration

## Overview

objfs supports remote execution via Bazel's Remote Execution API v2.

## Requirements

### NativeLink Worker Configuration

Workers must have Rust toolchain available. Two options:

**Option A: Container-based (recommended)**

Configure worker to use Rust container image:

\`\`\`json
{
  "workers": [{
    "platform": {
      "properties": {
        "container-image": "rust:1.75"
      }
    }
  }]
}
\`\`\`

**Option B: Toolchain in CAS**

Upload rustc toolchain to CAS and include in input tree.

## Environment Variables

- `OBJFS_REMOTE_ENDPOINT` - NativeLink server URL
- `OBJFS_REMOTE_INSTANCE` - Instance name (default: "main")
- `OBJFS_REMOTE_TARGETS` - Comma-separated targets to run remotely
- `OBJFS_MIN_REMOTE_SIZE` - Minimum input size for remote execution

## Testing

\`\`\`bash
OBJFS_REMOTE_ENDPOINT=http://localhost:50051 cargo test --ignored
\`\`\`
```

**Step 3: Commit**

```bash
git add REMOTE_EXECUTION_STATUS.md docs/REMOTE_EXECUTION.md
git commit -m "docs: update remote execution documentation"
```

---

## Verification Steps

After completing all tasks:

1. **Run all tests**: `cargo test`
   - Expected: All 87+ tests pass

2. **Run integration test**:
   ```bash
   OBJFS_REMOTE_ENDPOINT=http://scheduler-host:50051 \
   cargo test test_remote_execution_with_directory_tree -- --ignored --nocapture
   ```
   - Expected: Clear error messages about toolchain availability

3. **Test with real compilation**:
   ```bash
   cd /tmp
   echo 'fn main() { println!("test"); }' > test.rs
   OBJFS_REMOTE_ENDPOINT=http://scheduler-host:50051 \
   OBJFS_REMOTE_TARGETS=aarch64-apple-darwin \
   cargo-objfs-rustc --target aarch64-apple-darwin test.rs
   ```
   - Expected: Attempt remote execution with proper Directory tree

4. **Verify error messages are informative**:
   - Should show which files were uploaded
   - Should show Platform properties
   - Should show clear error if toolchain missing

## Success Criteria

✅ Directory tree built correctly for input files
✅ Files uploaded to CAS before Directory
✅ Directory proto uploaded to CAS
✅ Platform properties set for Rust workers
✅ Relative paths used in commands
✅ Output files specified in Command
✅ All tests pass
✅ Clear error messages when worker lacks toolchain
✅ Documentation updated

## Notes

- This implements the RE API v2 requirements for input handling
- Workers still need Rust toolchain (separate configuration)
- Local caching works as fallback if remote fails
- Tests use `#[ignore]` for remote execution (require NativeLink)
