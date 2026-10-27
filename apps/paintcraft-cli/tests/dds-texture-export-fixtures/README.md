# Actual headless DDS copies

`../dds_texture_export.rs` drives the real CLI `render` and MCP `export_image`
interfaces. Absent DDS options retain the old single-level RGBA8 route byte for
byte. Explicit options select the native Studio texture route. The test creates
owned PCND sources, then emits 16 magenta texture profiles (RGBA8/BC1/BC2/BC3,
sRGB/linear8, nearest/Box) and one MCP linear8 conversion. BC uses High effort;
the independently qualified backend fixtures separately cover Normal/High and
nonuniform compression errors. One additional CLI case uses the existing opaque
7×5 source fixture with BC1/sRGB/full mips/BoxStored/Normal and compares the
WHOLE output to its independently verified DDS golden. It asserts that the
BoxStored golden differs from Nearest, so ignoring the selected filter is
observable. The retained uniform consumer cases do not establish a
general BC error bound.

The MCP source `[128,188,255,255]` becomes `[55,128,255,255]` through the facade's
pinned sRGB-to-linear8 policy. Inspection of the original DXGI, alpha, geometry,
and mip fields precedes the independent Pillow check of every mip. sRGB BC
payloads are rewrapped as UNORM only for block-pixel verification; that does not
qualify Pillow's sRGB wrapper support or prove the transfer from tags alone.
The conversion LUT's independent arithmetic proof is in the formats crate's
`tests/dds-transfer-fixtures`.

Run ordinary deterministic consumer tests:

```sh
cargo test --offline --locked -p paintcraft-cli --test dds_texture_export
```

Retain a new, unique directory of actual files and run the independent oracle
using an already installed Pillow environment (no product dependency):

```sh
DDS_PROOF_DIR=$(mktemp -d "${TMPDIR:-/tmp}/paintcraft-dds-consumers.XXXXXX")
PAINTCRAFT_DDS_CONSUMER_OUTPUT="$DDS_PROOF_DIR" \
  cargo test --offline --locked -p paintcraft-cli --test dds_texture_export
/path/to/pinned-pillow/python verify.py "$DDS_PROOF_DIR" \
  --report "$DDS_PROOF_DIR/pillow.json"
```

The local proof used Pillow 13.0.0, `_imaging` SHA-256
`3b2c5b011e928b71339ccc938c75363562bcecca26a5520f42af6d314569ac9f`,
for 17 whole files / 68 exact mip surfaces. Raw files, source/binary receipts,
and the initial failed Clap argument-group integration attempt are retained
privately. No Windows graphics application, release-platform matrix, GPU sampler,
or performance benchmark is claimed.

The same tests cover default byte parity, missing actual loss consent, invalid
or unknown settings, source hash mismatch, non-DDS options, unsupported worker
count, Classic refusal, captured-source aliases and renamed-source identity,
and unchanged MCP document/saved state. Unit controls in `commands/dds_export`
cover cancellation during admitted preparation and at publication, rejected
resource admission, changed destinations, late hardlink aliases, stage cleanup
and budget release. They preserve the successful-publication acknowledgement
boundary: no cancellation/source recheck follows file replacement.
