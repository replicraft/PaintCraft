#!/bin/sh
# Run from the repository root. Rust is installed only if rustup is unavailable.
set -eu
if ! command -v rustup >/dev/null 2>&1; then
    if [ -f "$HOME/.cargo/env" ]; then
        . "$HOME/.cargo/env"
    fi
fi
if ! command -v rustup >/dev/null 2>&1; then
    installer=$(mktemp)
    trap 'rm -f "$installer"' EXIT HUP INT TERM
    curl --proto '=https' --tlsv1.2 --fail --silent --show-error https://sh.rustup.rs -o "$installer"
    sh "$installer" -y --profile minimal --default-toolchain none
    . "$HOME/.cargo/env"
fi
