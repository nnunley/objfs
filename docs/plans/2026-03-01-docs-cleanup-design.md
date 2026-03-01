# Documentation Cleanup Design

## Problem

The repository root contains 23 markdown files — a mix of stale status reports,
overlapping guides, and misleading completion claims. Developers cannot tell
which documents are current or where to find authoritative information.

## Audience

Developers and devops engineers who use objfs.

## Decision: Delete Status Reports

These files are point-in-time snapshots that no longer reflect reality.
Git history preserves them. Any actionable TODOs get extracted into git-issues
before deletion.

| File | Reason |
|------|--------|
| COMPLETE.md | Early-phase snapshot, local-only |
| FINAL_IMPLEMENTATION.md | Claims "final" but contradicted by later work |
| IMPLEMENTATION_STATUS.md | Mid-phase status, superseded |
| TDD_SUMMARY.md | Historical methodology narrative |
| TEST_SUMMARY.md | Stale test counts |
| REMOTE_EXECUTION_COMPLETE.md | Misleading completion claim |
| REMOTE_EXECUTION_STATUS.md | Infrastructure status snapshot |
| INTEGRATION_TEST_RESULTS.md | Single test run snapshot |
| CMAKE_INTEGRATION_COMPLETE.md | Superseded by CMAKE_STRATEGIES.md |
| CI_CD_INTEGRATION_SUMMARY.md | Completion report framing; key content moves to guide |

## New Structure

```
README.md                            # Updated links to docs/
ROADMAP.md                           # Stays in root

docs/
├── guides/
│   ├── quickstart.md                # From QUICKSTART.md
│   ├── remote-execution.md          # From REMOTE_EXECUTION.md
│   ├── worker-setup.md              # 4 worker docs → 1
│   ├── ci-cd.md                     # From CI_CD_INTEGRATION_SUMMARY.md
│   └── c-cpp-integration.md         # C_CPP + CMAKE docs → 1
│
├── reference/
│   ├── architecture.md              # From ARCHITECTURE.md
│   ├── linking-strategy.md          # 2 linking docs → 1
│   └── auto-worker-registration.md  # From AUTO_WORKER_REGISTRATION.md
│
└── plans/                           # Unchanged
    └── ...
```

## Consolidation Details

**Worker setup** (4 files → 1): MACOS_WORKER_SETUP, MULTI_WORKER_SETUP,
NATIVELINK_RUST_SETUP, and NATIVELINK_CROSS_COMPILE_SETUP merge into
`worker-setup.md` with sections for each platform. Cross-compile warnings
about NixOS/osxcross stay prominent.

**Linking strategy** (2 files → 1): DISTRIBUTED_COMPILATION_LOCAL_LINKING and
PLATFORM_COMPATIBLE_LINKING cover the same concept from different angles.
Merge into `linking-strategy.md`.

**C/C++ integration** (3 files → 1): C_CPP_INTEGRATION, CMAKE_INTEGRATION_COMPLETE,
and docs/cmake-integration/CMAKE_STRATEGIES.md consolidate into
`c-cpp-integration.md`. The `docs/cmake-integration/` directory is removed.

**CI/CD** (1 file → 1): Key workflows from CI_CD_INTEGRATION_SUMMARY distilled
into a how-to guide, dropping benchmarks and status report framing.

## README Update

Update the Documentation section to point to new docs/ paths.
