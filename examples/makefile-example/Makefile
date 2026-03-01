# Simple Makefile that works with objfs-cc-wrapper
# Usage:
#   make                    # Normal build
#   make CC=objfs-cc-wrapper gcc   # With wrapper (manual)
#
# Or set environment variables:
#   export CC="objfs-cc-wrapper gcc"
#   export CXX="objfs-cc-wrapper g++"
#   make

CC ?= gcc
CXX ?= g++
CFLAGS = -Wall -O2
CXXFLAGS = -Wall -O2

TARGETS = hello_c hello_cpp
OBJS = hello.o hello_cpp.o

all: $(TARGETS)

# Separate compilation and linking to show caching
hello.o: hello.c
	$(CC) $(CFLAGS) -c -o $@ $<

hello_cpp.o: hello.cpp
	$(CXX) $(CXXFLAGS) -c -o $@ $<

hello_c: hello.o
	$(CC) -o $@ $<

hello_cpp: hello_cpp.o
	$(CXX) -o $@ $<

clean:
	rm -f $(TARGETS) *.o

.PHONY: all clean
