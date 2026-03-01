# CMake Integration Example

objfs caching for CMake builds.

## Build

```bash
mkdir build && cd build
cmake .. && make
```

Shows `[objfs-cc]` for each compilation.

## How It Works

CMakeLists.txt sets:
```cmake
set(CMAKE_C_COMPILER_LAUNCHER "objfs-cc-wrapper")
set(CMAKE_CXX_COMPILER_LAUNCHER "objfs-cc-wrapper")
```

Wrapper intercepts every compilation:
1. Compute SHA256(source + flags)
2. Check local CAS
3. Hit: restore .o instantly
4. Miss: compile and store

## Run

```bash
./hello_c
./hello_cpp
./uselib
```

## Status

- ✅ Local caching
- ✅ Content-addressed storage
- ⏳ Remote execution
- ⏳ Distributed cache
