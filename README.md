# objfs - Object Filesystem for Build Artifacts

**Status:** Early prototype demonstrating transparent build caching for Rust

## What is this?

objfs is a transparent build artifact cache for Cargo/Rust that requires **zero build configuration**. It intercepts rustc invocations and stores compilation outputs in a content-addressed store (CAS), enabling:

- Instant clean rebuilds (artifacts served from cache)
- Deduplication across projects (same source = same artifact, stored once)
- Future: Distributed CAS across team members
- Future: Remote execution via NativeLink

## Architecture

```
Developer runs: cargo build
         ↓
rustc-wrapper intercepts compilation
         ↓
Check CAS for cached result (by input hash)
    ├─ Hit:  Restore from CAS (instant)
    └─ Miss: Compile normally, store in CAS
         ↓
Cargo sees successful compilation
```

## Current Implementation

### Core Components

1. **CAS (`src/cas.rs`)** - Content-addressed storage
   - Git-style object storage (first 2 chars as directory shard)
   - SHA256 hashing of all artifacts
   - Automatic deduplication
   - Stats tracking (object count, total size)

2. **Rustc Wrapper (`src/bin/rustc_wrapper.rs`)** - Transparent interception
   - Invoked by Cargo via `rustc-wrapper` config
   - Passes through metadata queries (`--version`, `--print`)
   - Hashes input files + compilation flags for cache key
   - Stores/retrieves artifacts from CAS

3. **CLI Tool (`src/bin/cli.rs`)** - Management interface
   - `objfs enable` - Add rustc-wrapper to project
   - `objfs stats` - Show CAS statistics
   - `objfs clear` - Clear all cached objects

## Installation

```bash
cd objfs
cargo install --path .
```

This installs two binaries to `~/.cargo/bin/`:
- `objfs` - CLI tool
- `cargo-objfs-rustc` - Rustc wrapper

## Usage

```bash
# In any Rust project
cd ~/my-project
objfs enable  # Creates .cargo/config.toml with rustc-wrapper

# Build as normal - caching is automatic
cargo build

# Check cache stats
objfs stats

# Clear cache
objfs clear

# Disable for a project
objfs disable
```

## What's Working

✅ CAS implementation with deduplication
✅ Rustc wrapper installation
✅ Metadata query passthrough
✅ Per-project enable/disable
✅ Cache statistics

## What Needs Work

🔧 **Output File Detection** - Currently guesses output paths, needs to parse:
   - `--emit=dep-info,link,metadata`
   - `--crate-type` (bin, lib, rlib, etc.)
   - Actual output filenames with hash suffixes

🔧 **Multiple Artifacts** - Handle all rustc outputs:
   - `.rlib` / `.so` / `.dylib` - Compiled libraries
   - `.d` - Dependency info
   - `.rmeta` - Metadata files
   - Incremental compilation artifacts

🔧 **Cache Key Stability** - Ensure hash is stable across:
   - Different absolute paths
   - Normalized compiler flags
   - Deterministic input ordering

## Future Roadmap

### Phase 1: Local CAS (Current)
- ✅ Basic caching infrastructure
- 🔧 Robust output file handling
- ⬜ LRU eviction policy
- ⬜ Compression for large artifacts

### Phase 2: Distributed CAS
- ⬜ Peer discovery via mDNS
- ⬜ libp2p-based CAS synchronization
- ⬜ Team cache sharing
- ⬜ Storage deduplication metrics

### Phase 3: Remote Execution
- ⬜ NativeLink integration
- ⬜ Remote build workers
- ⬜ Build result caching
- ⬜ Bandwidth optimization

### Phase 4: FUSE Integration (git-virtual)
- ⬜ Virtual filesystem for source code
- ⬜ Lazy git clone with sparse checkout
- ⬜ Multi-forge support (GitHub, Gitea, Codeberg)
- ⬜ Automatic Gitea mirroring for LAN cache
- ⬜ Overlay mount for build artifacts
- ⬜ Git safety (prevent adding build artifacts)

## Design Goals

1. **Zero Configuration** - Works with standard Cargo projects
2. **Transparent** - Build tools don't need to know it exists
3. **Opt-in** - Enable per-project, disable anytime
4. **Safe** - Never breaks builds (worst case: cache miss = normal compile)
5. **Rust-native** - Pure Rust, no Python/Java dependencies

## Comparison to Alternatives

| Feature | objfs | Bazel/Buck2 | sccache |
|---------|-------|-------------|---------|
| Cargo compat | ✅ Zero config | ❌ Requires BUILD files | ✅ Wrapper |
| Language support | Rust (expandable) | Multi-language | C/C++/Rust |
| Remote exec | 🔧 Planned | ✅ Built-in | ❌ Cache only |
| Distributed CAS | 🔧 Planned | ✅ Built-in | ⬜ Limited |
| Learning curve | None | Steep | Low |

## Contributing

This is an early prototype exploring transparent build caching. Areas for contribution:

1. **Rustc integration** - Better output file detection
2. **Cache verification** - Test cache hit/miss rates
3. **Performance testing** - Benchmark on real projects
4. **Documentation** - Usage patterns, troubleshooting

## License

MIT (to be added)

## Acknowledgments

Inspired by:
- Google's srcfs (sparse git VFS)
- Microsoft's Scalar/VFS for Git
- Bazel's remote execution
- sccache's compiler caching
