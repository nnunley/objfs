# CMake Integration Example

Demonstrates using objfs distributed caching with CMake builds.

## Build

```bash
mkdir build && cd build
cmake ..
make
```

Every compilation shows `[objfs-cc]` cache hit/miss status.

## How It Works

CMakeLists.txt sets:
```cmake
set(CMAKE_C_COMPILER_LAUNCHER "objfs-cc-wrapper")
set(CMAKE_CXX_COMPILER_LAUNCHER "objfs-cc-wrapper")
```

Every compilation becomes:
```
objfs-cc-wrapper gcc -c hello.c -o hello.o
```

The wrapper:
1. Parses compiler arguments
2. Computes cache key from source + flags
3. Checks local CAS for cached result
4. On hit: Restores .o file instantly
5. On miss: Compiles and stores result in CAS

## Run Examples

```bash
./hello_c
./hello_cpp
./uselib
```

## Features

- ✅ Local caching with content-addressable storage
- ✅ Cache key from source + compiler flags
- ✅ Instant .o file restoration on cache hit
- ⏳ Remote execution (future)
- ⏳ Distributed cache sharing (future)
