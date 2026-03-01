# Makefile Integration Example

Use objfs caching with standard make builds via CC/CXX environment variables.

## Quick Start

```bash
# Set wrapper path
WRAPPER=/path/to/objfs/target/release/objfs-cc-wrapper

# Build with GCC + caching
CC="$WRAPPER gcc" CXX="$WRAPPER g++" make

# Or Clang + caching
CC="$WRAPPER clang" CXX="$WRAPPER clang++" make

# Clean and rebuild (should hit cache)
make clean
CC="$WRAPPER gcc" CXX="$WRAPPER g++" make
```

## Persistent Setup

Add to `~/.bashrc` or `~/.zshrc`:

```bash
export OBJFS_WRAPPER="$HOME/.cargo/bin/objfs-cc-wrapper"
alias make-cached='CC="$OBJFS_WRAPPER gcc" CXX="$OBJFS_WRAPPER g++" make'
```

Then just run:
```bash
make-cached
```

## How It Works

The wrapper intercepts each compiler invocation:

```
make CC="objfs-cc-wrapper gcc"
  → objfs-cc-wrapper gcc -Wall -O2 -o hello_c hello.c
    → Compute cache key from hello.c + flags
    → Check CAS for cached result
    → On hit: Skip compilation
    → On miss: Run gcc and cache result
```

## Compatibility

Works with any build system that respects CC/CXX:
- GNU Make
- BSD Make
- Ninja (via CC/CXX)
- Autotools (./configure CC=... CXX=...)
- Plain shell scripts

Supports all compilers:
- gcc / g++
- clang / clang++
- Cross-compilers (arm-linux-gnueabi-gcc, etc.)
- Platform-specific wrappers

## Limitations

Currently caches only compilation (.c/.cpp → .o)
Linking operations pass through without caching
Header dependencies not tracked (change .h = rebuild all)

## Next Steps

For CMake projects, see `../cmake-example/`
For distributed caching, set `OBJFS_REMOTE_ENDPOINT`
