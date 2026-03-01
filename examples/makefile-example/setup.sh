#!/bin/bash
# Setup script for using objfs with make builds

set -e

# Build the wrapper
echo "Building objfs-cc-wrapper..."
cd "$(dirname "$0")/../.."
cargo build --release --bin objfs-cc-wrapper

WRAPPER_PATH="$(pwd)/target/release/objfs-cc-wrapper"

echo ""
echo "✓ Wrapper built at: $WRAPPER_PATH"
echo ""
echo "To use with make, run:"
echo "  CC='$WRAPPER_PATH gcc' CXX='$WRAPPER_PATH g++' make"
echo ""
echo "Or for Clang:"
echo "  CC='$WRAPPER_PATH clang' CXX='$WRAPPER_PATH clang++' make"
echo ""
echo "Add to ~/.bashrc or ~/.zshrc for permanent setup:"
echo "  export CC='$WRAPPER_PATH gcc'"
echo "  export CXX='$WRAPPER_PATH g++'"
