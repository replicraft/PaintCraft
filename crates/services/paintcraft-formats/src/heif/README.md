# HEIF reader preparation

This module inspects bounded item metadata. It does not decode HEVC, resolve
`iloc` payload extents, parse grid descriptors, validate coded SPS agreement,
assemble grids, apply transformations or convert color. No frontend capability
is enabled, and HEIF writing remains prohibited.

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
four times that limit. These declarations do not constrain actual HEVC buffers.

## Candidate source inspection, 2026-10-25

No new codec dependency was added. Downloaded packages were inspected privately:

| Candidate | Exact package evidence | Relevant API gap |
| --- | --- | --- |
| `heic-decoder` 0.1.1, MIT OR Apache-2.0 | Crate SHA-256 `61c012c388f9a8e5f2e611d67cfca513d4735fd298b648a9c5a61859e34d9bcf`; VCS `a1afbf4a558fccf3b82d5e0a60053a48a9b2b0f6` | `DecodeOptions` only toggles transforms. `probe` parses the first tile's SPS, then reports the grid output extent. Its fixed 2^28-pixel limit cannot enforce PaintCraft's smaller per-tile/aggregate quota. |
| `heifer` 0.2.0, MIT OR Apache-2.0 | Crate SHA-256 `397b5f8e1b211f3f5af885e744b3c9646f1dae6ba22815854f6e28d1cdb43167`; VCS `515c7a7fd67599c29b4e52e8b77fbc0674f7f24e` | Exposes pixel/thread options, but unconditionally depends on its HEVC encoder. Neither candidate exposes caller allocator or cooperative cancellation controls. |

Primary source references:

- [heic-decoder source at the evaluated revision](https://github.com/Multipad-cyber/heic-decoder-rust/tree/a1afbf4a558fccf3b82d5e0a60053a48a9b2b0f6), particularly `src/isobmff.rs` and `src/lib.rs`.
- [Heifer at the evaluated revision](https://github.com/HarpeLm/Heifer/tree/515c7a7fd67599c29b4e52e8b77fbc0674f7f24e), particularly `crates/heifer/Cargo.toml` and `src/lib.rs`.

Next gates are actual decoder selection; complete item-extent/grid/SPS admission;
independently generated and decoded pixel fixtures for primary selection, alpha,
color/depth and transforms; and bounded runtime confinement on native/browser.
The synthetic metadata tests are not pixel-oracle or codec-conformance evidence.
