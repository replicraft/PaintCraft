# Selected JPEG XL decoder source

This is the crates.io `jxl` 0.8.0 archive with a metadata exposure patch. Decoder arithmetic, color transforms, parsing, dependencies and feature defaults are unchanged.

- Archive: `https://static.crates.io/crates/jxl/jxl-0.8.0.crate`
- SHA-256: `b8379fc83d79477900c9aed78cb8370a556d834081bebcb771c6010381c4e230`
- Upstream VCS declaration: `9a08ca643fd2d01f803dd0be6a5bebc9cc11e597`, with no dirty flag.
- License: original BSD-3-Clause `LICENSE` retained verbatim.

All 179 archive files are retained. Only three metadata files differ: `src/api/data_types.rs`, `src/api/codestream_parser/image_info.rs` and `src/headers/extra_channels.rs`. The public extra-channel descriptor now reports its actual source integer/float bit depth, dimension shift and whether it has a nonempty name. A private getter exposes only name presence; the patch does not copy name strings or allocate a new name buffer. Public metadata is populated directly from the decoder's parsed source header. Missing information is never inferred from output pixels or main-channel precision.

PaintCraft's isolated worker profile uses these facts to refuse mixed color/alpha precision, floating alpha, downsampled alpha and named extra channels before allocating its output planes. Its general adapter retains its existing conversion behavior. This change alone does not enable 16-bit editing, new frontend imports, HDR, animation or preserving Save. Upstream may allocate header/ICC scratch before the public header event; hostile input still requires the bounded, killable helper.

The adjacent patch and provenance inventory bind the complete archive-to-vendor delta, all original licenses and every file hash. Eighteen unchanged inherited source files exceed 800 lines and receive only exact hash-verified exceptions. The three modified source files are below 800 lines; authored files do not inherit an exception. A package-local `rustfmt.toml` contains only `disable_all_formatting = true` to preserve inherited upstream bytes. Source attributes preserve exact checkout bytes across platforms.

Run `python3 scripts/verify_jxl_decoder_vendor.py` to verify this snapshot. Add `--archive <jxl-0.8.0.crate>` to verify the original archive and complete delta. Runtime and interoperability evidence belongs to the corresponding adapter/worker tests, rather than this source inventory.
