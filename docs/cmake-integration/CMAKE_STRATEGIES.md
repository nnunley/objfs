# CMake Integration Strategies

How to integrate objfs distributed caching with CMake builds.

## Challenge

CMake generates build systems (Makefiles, Ninja) rather than executing builds directly. We need to intercept the actual compiler invocations.

## Strategy 1: CMAKE_C_COMPILER_LAUNCHER / CMAKE_CXX_COMPILER_LAUNCHER

**Most robust approach** - CMake 3.4+ built-in support.

### How It Works

CMake wraps every compiler invocation with a launcher program.

```cmake
set(CMAKE_C_COMPILER_LAUNCHER "/usr/local/bin/objfs-wrapper")
set(CMAKE_CXX_COMPILER_LAUNCHER "/usr/local/bin/objfs-wrapper")
```

Every compilation becomes:
```bash
/usr/local/bin/objfs-wrapper gcc -c foo.c -o foo.o
```

### Implementation

Create `objfs-wrapper` that:
1. Parses compiler command line
2. Computes cache key from inputs
3. Checks remote cache
4. On miss: executes compiler, caches result
5. On hit: retrieves from cache

### Pros
- ✅ Works with any CMake generator (Make, Ninja, etc.)
- ✅ CMake handles it natively
- ✅ Clean, supported approach
- ✅ Works with ccache, distcc patterns

### Cons
- ❌ Requires wrapper for each compiler (gcc, g++, clang, clang++)
- ❌ Must handle complex compiler flags

### Example

```cmake
# CMakeLists.txt
cmake_minimum_required(VERSION 3.4)
project(MyProject)

# Enable objfs caching
set(CMAKE_C_COMPILER_LAUNCHER "/usr/local/bin/objfs-cc-wrapper")
set(CMAKE_CXX_COMPILER_LAUNCHER "/usr/local/bin/objfs-cxx-wrapper")

add_executable(myapp main.cpp foo.cpp bar.cpp)
```

## Strategy 2: CMAKE_<LANG>_COMPILER Override

**Direct compiler replacement.**

### How It Works

Replace the compiler entirely:

```cmake
set(CMAKE_C_COMPILER "/usr/local/bin/objfs-gcc")
set(CMAKE_CXX_COMPILER "/usr/local/bin/objfs-g++")
```

`objfs-gcc` is a wrapper that eventually calls real `gcc`.

### Implementation

```bash
#!/bin/bash
# /usr/local/bin/objfs-gcc
exec /usr/local/bin/objfs-wrapper gcc "$@"
```

### Pros
- ✅ Works with older CMake versions
- ✅ Complete control over invocations

### Cons
- ❌ CMake introspection may fail (compiler ID detection)
- ❌ Must respond correctly to --version, feature tests
- ❌ More fragile than launcher approach

## Strategy 3: Rule-Based Wrapper (Ninja/Make specific)

**Modify generated build rules.**

### How It Works

For Ninja:
```bash
cmake -G Ninja -DCMAKE_MAKE_PROGRAM=/usr/local/bin/objfs-ninja
```

`objfs-ninja` wraps Ninja and intercepts compilation commands.

### Pros
- ✅ No CMake changes needed
- ✅ Works across projects

### Cons
- ❌ Generator-specific (separate wrapper for Make, Ninja, etc.)
- ❌ Complex to parse build rules
- ❌ Fragile to build system changes

## Strategy 4: Remote Execution API Integration

**Use NativeLink's Remote Execution directly.**

### How It Works

Modify CMake to use NativeLink's Remote Execution API for compilation steps.

Requires CMake modifications or custom toolchain file.

### Pros
- ✅ Native remote execution support
- ✅ Parallel distributed builds
- ✅ Platform-aware execution

### Cons
- ❌ Requires significant CMake customization
- ❌ Complex to implement
- ❌ Not portable across projects

## Recommended Approach: Strategy 1 (Compiler Launcher)

Build `objfs-cc-wrapper` and `objfs-cxx-wrapper`:

```rust
// src/bin/objfs-cc-wrapper.rs
use std::env;
use std::process::Command;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    // First arg is the actual compiler (gcc, clang, etc.)
    if args.is_empty() {
        eprintln!("Usage: objfs-cc-wrapper <compiler> [args...]");
        std::process::exit(1);
    }

    let compiler = &args[0];
    let compiler_args = &args[1..];

    // Parse compilation command
    let build_info = parse_compiler_args(compiler, compiler_args);

    // Check if this is a compilation (not linking, not preprocessing)
    if !is_compilation(&build_info) {
        // Pass through non-compilation commands
        exec_compiler(compiler, compiler_args);
        return;
    }

    // Compute cache key
    let cache_key = compute_cache_key(&build_info);

    // Try cache
    if let Some(output) = check_cache(&cache_key) {
        restore_output(&output, &build_info);
        println!("[objfs] cache hit: {}", build_info.output);
        return;
    }

    // Cache miss - execute and cache
    exec_and_cache(compiler, compiler_args, &cache_key);
}
```

### Usage

```bash
# Install wrappers
cargo build --release
sudo cp target/release/objfs-cc-wrapper /usr/local/bin/
sudo cp target/release/objfs-cxx-wrapper /usr/local/bin/

# Configure CMake project
cd my-cmake-project
mkdir build && cd build
cmake .. \
  -DCMAKE_C_COMPILER_LAUNCHER=/usr/local/bin/objfs-cc-wrapper \
  -DCMAKE_CXX_COMPILER_LAUNCHER=/usr/local/bin/objfs-cxx-wrapper
make -j8

# Or set in CMakeLists.txt
```

### Toolchain File Approach

```cmake
# objfs-toolchain.cmake
set(CMAKE_C_COMPILER_LAUNCHER "/usr/local/bin/objfs-cc-wrapper")
set(CMAKE_CXX_COMPILER_LAUNCHER "/usr/local/bin/objfs-cxx-wrapper")
```

```bash
cmake .. -DCMAKE_TOOLCHAIN_FILE=objfs-toolchain.cmake
```

## Integration with Existing Tools

### ccache Compatibility

Stack objfs with ccache:

```cmake
set(CMAKE_C_COMPILER_LAUNCHER "ccache;/usr/local/bin/objfs-cc-wrapper")
```

Execution: `ccache /usr/local/bin/objfs-cc-wrapper gcc ...`

### distcc Compatibility

Use objfs instead of distcc for remote execution.

## Implementation Checklist

- [ ] Create `objfs-cc-wrapper` binary
- [ ] Create `objfs-cxx-wrapper` binary (or symlink to same code)
- [ ] Parse GCC/Clang command-line arguments
- [ ] Detect compilation vs linking vs preprocessing
- [ ] Compute cache keys from input files + flags
- [ ] Integrate with existing objfs CAS/AC
- [ ] Handle compiler-specific flags (GCC vs Clang)
- [ ] Support cross-compilation scenarios
- [ ] Test with complex CMake projects
- [ ] Document CMake integration

## Testing Plan

1. Simple C project
2. Simple C++ project
3. Mixed C/C++ project
4. Project with generated files
5. Cross-compilation
6. Large real-world project (LLVM, Chromium subset)

## Performance Expectations

**Cold cache:**
- Compilation time + network overhead
- Expect 10-20% slower than local

**Warm cache:**
- Network retrieval only
- Expect 10-100x faster (depending on file size)

**Distributed:**
- Parallel compilation across workers
- Expect 2-10x faster (depending on worker count)

## Next Steps

1. Implement `objfs-cc-wrapper` in Rust
2. Parse GCC/Clang arguments
3. Reuse objfs cache infrastructure
4. Test with real CMake project
5. Document usage patterns
6. Create example projects
