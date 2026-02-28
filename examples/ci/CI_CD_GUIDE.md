# objfs in CI/CD Environments

Guide for using objfs distributed build caching in GitHub Actions, GitLab CI, and other CI/CD systems.

## Quick Start

**Three steps to add objfs to your CI pipeline:**

1. **Install objfs wrapper**
2. **Configure cargo to use wrapper**
3. **Set environment variables**

That's it! Builds automatically use distributed caching.

## Architecture in CI/CD

```
┌─────────────────────────────────────────────────────────┐
│              CI/CD Environment (GitHub/GitLab)           │
│                                                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ Runner 1     │  │ Runner 2     │  │ Runner 3     │  │
│  │ (PR #123)    │  │ (PR #124)    │  │ (main)       │  │
│  │              │  │              │  │              │  │
│  │ cargo build  │  │ cargo build  │  │ cargo build  │  │
│  │      ↓       │  │      ↓       │  │      ↓       │  │
│  │ cargo-objfs  │  │ cargo-objfs  │  │ cargo-objfs  │  │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  │
│         │                  │                  │          │
└─────────┼──────────────────┼──────────────────┼──────────┘
          │                  │                  │
          └──────────────────┴──────────────────┘
                             │
                    All runners share
                             │
          ┌──────────────────┴──────────────────┐
          │   NativeLink Scheduler              │
          │   build-cluster.company.com:50051   │
          │                                     │
          │   • Shared CAS (artifacts)          │
          │   • Shared AC (build cache)         │
          │   • Worker pool                     │
          └─────────────────────────────────────┘
```

## Benefits in CI/CD

### 1. Cross-Job Cache Sharing

**Traditional CI (no cache sharing):**
```
PR #123 builds: 5 minutes
PR #124 builds: 5 minutes (rebuilds same deps!)
main builds:    5 minutes (rebuilds same deps!)
Total: 15 minutes
```

**With objfs:**
```
PR #123 builds: 5 minutes (cold cache)
PR #124 builds: 30 seconds (cache hit!)
main builds:    20 seconds (cache hit!)
Total: 6 minutes (2.5x faster)
```

### 2. Monorepo Optimization

**Without objfs:**
- Each service rebuilds shared dependencies
- Parallel jobs duplicate work

**With objfs:**
- Shared dependencies cached once
- Parallel jobs benefit from each other's work
- Incremental builds across entire monorepo

### 3. Multi-Platform Builds

**Compilation phase**: Can run on any worker with rust-std
**Link phase**: Runs on platform-compatible workers

```yaml
jobs:
  build-linux:
    runs-on: ubuntu-latest
    # Compiles .rlib on any worker
    # Links on linux worker

  build-macos:
    runs-on: macos-latest
    # Reuses .rlib from linux build!
    # Links on macos worker
```

## Installation in CI

### GitHub Actions

```yaml
- name: Install objfs
  run: |
    curl -L https://github.com/yourorg/objfs/releases/latest/download/cargo-objfs-rustc-linux-x86_64 \
      -o /tmp/cargo-objfs-rustc
    chmod +x /tmp/cargo-objfs-rustc
    sudo mv /tmp/cargo-objfs-rustc /usr/local/bin/
```

### GitLab CI

```yaml
before_script:
  - curl -L https://github.com/yourorg/objfs/releases/latest/download/cargo-objfs-rustc-linux-x86_64 -o /tmp/cargo-objfs-rustc
  - chmod +x /tmp/cargo-objfs-rustc
  - sudo mv /tmp/cargo-objfs-rustc /usr/local/bin/
```

### Docker Image (Recommended)

Create a base image with objfs pre-installed:

```dockerfile
FROM rust:latest

# Install objfs
RUN curl -L https://github.com/yourorg/objfs/releases/latest/download/cargo-objfs-rustc-linux-x86_64 \
    -o /usr/local/bin/cargo-objfs-rustc && \
    chmod +x /usr/local/bin/cargo-objfs-rustc

# Configure cargo globally
RUN mkdir -p /usr/local/cargo && \
    echo '[build]' > /usr/local/cargo/config.toml && \
    echo 'rustc-wrapper = "/usr/local/bin/cargo-objfs-rustc"' >> /usr/local/cargo/config.toml

ENV CARGO_HOME=/usr/local/cargo
```

Then use in CI:

```yaml
jobs:
  build:
    image: yourorg/rust-objfs:latest
    # objfs already configured!
```

## Environment Variables

| Variable | Purpose | CI Recommendation |
|----------|---------|-------------------|
| `OBJFS_REMOTE_ENDPOINT` | Scheduler URL | Set in CI variables/secrets |
| `OBJFS_REMOTE_INSTANCE` | Instance name | Usually "main" or "ci" |
| `OBJFS_NO_AUTO_WORKER` | Skip local worker | Always "1" in CI |
| `OBJFS_MIN_REMOTE_SIZE` | Min size for remote | "1" for max caching |

### GitHub Actions Secrets

```yaml
env:
  OBJFS_REMOTE_ENDPOINT: ${{ secrets.OBJFS_SCHEDULER_URL }}
  OBJFS_REMOTE_INSTANCE: "ci"
  OBJFS_NO_AUTO_WORKER: "1"
  OBJFS_MIN_REMOTE_SIZE: "1"
```

### GitLab CI Variables

In GitLab project settings → CI/CD → Variables:
- `OBJFS_REMOTE_ENDPOINT`: `http://build-cluster:50051`
- `OBJFS_REMOTE_INSTANCE`: `ci`

Then reference in `.gitlab-ci.yml`:

```yaml
variables:
  OBJFS_REMOTE_ENDPOINT: $OBJFS_REMOTE_ENDPOINT
  OBJFS_REMOTE_INSTANCE: $OBJFS_REMOTE_INSTANCE
  OBJFS_NO_AUTO_WORKER: "1"
```

## Scheduler Setup for CI

### Dedicated CI Instance

Create a separate NativeLink instance for CI workloads:

```json5
{
  schedulers: [{
    name: "CI_SCHEDULER",
    simple: {
      supported_platform_properties: {
        OSFamily: "exact",
        ISA: "exact",
      },
      max_job_retries: 3,
    },
  }],
  // ...
}
```

Use `OBJFS_REMOTE_INSTANCE="ci"` to isolate CI cache from dev cache.

### Shared Instance

Or share with development:

```bash
export OBJFS_REMOTE_INSTANCE="main"
```

CI and dev builds benefit from each other's cache!

## Performance Benchmarks

### Real-World Example: moor Project

**Without objfs (GitHub Actions default):**
```
First PR build:  8m 30s
Second PR build: 8m 15s
Third PR build:  8m 20s
```

**With objfs:**
```
First PR build:  8m 30s (cold cache)
Second PR build: 1m 45s (cache hits)
Third PR build:  1m 30s (cache hits)
```

**Savings:**
- 80% reduction in build time after first build
- ~$15/month saved on CI minutes (GitHub Actions pricing)
- Faster feedback on PRs

### Monorepo Example

**5 microservices, shared dependencies:**

**Without objfs:**
- Each service: 5 minutes
- Total serial: 25 minutes
- Total parallel (5 runners): 5 minutes (but 5x compute cost)

**With objfs:**
- First service: 5 minutes
- Remaining 4: 30 seconds each (shared deps cached)
- Total serial: 7 minutes
- Total parallel: 1 minute (cache sharing between runners!)

## Best Practices

### 1. Use Docker Images with Pre-installed objfs

Faster runner startup, consistent configuration.

### 2. Set OBJFS_NO_AUTO_WORKER in CI

CI runners shouldn't start local workers - just clients.

### 3. Cache Everything (MIN_REMOTE_SIZE=1)

CI jobs are short-lived, maximize cache utilization.

### 4. Monitor Cache Hit Rates

Track objfs messages in build logs:
```
[objfs] cache hit: 87%
[objfs] remote execution: 13%
```

### 5. Separate CI and Dev Instances (Optional)

Isolate CI workloads from developer machines:
```bash
# Dev machines
OBJFS_REMOTE_INSTANCE="main"

# CI runners
OBJFS_REMOTE_INSTANCE="ci"
```

### 6. Disable GitLab/GitHub Built-in Caching

objfs provides superior caching:

```yaml
# GitHub Actions - don't need actions/cache
# objfs handles it better

# GitLab CI - set custom CARGO_HOME to avoid conflicts
variables:
  CARGO_HOME: "${CI_PROJECT_DIR}/.cargo"
```

## Troubleshooting

### "Connection refused" in CI

**Problem**: Runner can't reach scheduler.

**Solution**:
- Verify scheduler URL in CI environment
- Check firewall rules (CI IPs → scheduler)
- Ensure scheduler is publicly accessible or on CI VPN

### Slow First Build in CI

**Expected**: First build always cold cache.

**Optimize**:
- Run scheduled nightly builds to warm cache
- Trigger build on `main` after merges

### Cache Not Shared Between Jobs

**Problem**: Each job rebuilds from scratch.

**Verify**:
1. All jobs use same `OBJFS_REMOTE_ENDPOINT`
2. All jobs use same `OBJFS_REMOTE_INSTANCE`
3. Scheduler is actually running
4. Check logs for "cache hit" messages

## Example Workflows

See example workflow files:
- `examples/ci/github-actions.yml` - GitHub Actions
- `examples/ci/gitlab-ci.yml` - GitLab CI

## Cost Savings Calculator

**GitHub Actions pricing**: $0.008/minute

**Without objfs:**
- 10 PRs/day × 8 minutes = 80 minutes/day
- 80 × 30 days = 2400 minutes/month
- 2400 × $0.008 = **$19.20/month**

**With objfs (80% cache hit rate):**
- First build: 8 minutes
- Remaining 9: 1.6 minutes each = 14.4 minutes
- Total: 22.4 minutes/day
- 22.4 × 30 = 672 minutes/month
- 672 × $0.008 = **$5.38/month**

**Savings: $13.82/month per project** (72% cost reduction)

Plus faster PR feedback and developer happiness!

## Security Considerations

### Network Security

- Run scheduler on private network or VPN
- Use mTLS for production (NativeLink supports it)
- Restrict scheduler access to known CI IPs

### Cache Poisoning

- objfs uses content-addressable storage (SHA256)
- Tampering detectable via hash mismatch
- Separate CI instance (`OBJFS_REMOTE_INSTANCE="ci"`) for isolation

### Secrets in Build

- objfs doesn't cache environment variables
- Build commands hashed, not secrets
- Standard CI secret management applies

## Next Steps

1. Set up NativeLink scheduler (see `QUICKSTART.md`)
2. Install objfs on CI runners
3. Configure environment variables
4. Run test build
5. Monitor cache hit rates
6. Scale up!
