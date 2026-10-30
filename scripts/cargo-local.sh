#!/bin/sh
# Optional helper for the project-local toolchain used during implementation.
# A normal Rust installation can run cargo directly.
set -eu
project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
export CARGO_HOME="$project_dir/.tools/cargo"
export RUSTUP_HOME="$project_dir/.tools/rustup"
export PATH="$CARGO_HOME/bin:$PATH"
exec cargo "$@"
