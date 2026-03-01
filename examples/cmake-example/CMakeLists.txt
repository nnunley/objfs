cmake_minimum_required(VERSION 3.5)
project(ObjfsCMakeExample)

# Enable objfs distributed caching
set(CMAKE_C_COMPILER_LAUNCHER "${CMAKE_CURRENT_SOURCE_DIR}/../../target/release/objfs-cc-wrapper")
set(CMAKE_CXX_COMPILER_LAUNCHER "${CMAKE_CURRENT_SOURCE_DIR}/../../target/release/objfs-cc-wrapper")

# Simple C executable
add_executable(hello_c hello.c)

# Simple C++ executable
add_executable(hello_cpp hello.cpp)

# Library and executable
add_library(mylib mylib.c)
add_executable(uselib main.c)
target_link_libraries(uselib mylib)
