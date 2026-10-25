# Install the CLI from this checkout. Requires the project's Rust toolchain.
$ErrorActionPreference = 'Stop'
$paintcraftRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
& cargo install --locked --path (Join-Path $paintcraftRoot 'apps/paintcraft-cli') @args
exit $LASTEXITCODE
