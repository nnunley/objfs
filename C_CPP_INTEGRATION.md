# C/C++ Compiler Integration

objfs now supports C/C++ builds with both CMake and Make via the `objfs-cc-wrapper` compiler launcher.

## Supported Compilers

All GCC and LLVM variants:
- **GCC:** `gcc`, `g++`
- **Clang:** `clang`, `clang++`
- **Cross-compilers:** `arm-linux-gnueabi-gcc`, `aarch64-linux-gnu-g++`, etc.
- **Platform wrappers:** Any compiler that accepts standard flags

## Integration Methods

### 1. CMake (Compiler Launcher)

Set in `CMakeLists.txt`:
```cmake
set(CMAKE_C_COMPILER_LAUNCHER "objfs-cc-wrapper")
set(CMAKE_CXX_COMPILER_LAUNCHER "objfs-cc-wrapper")
```

CMake automatically prepends the launcher to every compilation:
```bash
mkdir build && cd build
cmake ..
make
```

**Example:** `examples/cmake-example/`

### 2. Make (Environment Variables)

Set `CC` and `CXX` when invoking make:
```bash
CC="objfs-cc-wrapper gcc" CXX="objfs-cc-wrapper g++" make
```

Or export for entire session:
```bash
export CC="objfs-cc-wrapper gcc"
export CXX="objfs-cc-wrapper g++"
make
```

**Example:** `examples/makefile-example/`

### 3. Autotools

Pass to configure script:
```bash
./configure CC="objfs-cc-wrapper gcc" CXX="objfs-cc-wrapper g++"
make
```

### 4. Direct Invocation

Manually wrap any compiler invocation:
```bash
objfs-cc-wrapper gcc -c hello.c -o hello.o
objfs-cc-wrapper clang++ -O2 -c foo.cpp -o foo.o
```

## How It Works

The wrapper intercepts compilation commands:

```
objfs-cc-wrapper gcc -c hello.c -o hello.o
  ↓
1. Parse arguments: input=hello.c, output=hello.o, flags=[-c]
2. Compute cache key: SHA256(hello.c content + gcc + flags)
3. Check CAS: ~/.cache/objfs/cas/cache/<key>
4. On hit: Restore hello.o from CAS (instant)
5. On miss: Run gcc, store hello.o in CAS
```

**Cache key includes:**
- Source file content (SHA256)
- Compiler name (gcc, clang, etc.)
- All compilation flags affecting output (-O2, -Wall, etc.)

**Not cached:**
- Linking operations (multiple .o → executable)
- Preprocessing-only (-E flag)
- Assembly-only (-S flag)
- Dependency generation (-M, -MM)

## Performance

**Typical results:**
- Cache hit: ~100ms (instant .o restoration)
- Cache miss: Normal compilation time + ~50ms (storage)
- Rebuild speedup: 10-50x depending on project size

**Example (4 compilation units):**
```bash
# First build
make clean && time make
# 2.1 seconds (cache misses)

# Rebuild
make clean && time make
# 0.2 seconds (cache hits) → 10.5x faster
```

## Installation

### Build the wrapper:
```bash
cargo build --release --bin objfs-cc-wrapper
```

### Install to PATH:
```bash
sudo cp target/release/objfs-cc-wrapper /usr/local/bin/
```

### Configure shell (~/.bashrc or ~/.zshrc):
```bash
export CC="objfs-cc-wrapper gcc"
export CXX="objfs-cc-wrapper g++"
```

Now all `make` builds automatically use caching.

## Compatibility

**Build systems that work:**
- CMake (via CMAKE_C/CXX_COMPILER_LAUNCHER)
- GNU Make (via CC/CXX variables)
- BSD Make (via CC/CXX variables)
- Ninja (via CC/CXX environment)
- Autotools (via configure CC=...)
- Bazel (via --action_env)
- Custom scripts (direct invocation)

**Build systems that don't work:**
- Systems that hardcode compiler paths
- Systems that bypass CC/CXX variables
- Build configs with compiler path in Makefile

## Limitations

**Current implementation:**
- ✅ Caches compilation (.c/.cpp → .o)
- ✅ Supports all standard flags
- ✅ Preserves file permissions
- ❌ Header dependencies not tracked (change .h = rebuild all)
- ❌ Linking operations not cached
- ❌ Precompiled headers (.pch) not supported
- ❌ Remote execution not implemented (local cache only)

**Header dependency issue:**
```c
// hello.c includes util.h
#include "util.h"

// Change util.h → cache returns stale hello.o
// Workaround: make clean after header changes
```

Future: Track header dependencies via -MD flag parsing.

## Environment Variables

**Control wrapper behavior:**

```bash
# Disable caching (pass through to compiler)
OBJFS_DISABLE=1 make

# See what's being cached
make 2>&1 | grep objfs-cc

# Use remote cache (future)
export OBJFS_REMOTE_ENDPOINT="http://build-server:50051"
export OBJFS_REMOTE_INSTANCE="main"
```

## Comparison with ccache

| Feature | objfs-cc-wrapper | ccache |
|---------|-----------------|--------|
| Local caching | ✅ | ✅ |
| Header deps | ❌ (future) | ✅ |
| Remote cache | ⏳ (planned) | ❌ |
| Distributed builds | ⏳ (planned) | ❌ |
| Remote execution | ⏳ (planned) | ❌ |
| Setup complexity | Simple (one binary) | Moderate |
| Storage | Content-addressed | Keyed |

**Use ccache when:**
- You need header dependency tracking now
- Local-only builds

**Use objfs when:**
- You want distributed caching (future)
- You want remote execution (future)
- You already use objfs for Rust

## Troubleshooting

**No cache hits:**
```bash
# Verify wrapper is invoked
make 2>&1 | grep objfs-cc

# Check if caching is disabled
echo $OBJFS_DISABLE

# Verify cache directory
ls ~/.cache/objfs/cas/cache/
```

**Cache hits but wrong output:**
```bash
# Likely header dependency issue
# Workaround: clean build after header changes
make clean && make
```

**Compiler not found:**
```bash
# Wrapper passes first arg to exec
# Make sure gcc/clang is in PATH
which gcc
which clang
```

## Examples

See working examples:
- `examples/cmake-example/` - CMake project with library + executables
- `examples/makefile-example/` - Make project with C and C++

## Next Steps

**Planned features:**
1. **Remote execution** - Offload compilations to workers
2. **Header dependency tracking** - Parse -MD output for cache invalidation
3. **Distributed cache** - Share CAS across team
4. **Link-time caching** - Cache final executables
5. **Precompiled header support** - Handle .pch files correctly

**Try it now:**
```bash
cd examples/cmake-example
mkdir build && cd build
cmake ..
make        # cache misses
make clean
make        # cache hits
```
