# PCX writer evidence

`pcx_write.rs` generates arithmetic RGB and indexed samples at widths 1, 7, 63,
64, 65 and 257. Palette entries 3 and 17 deliberately contain the same color so
an RGBA-only comparison cannot mistakenly stand in for index preservation.
There is no third-party artwork and no PaintCraft-generated decoder oracle.

The selected `pcx` 0.2.5 encoder handles RGB24 and indexed-8 scanlines. The adapter
checks logical dimensions before calling it, including the padded u16 row-width
overflow at 65,535. It reports row/output cancellation without reclassification.
Flat RGBA export uses a declared white matte, with alpha loss checked by the
shared document-export preflight before bytes can be published.

Run the independent comparison explicitly with Python containing Pillow and
`ffmpeg` on PATH:

```sh
PAINTCRAFT_PILLOW_PYTHON=/path/to/python cargo test -p paintcraft-formats \
    --test pcx_write independent_pillow -- --ignored --nocapture
```

The 2026-10-25 run passed with Pillow 13.0.0 and FFmpeg 8.0.1: twelve exports had
exact RGBA; all six indexed exports also had exact palette order and index bytes.
The script prints tool versions and case outcomes. Default tests additionally
verify RLE/padding, exact white matte, loss consent, invalid inputs and cancellation.

The public indexed writer takes explicit indices and palette entries. Studio's
export UI offers RGB24 and explicit indexed palette policies with a preview.
Original historical-application reopen tests remain separate requirements.

`pcx_palette.rs` also tests explicit exact/quantized palette preparation, matching
preview/encoder ownership, precise loss codes and cancellation. The optional
`independent_decoders` test invokes `verify_palette.py` through the same Pillow
Python variable; both Pillow and FFmpeg must decode exactly the prepared preview.
It does not claim a lossless reduction of an image with more than 256 colors.
CLI/MCP indexed options and desktop/browser palette controls are connected.
