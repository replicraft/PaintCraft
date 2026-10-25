#!/bin/sh
# Install the CLI from this checkout. Requires the project's Rust toolchain.
set -eu
paintcraft_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
exec cargo install --locked --path "$paintcraft_root/apps/paintcraft-cli" "$@"
