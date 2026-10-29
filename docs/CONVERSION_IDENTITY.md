# Converted raster document identities

DCX page imports and native HDR/EXR conversions now bind document identities to
both the selected conversion and its actual RGBA8 output. This prevents a later
conversion of the same source from reusing the document, state and layer IDs of
a different result. It changes newly constructed conversion documents, without
changing the global image-import API, existing PCND identities or source guards.

## Demonstrated collisions

Previously, these routes passed only the original file's SHA-256 as the
`import_image` construction key:

- Desktop Studio, browser Studio, CLI and MCP could select different DCX pages
  from the same file. Their pixels differed, while their document/state/layer
  IDs were identical.
- The shared native float importer could convert the same HDR source using
  clipping or Reinhard tone mapping. Independent expected pixels differed, but
  the converted documents had the same IDs. HDR/EXR settings also include other
  choices that are already bound by the sealed conversion assessment.

A state ID can participate in render-cache lookup. Reusing it for different
pixels is a correctness problem, even when each decoder returns correct pixels.
The regression tests first reproduced the old behavior through the actual
consumers, then passed with the conversion keys applied.

## Versioned identity contract

`paintcraft_document::RasterConversionKey::checked` hashes this explicit sequence
using SHA-256. Integer fields use little-endian encoding.

| Field | Framing |
| --- | --- |
| Domain | Exact `paintcraft.raster-conversion-key/v1\0` bytes |
| Versioned profile | `u16` byte length, then profile bytes |
| Original source SHA-256 | Exactly 32 bytes |
| Selection | Tag 0 for image; tag 1 followed by a `u16` page index |
| Optional assessment digest | Tag 0 for absent; tag 1 followed by 32 bytes |
| Width and height | Two `u32` values |
| RGBA byte count | One `u64` value |
| Output samples | Exact straight RGBA8 bytes in row order |

Profiles are nonempty, contain at most 128 ASCII graphic bytes, and describe a
versioned conversion policy. DCX uses
`paintcraft.dcx-selected-page-rgba8/v1` with its explicit page index. Native
float conversion uses its existing HDR/EXR decoder profile and the sealed
assessment digest, which already binds settings and the reported conversion.
Hidden RGB beneath zero alpha contributes to the key independently of alpha.

Repeated source bytes, choices, geometry and output samples produce the same
key across the DCX frontends. Different sufficient caller pixel allowances do
not change it. Paths, timestamps, temporary budgets and UI session identities
are excluded. Float assessments distinguish different authored policies even
when those policies happen to produce equal pixels.

## Bounds and authority

The constructor checks nonzero dimensions against the canonical side limit of
1,048,576, checked pixel arithmetic against the caller's allowance capped at
268,435,456 pixels, and the exact `width * height * 4` sample length. These are
identity-construction bounds; individual decoder and import profiles retain
their existing smaller limits and separate resident/transient admission.

Samples are hashed in 65,536-byte chunks without constructing another full
pixel buffer. The caller's cancellation check runs before validation, before
each chunk and after finalization. A rejected extent, malformed profile or
cancelled operation returns no key.

The key is only a document-construction identity. It does not authenticate
caller-supplied digests, validate a conversion assessment, grant format
capabilities, acknowledge losses, reserve resources or authorize publication.
The original raw source SHA-256 remains in source provenance and filesystem
freshness/alias guards. It is not replaced by the converted-document key.
Existing owner, consent, source and cancellation checks still run before
adoption.

## Validation scope

Local headless checks on macOS ARM64 passed these separate scopes:

- Document tests: 10 passed, including two bounded-key controls for field
  separation, a separately serialized hash vector, hidden RGB/alpha, geometry,
  deterministic sufficient budgets, malformed input and cancellation.
- Native DCX tests: 6 passed, including actual page adoption, stable distinct
  document/state/layer identities, stale owners, cancellation and translated UI.
- Browser-host DCX tests: 5 passed, including the actual frontend conversion
  path, explicit selection/consent and distinct/repeated page identities.
- Compiled CLI and stdio MCP integration: one test passed, exercising three
  page choices through each transport, exact samples, cross-transport identity
  agreement and unchanged original source bytes.
- Actual installed float identity fixture: four conversions passed against a
  verified same-build V10 helper, including independent clipping/Reinhard pixel
  oracles, a repeated choice and an equal-output/different-policy choice.
- Existing actual float guard matrix: 12 independent HDR/EXR cases passed,
  covering exact samples and PCND, raw/report bindings, digest/loss refusal,
  source/owner changes, cancellation and budget release. Its historical test
  name contains `v7`; this run used the verified compatible V10 installation.

All five affected packages passed all-target strict Clippy. The browser library
also passed a Wasm check; repository formatting and source limits passed. The
new installed float identity test is wired into native candidate qualification,
but these local results do not establish that the new gate passed on all six
native targets. Browser-host controls and a Wasm check are not a new actual
browser execution claim for this identity change.

Raw failed, exploratory and successful logs are retained under ignored
`docs/validation/conversion-identity/`. The final source/runtime hash record is
`final-freeze.json`; `duplicate-cleanup.json` separately records removal of only
the redundant archive and installed executable copies after verification.
Canonical build artifacts, manifests, notices, inputs and validation evidence
were retained.

## Remaining scope

TIFF already uses its full selected-page assessment as document identity. SVG
already binds its render choices and output; ICO/CUR and ICNS bind their selected
entries. Those routes were left unchanged. Other fixed-policy image imports
were not globally migrated, and this work does not add formats, preserving Save,
new precision support or full format-roadmap qualification.
