# Private JPEG XR fresh encoder

The fixed opaque RGB8 writer and its native parent preparation are implemented
and locally qualified. Public export, Studio controls, CLI/MCP commands,
foreign preserving Save and default/release promotion remain unavailable.
The native import profile is a separate existing operation.

## Exact candidate

- Feature: `paintcraft-codec-worker/jxr-encode`, explicitly including `float-codecs`.
- Profile: `private-native-v8-jxr-rgb8-lossless-seekcap1-parent-lease1`.
- Record: `PAINTCRAFT_CODEC_WORKER_BUILD_V8_JXR_ENCODE_PRIVATE`.
- Manifest: exact schema 3, retaining seven import variants and separately
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

Qualification ran on macOS 14.7.2 Apple Silicon in debug builds:

- Fourteen fresh child encodes matched every RGB byte after whole-file decoding
  by untouched pinned C JXRLib. Vectors cover 1×1 primaries/black/white, gradients,
  odd sizes, macroblock edges, one-dimensional images, markers and seeded noise.
- Four actual parent-runner C cases matched exact pixels and retained source,
  owner/request/output bindings and lease/publication-preparation guards.
- Six nonignored child controls passed: exact record/import declarations;
  capped refusal; malformed/cross-direction input; cancellation/deadline/output
  bounds; actual parent death with the sole writer held during blocked input;
  and complete-request lease EOF. The parent adds manifest/pin/tamper and normal
  decoder rejection checks.

The largest independently decoded child image was **257×129**. Admitting one
million pixels does not qualify maximum-size or extreme-aspect encoder behavior.
Pre-reserving 64 MiB and observing requested-heap peaks does not prove worst-case
workspace or physical-memory use. Optimized execution is not yet qualified.

The immutable private producer helper SHA256 is
`bf364cfe171df011d224f5f95637089b97c96dcdf1ae4c30cc326fcf2af62c5c`.
The separately rebuilt default V7 helper SHA256 is
`51bcdda3830a648ee35b90e5868f7700a0392ca687ede2412ee42146e8d29f9b`;
it retains exactly one V7 record, no private V8 record, and rejects the encoder
argument and new frames. These identify actual local artifacts, not reproducible
binary hashes across compilers or release-platform proof.

Raw child/C/parent logs and producer source/record/notices receipts are retained
as local evidence. Initial build/test-harness failures remain separate from the
final successful runs. Earlier parent tests expected the wrong cap error class
and tried mutating a read-only fixture copy; only those test expectations and
copy permissions were corrected. The producer was not changed, deadlines were
not raised, and failures were not relabeled as successful runs.

Capped-sink `Error::Io` currently reports `CodecFailed`: its untyped channel can
also carry callback/panic failures, so no message matching guesses a quota error.
A typed quota-error API is a known follow-up. The existing nonallocating global
heap exit 75 remains `ResourceLimit`.

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

The next qualification must cover optimized maximum/skinny/noise images and
real limits before public staged-copy consumers. Windows WIC/Paint.NET and all
six native release targets remain unproved. Local C differential evidence does
not establish full JPEG XR or original-application compatibility.
