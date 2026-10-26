# Selected JPEG XL encoder source

This is the crates.io `zune-jpegxl` 0.5.2 archive, with one reviewed encoder patch:

- Archive: `https://static.crates.io/crates/zune-jpegxl/zune-jpegxl-0.5.2.crate`
- SHA-256: `cba11ecd07cc23351500e840ae03841b7e4997923ec8e4832ee3ae199ea0b6b4`
- Upstream VCS declaration: `a019244c4a8a4eb713e85fa77f9e18f841d6271b`, **dirty = true**.

The archive is the source authority. Its VCS declaration does not identify a clean commit checkout. The fourteen original archive files, original normalized and source manifests, and all three original licenses (`LICENSE-APACHE`, `LICENSE-MIT`, `LICENSE-ZLIB`) are retained. Only `src/encoder.rs` differs from its original bytes. The adjacent patch and provenance inventory record the complete delta.

The patch allows one-pixel dimensions in the width guard, height guard and size-header assertion, while retaining zero-dimension refusal. It also writes explicit straight 16-bit alpha extra-channel metadata when encoding 16-bit samples; upstream otherwise declares that channel as 8-bit. It does not pad or crop the image, discard alpha, change pixel values or replace the compression engine.

A separately inventoried, authored `rustfmt.toml` contains only `disable_all_formatting = true`. This stable, package-local setting preserves inherited upstream bytes during normal workspace formatting. The authored alpha block is independently checked with preservation disabled. Exact-byte Git attributes and verification protect the original files, the patch and the local configuration. Only the exact hash-verified inherited `src/encoder.rs` receives a source-line-limit exception; other files and added source cannot inherit that exception.

Twenty-one vendor controls passed, including mutation refusals, both vendors remaining required by the generic line checker, and exact Git index/checkout bytes with `core.autocrlf` set to `true`, `input` and `false`. Ten fresh vectors (1x1, 1x6, 6x1, 3x2 and 255x257, each RGBA8 and RGBA16) matched every sample through independent djxl 0.12.0 and the selected jxl 0.8 decoder. The 16-bit vectors include alpha 0, 1, 256, 32768, 65534 and 65535 with nonzero hidden RGB. Historical baseline failures remain recorded separately.

At this dependency-only stage, PaintCraft's first-party single-axis and nonopaque RGBA16 writer refusals remain in place. Their removal requires a separate adapter change and qualification. These tests do not establish a 16-bit editor, lossy/HDR or metadata-preserving export, animation, broad JPEG XL compatibility, or execution on every release platform.

Run `python3 scripts/verify_jpegxl_vendor.py` to verify this snapshot. Supplying `--archive <original .crate>` additionally verifies every original file and the complete archive-to-vendor delta. Detailed source/artifact receipts are under `docs/validation/jxl-writer-2026-10-26/` in the qualification workspace.
