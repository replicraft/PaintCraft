# Install the full verified CLI archive. Requires Python 3.12 or newer.
$ErrorActionPreference = 'Stop'
$paintcraftRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
& python (Join-Path $paintcraftRoot 'scripts/packaging/install_cli.py') @args
exit $LASTEXITCODE
