# Documentation Cleanup Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Consolidate 23 root markdown files into a clean docs/ structure with guides/ and reference/ subdirectories.

**Architecture:** Delete 10 stale status reports, consolidate 9 source files into 5 destination files across docs/guides/ and docs/reference/, move 2 files as-is, update README links.

**Tech Stack:** Markdown, git, jj

---

### Task 1: Create directory structure

**Files:**
- Create: `docs/guides/` directory
- Create: `docs/reference/` directory

**Step 1: Create the directories**

```bash
mkdir -p docs/guides docs/reference
```

**Step 2: Verify**

```bash
ls -la docs/
```

Expected: `guides/`, `reference/`, `plans/` directories present.

**Step 3: Commit**

```bash
jj commit -m "docs: Create guides/ and reference/ subdirectories"
```

---

### Task 2: Write docs/guides/quickstart.md

**Files:**
- Create: `docs/guides/quickstart.md`
- Source: `QUICKSTART.md` (move content, clean up)

**Step 1: Write quickstart.md**

Take the content from QUICKSTART.md. Changes to make:
- Update "Further Reading" links to point to new docs/ paths
- Remove the Further Reading section entirely (the docs/ structure makes navigation self-evident)
- Keep everything else — it's already well-written as a user guide

**Step 2: Verify the file exists and reads well**

```bash
wc -l docs/guides/quickstart.md
```

**Step 3: Commit**

```bash
jj commit -m "docs: Add quickstart guide"
```

---

### Task 3: Write docs/guides/remote-execution.md

**Files:**
- Create: `docs/guides/remote-execution.md`
- Source: `REMOTE_EXECUTION.md`

**Step 1: Write remote-execution.md**

Take content from REMOTE_EXECUTION.md. Changes to make:
- Remove the "Current Status" section at the bottom (lines 177-206) — this is a stale checklist
- Remove the "Testing" section that references specific test counts ("All 65 tests should pass")
- Keep: Architecture, Configuration, How It Works, Example Workflow, Platform Matching Rules, Hierarchical CAS, Troubleshooting
- Update any cross-references to other docs to use new paths

**Step 2: Commit**

```bash
jj commit -m "docs: Add remote execution guide"
```

---

### Task 4: Write docs/guides/worker-setup.md

**Files:**
- Create: `docs/guides/worker-setup.md`
- Sources: `MACOS_WORKER_SETUP.md`, `MULTI_WORKER_SETUP.md`, `NATIVELINK_RUST_SETUP.md`, `NATIVELINK_CROSS_COMPILE_SETUP.md`

**Step 1: Write worker-setup.md**

Consolidate all four worker docs into one with this structure:

```markdown
# NativeLink Worker Setup

## Overview
Brief intro: objfs uses NativeLink workers for remote execution. This guide
covers setting up workers on different platforms.

## macOS Worker
(From MACOS_WORKER_SETUP.md)
- Installation (Homebrew or source)
- Worker configuration (the json5 config)
- Launch daemon setup
- Testing
- Keep the "Advantages" section

## Multi-Worker Architecture
(From MULTI_WORKER_SETUP.md)
- Architecture diagram (scheduler + multiple workers)
- Option A: Scheduler on Mac (full config)
- Option B: Scheduler on Linux
- Linux worker configuration (remote worker mode)
- Testing and log checking

## Docker/Container Workers
(From NATIVELINK_RUST_SETUP.md)
- Option 1: ADDITIONAL_SETUP_WORKER_CMD (recommended)
- Option 2: Custom Dockerfile
- Option 3: Official Rust Docker image
- Testing remote compilation
- Cross-compilation limitations (what works, what doesn't)
- Recommended configuration

## Cross-Compilation from Linux
(From NATIVELINK_CROSS_COMPILE_SETUP.md)
- Problem statement
- Network configuration (incus proxy)
- Rust toolchain with macOS target (NixOS rust-overlay)
- Platform properties configuration
- WARNING: osxcross on NixOS is incompatible (keep this prominently)
- Alternative solutions: Ubuntu container, cargo-zigbuild, native macOS worker
- Lessons learned

## Troubleshooting
Consolidated troubleshooting from all four docs.
```

Drop from all sources:
- Status claims ("Implementation Complete", checkmark lists of what's done)
- "Next Steps" sections that are status tracking
- Duplicate content (e.g., the same json5 configs that appear in multiple docs — keep the most complete version)

**Step 2: Commit**

```bash
jj commit -m "docs: Add consolidated worker setup guide"
```

---

### Task 5: Write docs/guides/ci-cd.md

**Files:**
- Create: `docs/guides/ci-cd.md`
- Source: `CI_CD_INTEGRATION_SUMMARY.md`

**Step 1: Write ci-cd.md**

Extract the how-to content, drop the status report framing. Structure:

```markdown
# CI/CD Integration

## Overview
One paragraph: objfs speeds up CI builds by sharing cached artifacts
across runners.

## GitHub Actions
The 3-step integration (install, configure cargo, build).
Full workflow YAML.
Environment variables.

## GitLab CI
Reference to examples/ci/gitlab-ci.yml.

## Configuration
Environment variables for CI (OBJFS_NO_AUTO_WORKER=1, OBJFS_MIN_REMOTE_SIZE=1, etc.).
Why client-only mode matters in CI.

## Performance
Keep the measured results (2.1-3.8x, 73% cost reduction) as a brief section,
but frame as "measured performance" not "proof of concept validation".

## Troubleshooting
Cache not sharing across runners, connection issues, etc.
```

Drop:
- "Proven Results", "Success Criteria Met", "Conclusion" sections
- "Production Readiness" checklist
- "Real-World Usage Pattern" narrative
- "Example Metrics Dashboard"

**Step 2: Commit**

```bash
jj commit -m "docs: Add CI/CD integration guide"
```

---

### Task 6: Write docs/guides/c-cpp-integration.md

**Files:**
- Create: `docs/guides/c-cpp-integration.md`
- Sources: `C_CPP_INTEGRATION.md`, `docs/cmake-integration/CMAKE_STRATEGIES.md`

**Step 1: Write c-cpp-integration.md**

Merge both C/C++ docs. Structure:

```markdown
# C/C++ Integration

## Overview
objfs caches C/C++ compilation via objfs-cc-wrapper.

## Installation
Build and install the wrapper binary.

## Build System Integration

### CMake (Recommended: Compiler Launcher)
From CMAKE_STRATEGIES.md Strategy 1 — the recommended approach.
Include the CMakeLists.txt example, command-line usage, and toolchain file approach.

### Make
From C_CPP_INTEGRATION.md — CC/CXX variable approach.

### Autotools
From C_CPP_INTEGRATION.md — configure flags.

### Direct Usage
Wrapping individual compiler invocations.

## How It Works
Cache key computation, hit/miss flow.

## CMake Strategy Alternatives
Brief summary of strategies 2-4 from CMAKE_STRATEGIES.md
(compiler override, rule-based, RE API direct) with pros/cons.
Not recommended but documented for reference.

## Limitations
From C_CPP_INTEGRATION.md — no header tracking, no link caching, etc.
Include the ccache comparison table.

## Troubleshooting
Consolidated from both docs.

## Examples
Reference to examples/cmake-example/ and examples/makefile-example/.
```

Drop:
- "Implementation Checklist" (internal tracking)
- "Testing Plan" (internal)
- "Next Steps" (tracked in ROADMAP.md and git-issues)

**Step 2: Commit**

```bash
jj commit -m "docs: Add C/C++ integration guide"
```

---

### Task 7: Write docs/reference/architecture.md

**Files:**
- Create: `docs/reference/architecture.md`
- Source: `ARCHITECTURE.md`

**Step 1: Write architecture.md**

Move ARCHITECTURE.md content largely as-is. Changes:
- Update cross-references to point to new docs/ paths
- Remove "Future Enhancements" section (tracked in ROADMAP.md)
- Keep everything else — it's a solid technical reference

**Step 2: Commit**

```bash
jj commit -m "docs: Add architecture reference"
```

---

### Task 8: Write docs/reference/linking-strategy.md

**Files:**
- Create: `docs/reference/linking-strategy.md`
- Sources: `DISTRIBUTED_COMPILATION_LOCAL_LINKING.md`, `PLATFORM_COMPATIBLE_LINKING.md`

**Step 1: Write linking-strategy.md**

These two docs cover the same concept. Merge into one. Structure:

```markdown
# Linking Strategy

## Overview
objfs uses a hybrid approach: compile .rlib files on any worker,
link final binaries on platform-compatible workers.

## Compilation vs Linking
What can execute remotely (compilation) vs what needs platform matching (linking).
Use the "What Gets Distributed" table from DISTRIBUTED_COMPILATION doc.

## Platform Matching Rules
The table from PLATFORM_COMPATIBLE_LINKING showing target triple →
compatible workers → incompatible workers.
Key insight: architecture can differ, but OS must match.

## Decision Flow
The decision tree from PLATFORM_COMPATIBLE_LINKING (cleaner than the
DISTRIBUTED doc's version).

## Examples
Pick the best 2-3 examples from across both docs:
- Mac developer with mixed workers
- Mac developer with only Linux workers
- Linux CI building for both platforms

## Configuration
Automatic vs explicit OBJFS_REMOTE_TARGETS.

## Performance
The cold build parallel vs sequential comparison from DISTRIBUTED doc.

## Troubleshooting
Consolidated from both docs.
```

Drop:
- "Future Enhancements" sections from both docs
- "Summary" section (redundant with Overview)
- Duplicate content that appears in both docs

**Step 2: Commit**

```bash
jj commit -m "docs: Add linking strategy reference"
```

---

### Task 9: Write docs/reference/auto-worker-registration.md

**Files:**
- Create: `docs/reference/auto-worker-registration.md`
- Source: `AUTO_WORKER_REGISTRATION.md`

**Step 1: Write auto-worker-registration.md**

Move content largely as-is. Changes:
- Update cross-references
- Drop the "now" in "objfs now automatically registers" — just state the fact
- Keep: How It Works, Configuration, Worker Config Generation, Benefits,
  Examples, Monitoring, Security Considerations, Troubleshooting, Implementation Details

**Step 2: Commit**

```bash
jj commit -m "docs: Add auto-worker registration reference"
```

---

### Task 10: Delete stale status reports

**Files:**
- Delete: `COMPLETE.md`
- Delete: `FINAL_IMPLEMENTATION.md`
- Delete: `IMPLEMENTATION_STATUS.md`
- Delete: `TDD_SUMMARY.md`
- Delete: `TEST_SUMMARY.md`
- Delete: `REMOTE_EXECUTION_COMPLETE.md`
- Delete: `REMOTE_EXECUTION_STATUS.md`
- Delete: `INTEGRATION_TEST_RESULTS.md`
- Delete: `CMAKE_INTEGRATION_COMPLETE.md`
- Delete: `CI_CD_INTEGRATION_SUMMARY.md`

**Step 1: Delete the files**

```bash
rm COMPLETE.md FINAL_IMPLEMENTATION.md IMPLEMENTATION_STATUS.md \
   TDD_SUMMARY.md TEST_SUMMARY.md REMOTE_EXECUTION_COMPLETE.md \
   REMOTE_EXECUTION_STATUS.md INTEGRATION_TEST_RESULTS.md \
   CMAKE_INTEGRATION_COMPLETE.md CI_CD_INTEGRATION_SUMMARY.md
```

**Step 2: Verify only expected files remain in root**

```bash
ls *.md
```

Expected: `README.md`, `ROADMAP.md` only.

**Step 3: Commit**

```bash
jj commit -m "docs: Remove stale status reports (preserved in git history)"
```

---

### Task 11: Delete moved source files

**Files:**
- Delete: `QUICKSTART.md`
- Delete: `REMOTE_EXECUTION.md`
- Delete: `ARCHITECTURE.md`
- Delete: `AUTO_WORKER_REGISTRATION.md`
- Delete: `MACOS_WORKER_SETUP.md`
- Delete: `MULTI_WORKER_SETUP.md`
- Delete: `NATIVELINK_RUST_SETUP.md`
- Delete: `NATIVELINK_CROSS_COMPILE_SETUP.md`
- Delete: `DISTRIBUTED_COMPILATION_LOCAL_LINKING.md`
- Delete: `PLATFORM_COMPATIBLE_LINKING.md`
- Delete: `C_CPP_INTEGRATION.md`
- Delete: `docs/cmake-integration/CMAKE_STRATEGIES.md`
- Delete: `docs/cmake-integration/` directory

**Step 1: Delete the files**

```bash
rm QUICKSTART.md REMOTE_EXECUTION.md ARCHITECTURE.md \
   AUTO_WORKER_REGISTRATION.md MACOS_WORKER_SETUP.md \
   MULTI_WORKER_SETUP.md NATIVELINK_RUST_SETUP.md \
   NATIVELINK_CROSS_COMPILE_SETUP.md \
   DISTRIBUTED_COMPILATION_LOCAL_LINKING.md \
   PLATFORM_COMPATIBLE_LINKING.md C_CPP_INTEGRATION.md
rm -r docs/cmake-integration/
```

**Step 2: Verify**

```bash
ls *.md
ls docs/
```

Expected root: `README.md`, `ROADMAP.md`
Expected docs: `guides/`, `reference/`, `plans/`

**Step 3: Commit**

```bash
jj commit -m "docs: Remove source files moved to docs/"
```

---

### Task 12: Update README.md

**Files:**
- Modify: `README.md`

**Step 1: Update Documentation section**

Replace the current Documentation section with links to new paths:

```markdown
## Documentation

- [Quick Start](docs/guides/quickstart.md) - Installation and setup
- [Remote Execution](docs/guides/remote-execution.md) - Distributed builds
- [Worker Setup](docs/guides/worker-setup.md) - NativeLink worker configuration
- [CI/CD Integration](docs/guides/ci-cd.md) - GitHub Actions, GitLab CI
- [C/C++ Integration](docs/guides/c-cpp-integration.md) - CMake, Make builds
- [Architecture](docs/reference/architecture.md) - Technical details
- [Linking Strategy](docs/reference/linking-strategy.md) - Platform-compatible linking
- [Auto-Worker Registration](docs/reference/auto-worker-registration.md) - Zero-config clustering
- [Roadmap](ROADMAP.md) - C/C++ feature tracking
```

**Step 2: Commit**

```bash
jj commit -m "docs: Update README links to new docs/ structure"
```

---

### Task 13: Update ROADMAP.md cross-references

**Files:**
- Modify: `ROADMAP.md`

**Step 1: Check for stale cross-references**

Scan ROADMAP.md for any links to deleted files. Update to point to new paths.

**Step 2: Commit (if changes needed)**

```bash
jj commit -m "docs: Update ROADMAP cross-references"
```

---

### Task 14: Final verification

**Step 1: Verify no broken links**

```bash
# Check that all referenced files exist
grep -roh '\[.*\](.*\.md)' docs/ README.md ROADMAP.md | \
  grep -oP '\(.*?\)' | tr -d '()' | sort -u | \
  while read f; do test -f "$f" || echo "BROKEN: $f"; done
```

**Step 2: Verify directory structure matches design**

```bash
find docs/ -type f | sort
```

Expected:
```
docs/guides/c-cpp-integration.md
docs/guides/ci-cd.md
docs/guides/quickstart.md
docs/guides/remote-execution.md
docs/guides/worker-setup.md
docs/plans/2026-02-27-remote-execution-directory-tree.md
docs/plans/2026-03-01-docs-cleanup-design.md
docs/plans/2026-03-01-docs-cleanup-plan.md
docs/reference/architecture.md
docs/reference/auto-worker-registration.md
docs/reference/linking-strategy.md
```

**Step 3: Verify root is clean**

```bash
ls *.md
```

Expected: `README.md`, `ROADMAP.md`
