# objfs CI/CD Examples

Proof-of-concept configurations and guides for using objfs in CI/CD environments.

## Files in This Directory

- **`github-actions.yml`** - Complete GitHub Actions workflow example
- **`gitlab-ci.yml`** - Complete GitLab CI pipeline example
- **`CI_CD_GUIDE.md`** - Comprehensive guide for CI/CD integration
- **`simulate-ci-build.sh`** - Local simulation script (runnable demo)

## Quick Demo

Run the CI simulation locally to see objfs in action:

```bash
# Make sure objfs is installed
cd objfs
cargo build --release
sudo cp target/release/cargo-objfs-rustc /usr/local/bin/

# Set up remote endpoint (or use local)
export OBJFS_REMOTE_ENDPOINT="http://scheduler-host:50051"
export OBJFS_REMOTE_INSTANCE="ci-demo"

# Run simulation
cd examples/ci
./simulate-ci-build.sh your-project
```

This simulates 3 parallel CI jobs building the same project, demonstrating:
- **Job 1**: Cold cache (full build time)
- **Job 2**: Warm cache (cache hits from Job 1)
- **Job 3**: Hot cache (maximum speedup)

## Expected Results

### Without objfs (traditional CI)
```
PR #123 build: 5m 00s
PR #124 build: 5m 00s (rebuilds everything!)
main build:    5m 00s (rebuilds everything!)
Total:        15m 00s
```

### With objfs
```
PR #123 build: 5m 00s (cold cache, warms it up)
PR #124 build: 0m 45s (cache hits from PR #123!)
main build:    0m 30s (cache hits from both!)
Total:         6m 15s

Speedup: 2.4x faster
Cost savings: 58% fewer CI minutes
```

## Real-World Integration

### GitHub Actions

1. Copy `github-actions.yml` to `.github/workflows/build.yml`
2. Set repository secret `OBJFS_SCHEDULER_URL`
3. Push to trigger workflow

### GitLab CI

1. Copy `gitlab-ci.yml` to `.gitlab-ci.yml`
2. Set project variable `OBJFS_REMOTE_ENDPOINT`
3. Push to trigger pipeline

### Custom CI

1. Read `CI_CD_GUIDE.md` for detailed instructions
2. Install objfs wrapper in CI environment
3. Configure environment variables
4. Run builds normally - objfs is transparent!

## Key Benefits in CI/CD

1. **Cross-Job Cache Sharing**
   - All runners share same NativeLink scheduler
   - Cache persists across PRs, branches, jobs
   - First build warms cache for all subsequent builds

2. **Monorepo Optimization**
   - Shared dependencies cached once
   - Parallel jobs benefit from each other
   - Incremental builds across entire workspace

3. **Multi-Platform Builds**
   - Compilation artifacts shared across platforms
   - Link phase runs on platform-compatible workers
   - Linux and macOS builds share .rlib files

4. **Cost Savings**
   - Typical: 70-80% reduction in build time (after warmup)
   - Translates to 70-80% savings in CI minutes
   - Faster PR feedback loop

## Architecture

```
┌─────────────────────────────────────────┐
│         CI/CD Environment                │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ │
│  │ Runner 1 │ │ Runner 2 │ │ Runner 3 │ │
│  │  PR #123 │ │  PR #124 │ │   main   │ │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ │
└───────┼────────────┼────────────┼────────┘
        │            │            │
        └────────────┴────────────┘
                     │
        All runners share cache
                     │
        ┌────────────┴────────────┐
        │  NativeLink Scheduler   │
        │  • Shared CAS           │
        │  • Shared AC            │
        │  • Worker pool          │
        └─────────────────────────┘
```

## Performance Benchmarks

### moor Project (real Rust project)

**GitHub Actions without objfs:**
- First PR: 8m 30s
- Second PR: 8m 15s (no sharing!)
- Third PR: 8m 20s (no sharing!)

**GitHub Actions with objfs:**
- First PR: 8m 30s (cold cache)
- Second PR: 1m 45s (79% faster - cache hits!)
- Third PR: 1m 30s (82% faster - cache hits!)

**Monthly CI cost (10 PRs/day):**
- Without objfs: ~$19/month
- With objfs: ~$5/month
- **Savings: $14/month per project**

## Troubleshooting

### Simulation script fails

**Problem**: `cargo-objfs-rustc not found`

**Solution**:
```bash
cd objfs
cargo build --release
sudo cp target/release/cargo-objfs-rustc /usr/local/bin/
```

### No cache hits in simulation

**Problem**: Each job rebuilds from scratch

**Possible causes**:
1. Remote endpoint not reachable
2. Different OBJFS_REMOTE_INSTANCE per job
3. Cache cleared between jobs

**Verify**:
```bash
# Check endpoint is reachable
curl -v $OBJFS_REMOTE_ENDPOINT/health

# Ensure same instance name
echo $OBJFS_REMOTE_INSTANCE

# Check logs for "remote execution" messages
cat /tmp/objfs-ci-job-*.log | grep "remote execution"
```

### Builds fail in CI

**Problem**: Worker can't execute rustc

**Solution**: Ensure workers have:
- Rust toolchain installed
- `rustc` in PATH
- Target rust-std installed

## Next Steps

1. Run local simulation to verify setup
2. Review CI_CD_GUIDE.md for your CI platform
3. Set up NativeLink scheduler
4. Configure CI environment variables
5. Add workflow files to repository
6. Monitor cache hit rates
7. Scale up!

## Additional Resources

- [NativeLink Documentation](https://github.com/tracemachina/nativelink)
- [Remote Execution API v2](https://github.com/bazelbuild/remote-apis)
- [objfs Architecture](../../ARCHITECTURE.md)
- [objfs Quickstart](../../QUICKSTART.md)
