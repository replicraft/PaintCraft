# HEIF reader preparation

This module inspects bounded item metadata, resolves finite `iloc` extents only
inside `mdat`/`idat`, and checks grid descriptors against declared tile geometry.
Production code does not decode HEVC, validate coded SPS agreement, assemble
pixels, apply transformations or convert color. No frontend capability is
enabled, and HEIF writing remains prohibited.

Inspection follows the explicit `pitm` primary. It retains ordered `dimg` tile
references, directional alpha `auxl` and `prem` links, essential-property flags,
container-declared `ispe`/`pixi`/`hvcC` values, original ICC/NCLX and ordered
rotation/mirror/aperture properties. Unknown auxiliaries are not opacity.
Unknown essential properties, absent IDs, ambiguous primary/alpha declarations,
cyclic derivation and malformed property extents are refused. Unknown property
payloads remain inert borrowed bytes.

The existing `paintcraft-paint` BMFF walker bounds source bytes, box count/depth
and traversal allocations. This layer additionally limits leaf metadata to
4 MiB, 256 items, 1024 properties, 4096 references/associations, per-image declared
pixels to the caller's limit capped at 4,194,304, and aggregate declarations to
four times that limit. Finite payloads total at most 64 MiB across 4096 extents.
External data references, construction method 2, implicit lengths and partial
extent overlaps are refused; exact storage aliases are explicit. These
declarations do not constrain actual HEVC buffers.

## Candidate source inspection, 2026-10-25

The selected `heic-decoder` is pinned as a development-only dependency with its
`internals` feature; no runtime consumer or encoder dependency was added.
Downloaded candidate packages were inspected privately:

| Candidate | Exact package evidence | Relevant API gap |
| --- | --- | --- |
| `heic-decoder` 0.1.1, MIT OR Apache-2.0 | Crate SHA-256 `61c012c388f9a8e5f2e611d67cfca513d4735fd298b648a9c5a61859e34d9bcf`; VCS `a1afbf4a558fccf3b82d5e0a60053a48a9b2b0f6` | `DecodeOptions` only toggles transforms. `probe` parses the first tile's SPS, then reports the grid output extent. Its fixed 2^28-pixel limit cannot enforce PaintCraft's smaller per-tile/aggregate quota. |
| `heifer` 0.2.0, MIT OR Apache-2.0 | Crate SHA-256 `397b5f8e1b211f3f5af885e744b3c9646f1dae6ba22815854f6e28d1cdb43167`; VCS `515c7a7fd67599c29b4e52e8b77fbc0674f7f24e` | Exposes pixel/thread options, but unconditionally depends on its HEVC encoder. Neither candidate exposes caller allocator or cooperative cancellation controls. |

Primary source references:

- [heic-decoder source at the evaluated revision](https://github.com/Multipad-cyber/heic-decoder-rust/tree/a1afbf4a558fccf3b82d5e0a60053a48a9b2b0f6), particularly `src/isobmff.rs` and `src/lib.rs`.
- [Heifer at the evaluated revision](https://github.com/HarpeLm/Heifer/tree/515c7a7fd67599c29b4e52e8b77fbc0674f7f24e), particularly `crates/heifer/Cargo.toml` and `src/lib.rs`.

## Development-only decoder qualification

The owned [HEVC fixtures](../../tests/heif-fixtures/README.md) are independent
FFmpeg/libx265 encodes. The selected decoder matches every native gray8 sample
for a non-first primary, an ordered two-tile grid, and auxiliary alpha after
container admission and checks of every coded item's SPS. This establishes only
these tiny trusted monochrome fixtures, not a qualified general HEIF reader.

Before invoking the parameter parser, qualification caps each SPS NAL at 65,535
bytes. Source inspection at the pinned revision covered all reachable parameter
paths: 64 short-term reference sets; at most 16 direct negative/positive picture
references and 17 transient predicted entries; scaling lists with 24 fixed
vectors of at most 64 coefficients; fixed-size scan orders; single temporal
sublayer; VUI/HRD loops of at most two sets of 32 entries; and Exp-Golomb prefixes
bounded at 32 bits. Long-term references and unsupported coding tools are
refused. This parser audit is not a hard allocator or cancellation-tail proof.

The development helper checks all SPS coded dimensions before frame decoding,
then bounds their aggregate, compares cropped extents and `hvcC`/`pixi` depth
with coded values, and requires consistent grid tile depth/chroma.
It never assumes the public probe's first tile represents every tile. Hostile
frame/PPS decoding has not been qualified; for example, the pinned PPS tile-count
parser still has unchecked `ue()+1` arithmetic. No malformed frame test uses that
decoder. Runtime confinement, complete format/transform/color/10-12-bit oracles,
and production SPS admission remain required before any frontend read route.
