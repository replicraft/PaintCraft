# Still GIF logical-screen fixtures

These pixels and container inputs were authored for PaintCraft. No external
image is copied. `generate.c` uses independent GIFLIB 6.1.3 encoding; the
production reader uses the already-selected Rust `gif` 0.14.2 indexed decoder.
`giflib_oracle.c` independently decodes source descriptors, palettes, controls
and indices. Pillow 12.0.0 independently agrees on every actual in-frame RGBA
sample, including transparent palette RGB and the interlaced nine-row image.

The qualified `gif-still-logical-screen-v1` policy is explicit:

- Opaque uncovered screen pixels use the declared **global** background color,
  including when the frame has a different local palette.
- With a transparent index, uncovered screen pixels are clear `[0,0,0,0]` under
  the standalone transparent-image convention. Actual transparent pixels inside
  the frame retain their palette RGB with alpha zero.
- Disposal modes 0, 1, 2 and 3 do not erase the captured still. Disposal is an
  action after display; no animation/disposal timeline is imported.
- An opaque partial frame with no global color table still needs an explicit
  background choice and remains refused. No guessed local-palette/black fill.
- A second frame remains refused; this is not animation editing or a new
  first-frame animation conversion.

The background/color-index definitions and four interlace passes come from
[GIF89a](https://www.w3.org/Graphics/GIF/spec-gif89a.txt), sections 18, 20, 23 and
Appendix E. The expected logical-screen vectors are authored from those rules
and independently decoded source facts, not from PaintCraft output.

## Decoder disagreements are retained

`oracle.json` stores the complete Pillow and Apple ImageIO RGBA outputs as well
as the explicit policy vector. They are not interchangeable oracles for the
background policy. In these files, Pillow initializes opaque padding from the
active palette's first entry, ignoring a nonzero declared global background;
ImageIO initializes it to opaque black. The previous `image` 0.25.10 path used
transparent-black padding. With transparency, Pillow and ImageIO have alpha-zero
padding but differ in its hidden RGB. These differences are recorded rather
than changing expected pixels to match a decoder's default canvas convention.
ImageIO's premultiplied output is not a raw transparent-RGB preservation oracle.
Its capture used the included `imageio_oracle.c` without an OS window.

## Reproduce the source evidence

Use installed GIFLIB 6.1.3 and a Python environment containing Pillow 12.0.0.
Keep generated utilities in ignored validation storage, not a new Cargo target:

```sh
mkdir -p docs/validation/gif-still
clang -Wall -Wextra -Werror -I/opt/homebrew/opt/giflib/include \
  crates/services/paintcraft-formats/tests/gif-still-fixtures/generate.c \
  /opt/homebrew/opt/giflib/lib/libgif.a -o docs/validation/gif-still/generate
clang -Wall -Wextra -Werror -I/opt/homebrew/opt/giflib/include \
  crates/services/paintcraft-formats/tests/gif-still-fixtures/giflib_oracle.c \
  /opt/homebrew/opt/giflib/lib/libgif.a -o docs/validation/gif-still/giflib-oracle
python crates/services/paintcraft-formats/tests/gif-still-fixtures/verify.py \
  --giflib-decoder docs/validation/gif-still/giflib-oracle
```

The C encoder regenerates the ten `offset-*.gif` files. On another platform,
substitute the installed GIFLIB include/library paths. Reverification does not
require ImageIO or any PaintCraft runtime.

Two deliberately curated streams have independently stored GIFLIB facts:

- `invalid-palette-index.gif` starts from `offset-background2.gif`, changes the
  background index to zero, declares two global entries, and removes entries
  2 and 3. Its compressed LZW bytes are unchanged and actually decode indices
  2 and 3 beyond the now two-entry palette. Geometry, transparent-index metadata
  and logical background remain valid. Production rejects the actual samples;
  it does not invent transparent black for an invalid index.
- `literal-three.gif` is a hand-built full-screen 3×1 stream. The two LZW bytes
  `44 50` encode clear, 0, 1, 0, end, verified as `[0,1,0]` by GIFLIB. Tests
  remove its end code or change its declared width to expose absent EOI,
  undershort and extra output. Filling the declared samples is not sufficient:
  production drains the decoder with strict EOI validation before returning.

The bounded reader retains a 1 GiB encoded-input ceiling, the caller's pixel
allowance capped by the shared 268,435,456-pixel policy, and the existing 1 MiB
extension-metadata limit. Output/index buffers reserve fallibly. GIF's own
16-bit dimensions constrain each logical side to 65,535. Those are logical
admission controls, not a hard aggregate RSS/scratch-memory or interruptible
cancellation guarantee. Out-of-screen rectangles are rejected rather than
silently clipped; exact boundary placement is accepted.

These are finite backend/profile checks. Actual Windows Paint compatibility,
all browsers/devices and broader GIF semantics remain separate qualification.
