# Private JPEG XR fresh encoder

The fixed opaque RGB8 writer and its native parent preparation are implemented
with locally qualified seekcap2 encoding. The new jxrread1 import mode is source-only and unexecuted. Public export, Studio controls, CLI/MCP commands,
foreign preserving Save and default/release promotion remain unavailable.
The native import profile is a separate existing operation.

## Exact candidate

- Feature: `paintcraft-codec-worker/jxr-encode`, explicitly including `float-codecs`.
- Profile: `private-native-v8-jxr-rgb8-lossless-seekcap2-jxrread1-parent-lease1`.
- Record: `PAINTCRAFT_CODEC_WORKER_BUILD_V8_JXR_ENCODE_PRIVATE`.
- Manifest: exact schema 3, retaining seven existing import variants plus the
  distinct private `native-jxr-writer-rgb8-worker-v1` import and separately
  declaring `native-jxr-rgb8-lossless-worker-v1` in `qualified_export_variants`.
- Encode request/reply: `PCENC001` / `PCJXR001`, schema 1, fixed 176/208-byte
  headers. These are not decoded-RGBA frames or extensions to `PCPIX002`.
- Production decoder discovery still accepts only its exact V4–V7 contracts and
  rejects this candidate. The default helper remains V7; `--jxr-encode` exists
  only in the optional private build.

Request geometry, packed stride, exact RGB length, output cap, owner/nonce and
source hash are bound by a canonical domain-versioned digest. Reply validation
checks bindings, application version, heap declaration, encoded length/hash and
strict fresh JXR headers before returning a private view. Header/hash checks are
structural/provenance validation, not independent entropy or pixel proof.

## Fixed profile and resource scope

Input is packed unsigned opaque RGB8 only, at most 1,048,576 pixels and
3,145,728 bytes, each side at most 65,536. There is no RGBA entrypoint or silent
alpha discard. The selected encoder uses fixed lossless YUV444, spatial order,
quantizer zero and no overlap. It emits raw positive-zero FLOAT density fields;
T.832 interprets those as effective 96 DPI, not explicit stored 96.0 values.
Alpha, quality/depth choices, ICC/HDR and broader writer profiles are unqualified.

The child validates framing and exact source extent/hash before codec work,
reserves the requested output capacity fallibly under its existing allocator,
and encodes through the reviewed capped seekable memory cursor. Output is at
most 64 MiB. No output path or predictable/reopened temporary file is used.
Only a completed, validated file is streamed into the typed reply, without a
second whole-file reply copy. The source binding is checked again after encoding.

The existing requested Rust/selected-C allocation ceiling is 256 MiB; it is not
physical RSS, native allocator overhead, mappings or thread stacks. Parent
admission prices that child ceiling once plus distinct parent RGB, header,
response and IO buffers; original document/composite storage is separate.
Supervised cancellation/deadlines terminate and reap the child. A sole stdin
writer supplies its parent lifetime lease after complete input; parent death
while blocked on input instead closes the pipe and ends the read. Neither is a
process-tree/filesystem sandbox or a hard cancellation-tail guarantee.

## Actual local evidence

Qualification is local macOS Apple Silicon evidence, not six-target or original-application qualification.

The corrected optimized **seekcap2 encoder** producer was built from immutable
source `3e2b65db389495d7c786c2884f832b8b2158f873`. Its SHA256 is
`2e9def24ee4adc6867b2fee7d3eba5d3ffb292f4c5fdad8c92883b5cc48ed363`,
with manifest SHA256
`204f2cf5c3bea2dcbed17535b8a014ceadb5c3e4d2bfa69429061777d0bc4bfa`.
It has the historical seekcap2 profile, **seven** imports, and no jxrread1 mode.
The producer, source/build/record/notices receipt and raw matrix logs remain at
`/private/tmp/paintcraft-jxr-long-release-mgr_c7w6`.

- All fourteen small and fifteen maximum/skinny/noise vectors matched every RGB
  byte after whole-file decoding by untouched pinned C JXRLib. The fifteen large
  vectors include 1024×1024 and 65536×16/16×65536 geometry; each child/C case
  retained the unchanged ten-second deadline.
- Typed cap controls at 1/133/341 bytes returned `ResourceLimit` with no encoded
  payload; the exact 342-byte cap produced byte-identical successful output.
- The existing actual parent fixture passed in 4.65 seconds against that helper,
  with its unchanged twenty-second job deadline and four independent small C
  outputs, owner/hash/lease/publication and manifest controls. Parent evidence:
  `docs/validation/jxr-encode-parent-long-header-2026-10-26/051dd835-6d10-4367-900c-049b148bef30`.

The earlier optimized `a310250a…` producer stopped at 65536×16 because its
writer-output inspector wrongly required abbreviated geometry. That failure and
its unrun remainder remain separate from the corrected complete matrix. The
writer-only inspector correction did not widen ordinary public JXR import.

The first private seekcap1 debug producer
`bf364cfe171df011d224f5f95637089b97c96dcdf1ae4c30cc326fcf2af62c5c`
remains historical evidence: fourteen small child/C vectors (largest 257×129),
four parent/C vectors and process controls. Its untyped sink cap errors were
`CodecFailed`; subsequent typed `OutputLimit` work introduced seekcap2 and maps
only that typed condition to `ResourceLimit`. Callback IO/panic errors remain
`CodecFailed`; global requested-heap exit 75 remains `ResourceLimit`.
Earlier parent expectation/read-only-fixture failures are also retained.

The current **seekcap2+jxrread1 candidate is not yet qualified**. Its exact new
mode token/eighth import and dedicated `--jxr-writer-source` dispatch require a
fresh immutable producer, child/C matrix and parent import execution. Existing
artifacts must not be relabeled as this candidate. Its explicit assessment
requires fresh v1, positive-zero density fields, fixed RGB8 planes/flags, either
measured geometry header, at most 1MP/65536 sides and 64MiB encoded input.
Ordinary `inspect_checked`/`assess_import` stay short-only. Encoded JXR input is
64MiB; 16MiB is the ordinary RGBA response capacity for 4MP.

## Explicit developer reproduction

Build into a separate private target directory to preserve the default helper:

```sh
cargo test -p paintcraft-codec-worker --features jxr-encode \
    --target-dir /absolute/private/target --test native_jxr_encode \
    --offline --locked --no-run
```

Run the resulting cached test executable serially after other heavy jobs finish.
The independent case is explicitly ignored by default; select
`cases::actual_child_all14_fresh_rgb_vectors_match_independent_c_jxrlib` with
`--ignored --exact --nocapture` and set `PAINTCRAFT_JXR_REFERENCE` to the pinned
C executable. Its exact expected hash and original source provenance are in the
[JXR fixture record](../../../tests/jxr-fixtures/README.md); it is a development
oracle, not a shipped C encoder/decoder fallback.

The parent fixture is explicitly ignored too:
`codec_worker::native::jxr_encode::installed_tests::actual_private_installed_encoder_parent_lifetime_and_pixels`.
It requires `PAINTCRAFT_JXR_ENCODE_HELPER` pointing to a hash-frozen private
producer with the exact adjacent schema 3 manifest, plus the same explicit C
reference. A public V7 helper must never be relabeled as the candidate.

Next qualify the new private jxrread1 candidate and its parent import path;
optimized seekcap2 encoder matrix evidence does not qualify that new reader. Windows WIC/Paint.NET and all
six native release targets remain unproved. Local C differential evidence does
not establish full JPEG XR or original-application compatibility.
