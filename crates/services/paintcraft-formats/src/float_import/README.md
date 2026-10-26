# Private float import preparation

`float_import` composes the existing bounded HDR/EXR inspectors and optional decoders with the existing `precision::Binding` and conversion math. `prepare_hdr` exists only with `hdr-codec`; `prepare_exr` only with `exr-codec`. Neither feature is enabled by default. This is private backend qualification, not a worker profile, public format route, UI, CLI or MCP import.

Preparation derives source SHA, geometry, signed origin, pixel aspect, original color evidence, channel types and alpha convention from the inspected source. It creates an immutable policy Binding before decode, so unresolved EXR color requires an explicit source assignment. The sealed assessment retains the exact bounded original header bytes, settings, output hash, actual conversion statistics and every required loss. A cached digest binds the complete assessment, including its raw header and output hash. No unrelated decoded buffer or replacement metadata can be supplied by a caller.

`Prepared::preview_rgba` is explicitly a preview. Consuming `approve(expected_assessment_digest, allowed_losses)` checks the exact assessment/settings digest, rejects unknown or missing loss consent, and returns `Approved`. Actual adoption must require that approved result and independently validate the captured source file/editor owner. This digest does not prove filesystem freshness, source-path identity, active tab, unsaved drafts or publication permission. Those remain native Job/frontend responsibilities; preparation performs no filesystem writes or editor/document mutation.

The assessment's raw header is retained for the receipt, not preserved in a PCND document. Its receipt explicitly marks `metadata_preserved_in_document=false`. Detached JSON receipt storage belongs to the caller's admission. Format-specific limitations supplement the existing precision policy's actual losses:

- `hdr-metadata-not-retained`: original HDR calibration/comments/unknown records, or EXR descriptive/unknown attributes and nondefault screen-window metadata are not adopted into the destination document. Raw values remain in this assessment. Encoding structure and color/geometry already represented by the explicit conversion are distinguished from additional metadata.
- `hdr-pixel-aspect-normalized`: nonunit source pixel aspect becomes square destination pixels without resampling. The exact original aspect is retained as float64 bits; this is a declared physical-display change, not a hidden rescale.
- `exr-origin-normalized`: a nonzero signed source origin becomes destination origin [0, 0]. Pixels and their relative positions remain unchanged; original coordinates remain in the receipt.
- `hdr-linear-alpha-compositing-changed`: retained source alpha contains values strictly between 0 and 1. The receipt names the destination's encoded-sRGB u8 source-over blending and binds the actual count. This can differ from scene-linear source compositing independently of alpha quantization. Opaque sources and explicitly flattened outputs do not receive this limitation.
- `hdr-rgbe-lower-edge-representative`: explicit RGBE reconstruction choice. Exponent zero means black; positive exponent E uses mantissa M times 2^(E-136). This is the selected lower-edge representative, not a claim that it equals original Radiance's midpoint representative.

Radiance samples remain as stored: EXPOSURE and COLORCORR are neither applied a second time nor undone. The original ordered records and calibration values survive in the raw source header receipt. The nominal Radiance equal-energy color space is converted by the existing qualified precision policy; RGB channel names alone never imply sRGB. EXR samples remain as-stored floats until that same policy applies the chosen source-space, alpha, exposure, negative-value and curve settings.

Alpha convention defaults to OpenEXR's premultiplied interpretation and can be explicitly overridden, with consent. Additive color at zero alpha requires explicit linear-light flattening or refusal; it is never silently discarded. Nonfinite color or alpha and alpha outside [0, 1] are refused. Clip and Reinhard-per-channel remain named choices, so the latter's ordinary-white mapping is never substituted silently. This module implements no duplicate compositor, color transform or tone-mapping engine.

Logical accounting uses the same SharedBudget for preflight metadata scratch, source receipt, decoder floats and converted RGBA. A 3 MiB transient metadata declaration covers the bounded preflight offset/interval inventories (up to 65,536 entries), original-header processing and internal receipt/digest construction. A separate declaration covers the retained source header's actual capacity plus bounded descriptor/loss storage. The decoded float buffer remains charged while RGBA conversion allocates and runs; both coexist under that one budget. On return, decoder floats/scratch and transient metadata release, leaving only RGBA plus retained assessment metadata. Approved results retain those accounts until drop. Cancellation, policy refusal or failed consent releases all private storage.

These are logical declarations, not heap/physical RSS limits. Source input storage remains caller-admitted. EXR's one-shot scoped decoder thread joins before its scratch charge releases; join is not killable cancellation or a hard deadline. Real process confinement, protocol/owner/source guards and actual consumer workflows remain required before any public import route.

Independent end-to-end expected pixels live in `tests/float-import-fixtures`. Their generator reads the existing official OpenEXR-reopened float references and independent Radiance lower-edge references, then uses the separately qualified Decimal100/NumPy policy oracle. It never invokes the selected Rust decoder or Rust conversion implementation. Tests cover both curves, exposure, assignments, calibration provenance, signed origins, both formats' pixel aspect, additive-alpha refusal/flattening, explicit straight-alpha override, nonfinite refusal, exact consent, and cancellation while float and RGBA allocations coexist.

The compositing limitation is confirmed against the actual editor `Rgba::source_over` API. Scene-linear premultiplied RGB .25 with alpha .5 becomes straight sRGB [188,188,188] with A128 when transparency is retained; the current encoded-u8 source-over compositor produces [94,94,94] over black. Explicit scene-linear flattening over black instead produces [137,137,137] with A255 and no retained-alpha compositing limitation. The generic precision conversion and editor compositor are unchanged. The assessment reports this difference instead of implying preservation of source compositing.

Historical initial-preparation validation: five targeted float-import tests covered all seventeen independent reference cases and the actual compositor counterexample. `cargo test -p paintcraft-formats --features exr-codec,hdr-codec --lib float_import:: --offline --locked` and the corresponding `cargo clippy --lib --tests -- -D warnings` pass, as do scoped Rust formatting and diff checks. The independent oracle source/golden hashes and the exact-once entrypoint guard are checked.

## Parent source plan and finite payloads

Pure `plan_hdr` / `plan_exr`, `SourcePlan`, `WorkerAssessment` and `wire` are available without either decoder feature. Planning runs bounded structural inspection and creates the exact Binding/header/source-loss description, never floating-point decoding. The initial private parent profile is limited to 16 MiB source and 1,048,576 pixels; this narrower worker prerequisite does not replace the standalone decoder qualification bounds or define permanent format support. Actual `prepare_hdr` / `prepare_exr` remain feature-gated.

`wire::encode_settings` and `decode_settings` use exactly 32 bytes, with no implicit child defaults:

| Offsets | Meaning |
| --- | --- |
| 0..8 | `PCFLTS01` magic |
| 8..10 | big-endian schema 1 |
| 10, 11 | Clip/Reinhard and Reject/ClipToBlack finite tags |
| 12, 13, 14 | Preserve/Flatten, optional source space and optional alpha assignment |
| 15 | zero reserved byte |
| 16..20 | signed big-endian exposure milli-EV, -16000..16000 |
| 20..23 | flatten backdrop RGB; all zero for Preserve |
| 23..32 | zero reserved bytes |

A `Report` encodes exactly 96 bytes: `PCFLTR01` at 0..8, schema1 at 8..10, zero reserved bytes at 10..16, assessment digest at 16..48, five big-endian u64 statistics at 48..88 in Statistics declaration order, and retained original partial-alpha count at 88..96. Unknown tags, versions, reserved/inactive values and wrong byte lengths refuse. This defines no active worker opcode or helper profile.

`SourcePlan::accept_report` accepts only the expected dimensions and exact RGBA extent. It bounds RGB statistics by three times the source pixel count, alpha statistics by pixel count, checks policy-dependent zero values and opaque output, then derives required losses with the same classifier used by actual conversion. The parent constructs the raw source metadata and canonical assessment itself and compares the child's digest. No worker JSON or arbitrary worker loss list can replace those source facts. Original float statistics are bounded semantic reports from the qualified decoder, not independently decoded proofs on the host.

The source partial-alpha count is not the output count of A1..254: values near zero or one can quantize to A0 or A255. The validator requires output-partial count no greater than retained source-partial count, and alpha quantization count no greater than retained source-partial count. Opaque/flattened output requires both zero. Additive-zero-alpha statistics are permitted only for effective premultiplied alpha with explicit flattening. Tests cover near-endpoint alpha and impossible semantic reports even when a matching canonical digest is forged.

The accepted WorkerAssessment retains metadata admission; the matching RGBA storage and source/file/editor freshness guards remain the caller's responsibility. Reconstruction declares transient metadata/digest scratch before allocating receipts. No default codec feature, PCDEC002 header, legacy codec, browser route or helper build contract changes in this layer.

Current source-plan/payload validation: nine targeted float-import tests pass, including reconstruction against the seventeen independent fixtures, finite settings/report checks, matching-digest semantic forgeries and near-zero/near-one alpha. Three precision-policy tests pass after extracting the shared loss classifier. Strict Clippy passes with both decoder features and without either feature; default, HDR-only and EXR-only checks pass. This remains a qualified-worker reconstruction seam: the host bounds and binds reported original-float statistics without independently decoding them. No native worker opcode, default helper or public format gate is enabled by this foundation.
