# Independent WebP fixtures

These synthetic inputs were generated for PaintCraft on 2026-10-24 using reference C libwebp `cwebp 1.6.0` (libsharpyuv 0.4.2), independently of the Rust `image` / `image-webp` production path. No third-party photographs or assets were used.

- `alpha.webp`: 16x16 RGBA8. Pixel index `a=0..255` has RGBA `[a % 255, 80, 170, a]`. Generated from an RGBA PNG using `cwebp -quiet -lossless -exact alpha.png -o alpha.webp`. SHA256 `232a08cd72befb386314ff4892107b44798b23276b79b3813495c206b8126a09`.
- `opaque.webp`: 3x2 RGBA8, every pixel `[80, 90, 100, 255]`. Generated using `cwebp -quiet -q 100 opaque.png -o opaque.webp`. SHA256 `e858b7a3e9c37ea9db9854f2f95dfc7f3b2af98ab021278d685d77d1692cd2f8`.

The fixture test checks lossless RGBA values exactly, including all alpha values and RGB under alpha zero. The lossy fixture permits an absolute error of 3 per channel. The explicitly ignored external oracle test can be invoked on machines with `dwebp` installed to check PaintCraft lossless export against the reference C decoder, including every raw alpha sample.

```sh
CARGO_INCREMENTAL=0 cargo test --offline -p paintcraft-formats --test raster_tga_webp external_libwebp -- --ignored
```

TGA fixtures are assembled directly in the integration test from their declared headers, palette entries, packet layouts and expected pixel coordinates.
