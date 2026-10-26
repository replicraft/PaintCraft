#!/bin/sh
# Install the full verified CLI archive. Requires Python 3.12 or newer.
set -eu
paintcraft_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
exec python3 "$paintcraft_root/scripts/packaging/install_cli.py" "$@"
