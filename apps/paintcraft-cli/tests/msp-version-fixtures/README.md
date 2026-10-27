# Actual MSP version consumers

`../msp_versions.rs` drives the real CLI `render --msp-version v1|v2` and MCP
`export_image` optional `msp_version` field. Absence retains the old MSP1 route;
explicit V1 and V2 retain both admission leases and the prepared bytes through
staged publication. No UI or browser version selector is qualified by this test.

Both consumers export the same two owned, nonuniform opaque black/white sources:

- 2048×3: packed rows of 256 zero bytes, 256 white bytes, and bytes 0..255. The
  entire V2 file must equal the independently qualified formats-crate specimen
  `msp2-writer-fixtures/runs-2048.msp`.
- 9×3: canonical packed rows `80 80`, `FF 80`, `55 00`, with unused tail bits zero.
  This exercises odd-width padding without claiming original unused-bit retention
  when a new RGBA document is packed.

The default and explicit V1 files are byte-identical; V1/V2 headers/bytes differ.
The independent `verify.py` parses complete headers, checksum, every row length
and literal/repeat packet, compares all packed bytes, then asks the untouched
pinned Pillow 13.0.0 MSP reader to compare every visible RGBA pixel. Its plugin
SHA-256 is `8e28773c59bae649f034c22b68d64ae4cdc9c26605947e34114e254dd458f8e6`.
Fourteen complete files cover the two consumers, default/V1/V2 and an unrelated
MCP export after a renamed-source alias refusal. Each MCP fixture opens a bound
PCND, then uses actual `apply_operations` to rename its layer before exporting.
The pixels stay unchanged while dirty becomes true and history gains an entry.
The complete inspection result remains equal after every export and alias
refusal; source binding is exercised through protected renamed-file aliases
before and after the positive exports. No Save is used to clean that state. This is local CPU/converter
qualification, not original historical Windows Paint compatibility.

```sh
cargo test --offline --locked -p paintcraft-cli --test msp_versions
MSP_PROOF_DIR=$(mktemp -d "${TMPDIR:-/tmp}/paintcraft-msp-consumers.XXXXXX")
PAINTCRAFT_MSP_CONSUMER_OUTPUT="$MSP_PROOF_DIR" \
  cargo test --offline --locked -p paintcraft-cli --test msp_versions
/path/to/pinned-pillow/python verify.py "$MSP_PROOF_DIR" "$MSP_PROOF_DIR/pillow.json"
```

Actual failures cover unknown versions, wrong families/extensions, Classic
export, unsupported worker count, source hash mismatch and direct/renamed source
hardlinks. Color, gray and transparency remain refused with no threshold/dither/
matte; exports preserve MCP document/history/dirty/saved binding. Unit controls
exercise cheap cancellation, retained two-stage reservations, publication source
protection and stage cleanup. Existing common flatten/hidden-RGB loss consent
still applies; compression itself adds no pixel loss. Preserving MSP Save and
additional historical metadata compatibility are not enabled.
