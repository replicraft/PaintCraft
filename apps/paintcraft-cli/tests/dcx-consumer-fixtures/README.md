# Production single-page DCX qualification

The real CLI and MCP production routes import a captured PCND document/session,
then export exactly one DCX composite page using explicit RGB24, exact indexed8 or
requested quantized256 PCX mode. The six files recorded in `imagemagick.json` were
produced by `tests/dcx_export.rs` through the actual binary and stdio tool router,
not by calling the typed backend directly. All modes retain the six authored RGB
samples exactly in this four-color specimen; indexed modes retain the first-seen
four-color palette and raw indices `[0,1,0,2,3,1]`, with a black unused suffix.

`verify.mjs` passes the entire files to untouched ImageMagick 7.1.2-32 Q8 from
`@imagemagick/magick-wasm` 0.0.44. It verifies one page, dimensions 3×2, DPI 72/72,
all samples, all 256 palette entries and indices. Node was 24.10.0. The package is
qualification-only, not a product dependency. Exact package archive provenance,
SHA512 integrity, wrapper/Wasm SHA256 and the broader mixed multipage qualification
are in `crates/services/paintcraft-formats/tests/dcx-fixtures/writer/README.md`.
The script verifies the untouched wrapper/Wasm hashes before loading them.

From the repository root, with that verified archive extracted privately:

```sh
mkdir /private/tmp/paintcraft-dcx-production-owned
PAINTCRAFT_DCX_CONSUMER_OUTPUT=/private/tmp/paintcraft-dcx-production-owned \
    cargo test -p paintcraft-cli --test dcx_export --offline --locked
node apps/paintcraft-cli/tests/dcx-consumer-fixtures/verify.mjs \
    /private/tmp/paintcraft-dcx-production-owned /path/to/private/oracle/package
```

The Rust tests check actual loss refusal/consent and white matte, explicit mode
requirements, unknown losses, unchanged source bytes and MCP document/dirty state,
Classic refusal, hardlinked sources, and successful unrelated copies before/after
renaming the retained native source. A separate private staged-route regression
creates an alias at the publication barrier and proves refusal without changing
source bytes or leaving staged files. Deterministic checks cancel during preparation
and at the final publication barrier, preserving session semantics and the old
destination with no staged residue. The actual stdio client also cancels a large
export RPC immediately and confirms unchanged state/output; it does not assert
that cancellation arrived after rendering started. MCP uses the existing RPC
lifetime and request token through conversion, staged chunks and publication;
filesystem identity protection remains a separate coarse callback. The registry
matrix excludes every other
frontend/platform/operation/variant, including generic Open and preserving Save.

This qualifies the native headless Studio fresh-copy route only. No desktop or
browser DCX export controls, multipage document model, layers-as-pages conversion,
or additional Classic stock capability is claimed. Typed multi-page output remains
available separately inside the formats backend.
