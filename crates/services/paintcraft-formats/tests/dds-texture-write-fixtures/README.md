# DDS typed 2D texture writer specimens

The bounded internal typed writer has independent local conformance evidence.
No public compression/mip option is activated by these files. The existing
single-level RGBA8 encoder is unchanged.

The repository-owned Rust specimen generator is
[`dds_texture_write.rs`](../dds_texture_write.rs). It authors 7×5, 1×17 and 17×1
packed RGBA8 samples directly from coordinates, with opaque BC1 inputs and
straight-alpha/hidden-RGB inputs for RGBA8, BC2 and BC3. Each is encoded with
Linear and sRGB declarations and both filters, giving 48 containers. The input
bytes are already declared to have that sample transfer. These low-level tests
do not retag a display/sRGB document as linear; a future consumer must convert
or obtain explicit assignment consent.

The explicit `dds-uniform-lossy-samples-v1` policy offers selected Normal and
High effort, Uniform squared-error objective, no dithering and no parallel
encoding. It promises no universal maximum/RMS color error. There are 48
specimens per effort in `normal/` and `high/`, with six deduplicated packed
sources in `sources/`. The two qualities use byte-identical input sources. BC1 cutout, premultiplied/custom alpha,
BC4/5/6H/7, swizzles, arrays, cubemaps, volumes and stored custom-mip preservation
remain unqualified. Full mip geometry uses floor-halving with a minimum side of
one. Nearest chooses the source pixel containing the destination center. Box
computes exact area overlaps directly from the base, averages alpha and
alpha-weighted stored RGB, rounds nearest with ties up, and emits zero RGB if the
alpha sum is zero. This is stored-sample filtering, including for sRGB; it is
not linear-light filtering. Actual nonzero hidden RGB ignored by generated Box
mips is reported separately from block compression loss.

`verify.py` is an independent Python implementation of the original DX10 header,
complete mip layout and rational filter reference. Every mip is decoded by
Pillow from a separately authored single-level header. Original sRGB DXGI tags
are asserted independently. If Pillow does not accept that original tag, its
negative result is retained, and a new UNORM wrapper is used only to verify BC
payload pixels. This is not evidence that Pillow traversed the original chain
or accepted its original sRGB declaration. RGB error is measured at every pixel,
including alpha zero. The original exploratory joint target was maximum RGB
error 96 and RMS 32, with alpha errors 0/8/36 for BC1/2/3. Both efforts failed
that target on six 17×1 Nearest level-one cases (three BC formats, two transfer
declarations). Their `initial-quality-report.json` files remain negative; no
source pixel or original threshold was changed.

The exact finite proof in `palette_bound.py` explains the policy decision. For
the four problematic pixels, enumerate all palette-index assignments and each
channel's RGB565 endpoint pair, using independently checked Pillow integer
interpolation. Even ignoring whole-word endpoint ordering, component error at
most 96 requires second-block RGB squared error at least 24819 (four colors) or
25286 (three colors). The whole eight-pixel mip's RMS 32 budget is only 24576,
even if its first block were perfect. Default black cannot fit any problematic
pixel within component error 96. Thus the joint target cannot be met under this
verified palette interpretation. Relaxing endpoint ordering makes this a lower
bound, not a claim that any endpoint combination attaining it is a legal block.
The earlier continuous endpoint analysis omitted quantization/interpolation and
is retained privately; it is not substituted for this finite proof.

The new policy qualifies standard block decoding, exact layout/filter behavior
and measured lossy output, rather than falsely promising that impossible joint
target. Both `conformance-report.json` files explicitly record
`initial_targets_met:false` alongside frozen decoded hashes and all original
metrics. `verify.py --policy uniform-lossy-v1` requires those independent frozen
pixel goldens; an unreviewed all-black or different compressed result cannot
silently pass. Without that explicit new policy the original target still
returns failure. No vendor patch or new codec was introduced.

Normal measured maximum RGB error 116 and worst mip RMS 32.5064096. High reduced
the BC1 problematic mip's maximum to 96 and RMS to 32.4858944; BC2/3 retained
116/32.5064096 there. High improved the BC3 alpha error on that mip from 9 to 1.
Across these vectors the alpha maximum was 11; this is not a universal alpha
error guarantee. These findings describe actual quantization/quality tradeoffs,
not lossless compression or exact attainable palette optimality.

## Logical workspace audit

Selected dependency is `dds` 0.2.0 from the root lockfile, crates.io checksum
`e917a58e17b27d2f15626c490754ccced7f46592dc5b5f39a3013b89cba5f068`. Audited source paths
are relative to that exact crate archive:

- `src/encode/uncompressed.rs:427–432`: the RGBA8 encoder lists the exact
  `Encoder::copy(RGBA_U8)` path first.
- `src/encode/encoder.rs:220–230` chooses that exact precision path.
  `copy_directly` at lines 264–279 writes a contiguous U8 view directly to the
  sink. It allocates no heap row or float conversion buffer; the fallback stack
  buffer is outside the heap payload estimate.
- `src/encode/bc.rs:37` allocates one encoded block row. `write_util.rs:21`
  allocates four rows of `[f32;4]`. `bc.rs:79` allocates a 4×4 partial-block
  buffer. The plan conservatively prices `64*width + 16*ceil(width/4) + 256`
  bytes for BC1/2/3; BC1's actual encoded block is 8 bytes.
- Automatic resize and parallel encoding are disabled. The adapter allocates
  only one generated mip at a time, directly from immutable base bytes. All
  dimensions, exact output offsets/block tails, filter-work and payload costs
  are checked before admission and codec work.

This prices borrowed source, private output, one mip, selected heap payloads and
plan records. It excludes allocator metadata, stack and physical RSS. Upstream
fixed bounded Vec allocations are infallible; `catch_unwind` is a panic boundary,
not recovery from allocator abort or an OS memory sandbox. Sink and selected
progress callbacks check cancellation; no real-time cancellation bound is claimed.

## Actual oracle and controls

Pillow 13.0.0, including compiled `_imaging` SHA256
`3b2c5b011e928b71339ccc938c75363562bcecca26a5520f42af6d314569ac9f`,
checked 96 containers and 416 mip surfaces. Each report records Python and
plugin hashes. The 24 sRGB files per effort had six original RGBA8 wrappers
accepted and eighteen BC wrappers unsupported by Pillow; the latter only gain
payload-pixel evidence after explicit independent UNORM rewrapping. Original
sRGB DXGI tags are separately asserted, never claimed as Pillow successes.
All uncompressed mip bytes match the independent Fraction reference exactly.

Rust controls cover refusal before admission/encoding, incorrect lengths,
explicit transfer/alpha requirements, BC1 opacity, geometry/mip counts, longest
skinny layout and full block tails, old RGBA8 writer byte identity, actual hidden
RGB loss, cancellation and immutable source bytes. Ordinary integration tests
encode both efforts and compare all 96 externally measured frozen files.
No source/reference codec is executed by that ordinary byte-drift test.

The private record `/private/tmp/paintcraft-dds-texture-write-w5tou9na` retains
pre-run source hashes, original failing logs, all Normal/High artifacts, source
identity checks and timing diagnostics. Same-artifact Normal replay matched all
original encoded bytes. One-shot debug adapter-call times summed 14.101 ms for
Normal and 36.504 ms for High over these tiny cases. Source construction and
result destruction were outside those diagnostic spans; there were no repeats
or statistical benchmark. No optimization or general performance claim follows.

## Reproduction

Use the existing pinned Pillow environment; this is not a product Python
dependency. From this fixture directory:

```sh
/absolute/pinned/python verify.py normal --policy uniform-lossy-v1 \
    --expected-report normal/initial-quality-report.json --report /absolute/new/normal.json
/absolute/pinned/python verify.py high --policy uniform-lossy-v1 \
    --expected-report high/initial-quality-report.json --report /absolute/new/high.json
/absolute/pinned/python palette_bound.py normal > /absolute/new/palette-bound.json
cargo test -p paintcraft-formats --locked --offline --test dds_texture_write
```

Omit `--policy`/`--expected-report` to reproduce the initial joint-target failure.
For fresh evidence, set `PAINTCRAFT_DDS_WRITE_VECTORS` to a new private directory
and `PAINTCRAFT_DDS_WRITE_QUALITY=Normal` or `High`; writes use `create_new` and
cannot replace existing evidence. Ordinary tests always encode both qualities;
only evidence emission is opt-in.

Microsoft's [DDS header](https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-header)
and [BC1–BC3 descriptions](https://learn.microsoft.com/en-us/windows/win32/direct3d10/d3d10-graphics-programming-guide-resources-block-compression)
provide the container/block definitions. All source pixels and specimens here
are repository-owned; selected codec outputs carry the same owned pixel data.
DX10 fresh output is the admitted increment. Fresh legacy headers, BC1 cutout,
other BC/alpha/swizzle/filter profiles, original application/platform coverage
and actual frontend export options remain separate work, not completed by this
local backend proof. Foreign preserving Save remains denied.
