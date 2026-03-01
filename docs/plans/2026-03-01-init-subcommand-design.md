# Init Subcommand Design

Related issue: #5d730f5

## Goal

Add an `objfs init` subcommand that configures a repository to use objfs.
A persistent config file serves as the single source of truth: the init
command writes it, and the runtime binaries read it.

## Config File

The config file lives at the project root as `objfs.toml` (or `.objfs.toml`).
Both the init command and the runtime read it. The `--config` flag accepts
JSON5 files as well.

```toml
[remote]
endpoint = "http://build-server:50051"
instance = "main"
min_remote_size = 1

[worker]
auto_start = true
targets = ["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"]

[project]
type = "rust"   # auto-detected: rust, cmake, make, mixed
```

Precedence, highest to lowest: environment variables, `objfs.toml`, defaults.

## Init Command

```
objfs init                        # auto-detect project, prompt for remote
objfs init --config team.toml     # apply shared config
objfs init --config team.json5    # apply shared JSON5 config
objfs init --export-config        # write current config to objfs.toml
objfs init --export-config=json5  # write current config as JSON5
```

The command performs these steps in order:

1. Scan for Cargo.toml, CMakeLists.txt, and Makefile to detect the project type.
2. Read the `--config` file if provided; otherwise prompt for the remote endpoint.
   An empty endpoint means local-only mode.
3. Write `objfs.toml` to the project root.
4. For Rust projects: create or update `.cargo/config.toml` with the
   rustc-wrapper setting.
5. For C/C++ projects: print instructions for setting CC/CXX or
   CMAKE_COMPILER_LAUNCHER.
6. If an endpoint is configured: run a health check against it and warn if
   the server is unreachable.
7. Print a summary of what was configured.

Re-running `objfs init` in a project that already has `objfs.toml` reads the
existing values as defaults, lets the user update them, and rewrites the file.
The command preserves all existing settings that the user leaves unchanged.

## Runtime Config Loading

`cargo-objfs-rustc` and `objfs-cc-wrapper` load configuration through a
four-level hierarchy:

1. Check environment variables (existing behavior, highest precedence).
2. Walk up from the current directory looking for `objfs.toml` or `.objfs.toml`.
3. Fall back to `~/.config/objfs/config.toml` for global defaults.
4. Apply built-in defaults for anything still unset.

A team commits `objfs.toml` to their repository, and every developer gets
the correct configuration automatically.

## Error Handling

| Condition | Behavior |
|-----------|----------|
| `--config` file missing | Hard error; abort |
| Remote endpoint unreachable | Warn and continue; write the config anyway |
| `.cargo/config.toml` has a different rustc-wrapper | Warn and prompt before overwriting |
| Unknown project type | Warn; write config but skip integration files |

## Current State

The `objfs` binary (`src/bin/cli.rs`) parses subcommands manually: stats,
clear, enable, disable, evict, help. The `enable` subcommand already creates
`.cargo/config.toml`. Configuration relies entirely on environment variables
(`RemoteConfig` in `src/remote_config.rs`). No config file support exists.

Three binaries ship today: `cargo-objfs-rustc`, `objfs`, and `objfs-cc-wrapper`.
