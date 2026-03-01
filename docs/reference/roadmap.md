# objfs C/C++ Roadmap to ccache Parity

Track progress toward ccache-level caching support.

## Current Status

✅ **Working:**
- Local CAS caching for .c/.cpp → .o
- GCC and Clang support
- CMake and Make integration
- Basic flag handling
- Content-addressed storage

❌ **Missing (vs ccache):**
- Header dependency tracking
- Comprehensive flag handling
- LTO support
- Precompiled headers
- Link caching

## Issue Tracking

Issues stored in git notes (refs/notes/issue-*).

Commands:
- `git-issue list` - Show all issues
- `git-issue ready` - Show ready to work on
- `git-issue deps` - Show dependency graph
- `git-issue show <id>` - View issue details
- `git-issue update <id>` - Modify issue

Sync with remote:
- `git-issue setup-sync enable` - Auto-sync notes on push/pull
- Or manually: `git push origin 'refs/notes/*'`

### High Priority (Correctness)

**#f6021c5: Comprehensive compiler flag handling** (no blockers)
- Include all flags in cache key (-I, -D, -O, -std, etc)
- Document which flags affect output
- Test with all common flag combinations
- Status: Ready to start

**#1b51c7f: Header dependency tracking** (blocked by #f6021c5)
- Parse -MD output for .h dependencies
- Include header hashes in cache key
- Invalidate on header changes
- Critical: Currently returns stale .o files
- Status: Blocked until flags done

**#86a021d: Remote execution support** (no blockers)
- Integrate with RE API v2
- Upload sources to remote CAS
- Execute on remote workers
- Download results
- Status: Ready to start

### Medium Priority

**#75ac80a: LTO flag support** (relates to #f6021c5)
- Handle -flto and variants
- Cache GIMPLE bytecode correctly
- Support thin LTO
- Status: Can start anytime

### Low Priority (Optimizations)

**#7b7c653: Precompiled header caching** (blocked by #1b51c7f)
- Cache .pch files
- Track transitive header deps
- Status: Blocked

**#d4f9b53: Link-time caching** (no blockers)
- Cache linking operations
- Handle large executables
- Platform-specific
- Status: Can start anytime

## Dependency Graph

```
f6021c5 (flags) ──blocks──> 1b51c7f (headers) ──blocks──> 7b7c653 (PCH)
                                ↑
                            relates_to
                                ↑
                          75ac80a (LTO)

86a021d (remote) ← independent
d4f9b53 (link)   ← independent
```

## Next Steps

1. **Start with #f6021c5** (flags): Foundation for everything
2. **Then #1b51c7f** (headers): Critical correctness issue
3. **Parallel #86a021d** (remote): Independent, high value

After these three, objfs will match ccache for common use cases.

## Milestones

### Milestone 1: Correctness (flags + headers)
- Complete flag handling
- Header dependency tracking
- No stale cache returns
- Status: ccache-equivalent correctness

### Milestone 2: Distribution (remote)
- Remote execution working
- Distributed cache sharing
- Status: Beyond ccache capabilities

### Milestone 3: Optimizations (LTO, PCH, link)
- Advanced feature support
- Full compiler flag coverage
- Status: ccache parity + distribution

## Testing Strategy

For each issue:
1. Write tests with real C/C++ projects
2. Compare behavior with ccache
3. Verify cache hits/misses correct
4. Benchmark performance

Test projects:
- Simple: examples/cmake-example
- Medium: Real OSS project (TBD)
- Large: Major C++ codebase (TBD)

## Success Criteria

**ccache parity achieved when:**
- ✅ Header changes invalidate cache
- ✅ All compiler flags handled correctly
- ✅ No false cache hits (correctness)
- ✅ Comparable hit rate to ccache
- ✅ Distributed caching works
- ✅ Remote execution works

**Beyond ccache:**
- ✅ Distributed builds (not just cache)
- ✅ Works for Rust + C/C++ in same project
- ✅ Unified CAS for all languages
