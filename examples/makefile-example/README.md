# Makefile Integration Example

objfs caching for make builds via CC/CXX.

## Quick Start

```bash
WRAPPER=/path/to/objfs/target/release/objfs-cc-wrapper

# GCC
CC="$WRAPPER gcc" CXX="$WRAPPER g++" make

# Clang
CC="$WRAPPER clang" CXX="$WRAPPER clang++" make

# Rebuild (hits cache)
make clean && CC="$WRAPPER gcc" CXX="$WRAPPER g++" make
```

## Setup

Add to ~/.bashrc:

```bash
export CC="objfs-cc-wrapper gcc"
export CXX="objfs-cc-wrapper g++"
```

Or create alias:
```bash
alias mc='CC="objfs-cc-wrapper gcc" CXX="objfs-cc-wrapper g++" make'
```

## Works With

Any system respecting CC/CXX:
- GNU Make, BSD Make, Ninja
- Autotools (./configure CC=...)
- Shell scripts

All compilers:
- gcc, g++, clang, clang++
- Cross-compilers (arm-linux-gnueabi-gcc)

## Limitations

Caches .c/.cpp → .o only.
Headers not tracked (change .h = `make clean`).

## See Also

CMake: `../cmake-example/`
Remote: Set `OBJFS_REMOTE_ENDPOINT`
