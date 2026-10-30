# Separate PCND fuzz inputs

`generate.py` retains the four independent Python PCND1/2 fixture bytes by
reference and writes ten derived fixtures. No Rust writer generates these seeds.
The separate manifest records input lengths, SHA-256 and expected target
acceptance, not universal format validity. Its provenance file records every
parent digest and exact transformation recipe.

Three metadata negatives recompute the valid outer checksum so they reach the
actual version, profile-extent and inner-attachment-digest gates. Four encoded
variants retain the independently authored legacy specimen's exact raw RGBA:
RGBA runs, Zstandard 1.5.7 CLI with declared stream size, an independently
constructed LZ4 literal block, and literal LZ4 around RGBA runs. The invalid
three-tile document is a resource-policy rejection, not malformed PCND. The
huge default-only document is a positive sparse-area boundary with zero resident
tiles. No output image, external application or color transform is qualified.

Regenerate using Python standard library and installed `zstd` 1.5.7:

```sh
python3 fuzz/pcnd-fixtures/generate.py
```

Historical and stock manifests, sources and prior receipts are not regenerated.

PCND3 integration changes only `unknown-version.pcnd` from container byte 3 to 4
with a recomputed outer SHA-256. The other thirteen manifest-listed source bytes
are unchanged. Earlier qualification records retain their original v3-negative
hash and do not describe a run of this migrated inventory. The sixteen new
filter-stack fixtures have their own `../pcnd-filter-seeds.json` inventory and
standalone runner; they are not appended to this fourteen-source manifest.
