# C/C++ Compiler Integration

objfs caches C/C++ builds via `objfs-cc-wrapper` compiler launcher.

## Supported Compilers

- GCC: `gcc`, `g++`
- Clang: `clang`, `clang++`
- Cross-compilers: `arm-linux-gnueabi-gcc`, `aarch64-linux-gnu-g++`
- Any compiler accepting standard flags

## Integration

**CMake:** Set in `CMakeLists.txt`:
```cmake
set(CMAKE_C_COMPILER_LAUNCHER "objfs-cc-wrapper")
set(CMAKE_CXX_COMPILER_LAUNCHER "objfs-cc-wrapper")
```

CMake prepends the wrapper to every compilation. Example: `examples/cmake-example/`

**Make:** Set `CC` and `CXX`:
```bash
CC="objfs-cc-wrapper gcc" CXX="objfs-cc-wrapper g++" make
```

Example: `examples/makefile-example/`

**Autotools:** Pass to configure:
```bash
./configure CC="objfs-cc-wrapper gcc" CXX="objfs-cc-wrapper g++"
make
```

**Direct:** Wrap any compilation:
```bash
objfs-cc-wrapper gcc -c hello.c -o hello.o
```

## How It Works

Wrapper intercepts compilation:

```
objfs-cc-wrapper gcc -c hello.c -o hello.o
```

1. Parse arguments: input, output, flags
2. Compute SHA256(source content + compiler + flags)
3. Check CAS at `~/.cache/objfs/cas/cache/<key>`
4. Hit: Restore .o file instantly
5. Miss: Run gcc, store result

Caches compilation only. Skips linking, preprocessing (-E), assembly (-S), dependency generation (-M).

## Performance

- Hit: ~100ms (.o restoration)
- Miss: Normal time + 50ms (storage)
- Rebuild: 10-50x faster

Example (4 files):
```bash
time make  # 2.1s (miss)
time make  # 0.2s (hit) → 10.5x
```

## Installation

Build:
```bash
cargo build --release --bin objfs-cc-wrapper
```

Install:
```bash
sudo cp target/release/objfs-cc-wrapper /usr/local/bin/
```

Configure (~/.bashrc):
```bash
export CC="objfs-cc-wrapper gcc"
export CXX="objfs-cc-wrapper g++"
```

## Compatibility

Works with systems respecting CC/CXX:
- CMake, GNU Make, BSD Make, Ninja
- Autotools, Bazel
- Custom scripts

Fails when compilers are hardcoded in build files.

## Limitations

Caches .c/.cpp → .o compilation only.

Missing:
- Header dependency tracking (change .h returns stale .o)
- Link caching
- Precompiled headers
- Remote execution

Workaround for headers: `make clean` after .h changes.

## Environment Variables

```bash
OBJFS_DISABLE=1 make              # Disable caching
make 2>&1 | grep objfs-cc         # Show cache activity
OBJFS_REMOTE_ENDPOINT=...         # Remote cache (future)
```

## vs ccache

| Feature | objfs | ccache |
|---------|-------|--------|
| Local cache | ✅ | ✅ |
| Header deps | ❌ | ✅ |
| Remote cache | ⏳ | ❌ |
| Remote execution | ⏳ | ❌ |
| Setup | One binary | Moderate |

Use ccache for header tracking now.
Use objfs for distributed builds (future) or with Rust.

## Troubleshooting

No cache hits:
```bash
make 2>&1 | grep objfs-cc      # Verify wrapper runs
echo $OBJFS_DISABLE            # Check if disabled
ls ~/.cache/objfs/cas/cache/   # Verify cache exists
```

Wrong output: Header changed. Run `make clean && make`.

Compiler not found: Add gcc/clang to PATH.

## Examples

- `examples/cmake-example/` - CMake with library + executables
- `examples/makefile-example/` - Make with C and C++

## Roadmap

1. Remote execution
2. Header dependency tracking
3. Distributed cache
4. Link caching
5. Precompiled header support

## Quick Start

```bash
cd examples/cmake-example
mkdir build && cd build
cmake .. && make     # miss
make clean && make   # hit
```
