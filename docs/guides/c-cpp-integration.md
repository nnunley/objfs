# C/C++ Integration

## Overview

objfs caches C/C++ compilation via `objfs-cc-wrapper`, a compiler launcher that intercepts compilation commands, computes cache keys from source content and compiler flags, and restores cached object files on cache hits.

## Supported Compilers

- GCC: `gcc`, `g++`
- Clang: `clang`, `clang++`
- Cross-compilers: `arm-linux-gnueabi-gcc`, `aarch64-linux-gnu-g++`
- Any compiler accepting standard flags

## Installation

Build:
```bash
cargo build --release --bin objfs-cc-wrapper
```

Install:
```bash
sudo cp target/release/objfs-cc-wrapper /usr/local/bin/
```

Configure in `~/.bashrc` or `~/.zshrc`:
```bash
export CC="objfs-cc-wrapper gcc"
export CXX="objfs-cc-wrapper g++"
```

## Build System Integration

### CMake (Recommended: Compiler Launcher)

CMake 3.4+ has built-in support for compiler launchers. This is the most robust approach.

Set in `CMakeLists.txt`:
```cmake
cmake_minimum_required(VERSION 3.4)
project(MyProject)

set(CMAKE_C_COMPILER_LAUNCHER "/usr/local/bin/objfs-cc-wrapper")
set(CMAKE_CXX_COMPILER_LAUNCHER "/usr/local/bin/objfs-cc-wrapper")

add_executable(myapp main.cpp foo.cpp bar.cpp)
```

Or pass at configure time:
```bash
cmake .. \
  -DCMAKE_C_COMPILER_LAUNCHER=/usr/local/bin/objfs-cc-wrapper \
  -DCMAKE_CXX_COMPILER_LAUNCHER=/usr/local/bin/objfs-cc-wrapper
```

Or use a toolchain file (`objfs-toolchain.cmake`):
```cmake
set(CMAKE_C_COMPILER_LAUNCHER "/usr/local/bin/objfs-cc-wrapper")
set(CMAKE_CXX_COMPILER_LAUNCHER "/usr/local/bin/objfs-cc-wrapper")
```

```bash
cmake .. -DCMAKE_TOOLCHAIN_FILE=objfs-toolchain.cmake
```

Every compilation becomes:
```bash
/usr/local/bin/objfs-cc-wrapper gcc -c foo.c -o foo.o
```

This works with any CMake generator (Make, Ninja, etc.) and follows the same pattern as ccache and distcc.

### Make

Set `CC` and `CXX`:
```bash
CC="objfs-cc-wrapper gcc" CXX="objfs-cc-wrapper g++" make
```

Example: `examples/makefile-example/`

### Autotools

Pass to configure:
```bash
./configure CC="objfs-cc-wrapper gcc" CXX="objfs-cc-wrapper g++"
make
```

### Direct Usage

Wrap any compilation:
```bash
objfs-cc-wrapper gcc -c hello.c -o hello.o
```

## How It Works

The wrapper intercepts compilation commands:

```
objfs-cc-wrapper gcc -c hello.c -o hello.o
```

1. Parse arguments: input files, output file, compiler flags
2. Compute SHA256(source content + compiler identity + flags)
3. Check CAS at `~/.cache/objfs/cas/cache/<key>`
4. Hit: Restore .o file instantly
5. Miss: Run gcc, store result in cache

The wrapper caches compilation only. It passes through linking, preprocessing (`-E`), assembly (`-S`), and dependency generation (`-M`) without caching.

**Performance:**
- Cache hit: ~100ms (object file restoration)
- Cache miss: Normal compilation time + ~50ms (storage overhead)
- Typical rebuild speedup: 10-50x

## CMake Strategy Alternatives

Beyond the recommended compiler launcher approach, there are other ways to integrate with CMake:

**Strategy 2: Compiler Override** -- Replace `CMAKE_C_COMPILER` with a wrapper script that calls objfs then the real compiler. Works with older CMake versions but may interfere with CMake's compiler introspection.

**Strategy 3: Rule-Based Wrapper** -- Wrap the build system itself (Ninja or Make) to intercept compilation commands. No CMake changes needed but is generator-specific and fragile.

**Strategy 4: Remote Execution API** -- Modify CMake to use NativeLink's Remote Execution API directly. Enables parallel distributed builds but requires significant CMake customization and is not portable across projects.

The compiler launcher (Strategy 1) is recommended for most use cases.

## Limitations

Caches `.c`/`.cpp` to `.o` compilation only.

Not supported:
- Header dependency tracking (changing a `.h` file may return a stale `.o`)
- Link caching
- Precompiled headers
- Remote execution for C/C++

Workaround for headers: run `make clean` after `.h` changes.

### Comparison with ccache

| Feature | objfs | ccache |
|---------|-------|--------|
| Local cache | Yes | Yes |
| Header deps | No | Yes |
| Remote cache | Planned | No |
| Remote execution | Planned | No |
| Setup | One binary | Moderate |

Use ccache if header dependency tracking is critical. Use objfs for distributed builds or alongside Rust projects already using objfs.

## Compatibility

Works with build systems that respect CC/CXX environment variables:
- CMake, GNU Make, BSD Make, Ninja
- Autotools, Bazel
- Custom build scripts

Does not work when compilers are hardcoded in build files.

### ccache Stacking

objfs can be stacked with ccache:

```cmake
set(CMAKE_C_COMPILER_LAUNCHER "ccache;/usr/local/bin/objfs-cc-wrapper")
```

## Environment Variables

```bash
OBJFS_DISABLE=1 make              # Disable caching
make 2>&1 | grep objfs-cc         # Show cache activity
OBJFS_REMOTE_ENDPOINT=...         # Remote cache (future)
```

## Troubleshooting

**No cache hits:**
```bash
make 2>&1 | grep objfs-cc      # Verify wrapper runs
echo $OBJFS_DISABLE            # Check if disabled
ls ~/.cache/objfs/cas/cache/   # Verify cache exists
```

**Wrong output after header change:**
Header changed but cached `.o` is stale. Run `make clean && make`.

**Compiler not found:**
Ensure gcc or clang is in PATH.

## Examples

- `examples/cmake-example/` -- CMake project with library and executables
- `examples/makefile-example/` -- Make project with C and C++ sources

Quick start:
```bash
cd examples/cmake-example
mkdir build && cd build
cmake .. && make     # miss
make clean && make   # hit
```
