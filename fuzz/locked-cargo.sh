#!/bin/sh
# cargo-fuzz 0.13.2 does not forward --locked. Keep its build/metadata locked.
set -eu
: "${PAINTCRAFT_FUZZ_CARGO:?Set the absolute path to the selected real Cargo binary}"
case "$PAINTCRAFT_FUZZ_CARGO" in
    /*) ;;
    *) echo "PAINTCRAFT_FUZZ_CARGO must be absolute" >&2; exit 2 ;;
esac
case "${1:-}" in
    build|metadata) exec "$PAINTCRAFT_FUZZ_CARGO" "$@" --locked ;;
    *) exec "$PAINTCRAFT_FUZZ_CARGO" "$@" ;;
esac
