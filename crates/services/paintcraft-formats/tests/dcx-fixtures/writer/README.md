# Fresh DCX writer qualification

These three owned files each contain three independent flat PCX pages in directory
order: RGB24, indexed8, and mixed indexed8/RGB24/indexed8. They are not document
layers, animation frames, or an inferred document-page model. Pixel dimensions are
3×2, 65×2, 1×5; DPI is 72/144, 73/145, 74/146. The indexed pages deliberately use
separate 256-entry palettes with duplicate colors at indices 3 and 17. Odd widths,
RLE-marker-valued literals and long same-channel runs exercise the selected writer.
Normal Rust tests regenerate and compare every byte against these independently
verified specimens; optional output uses create_new and never overwrites fixtures.

## Independent whole-container consumer

`../verify_writer.mjs` uses untouched `@imagemagick/magick-wasm` 0.0.44, containing
ImageMagick 7.1.2-32 Q8 wasm32 ad98b244c:20260927. Node was v24.10.0. It asserts
all nine pages' exact RGB samples, dimensions, order and DPI, all 256 palette entries
and original raw indices (including distinct 3/17), source immutability, and refusal
of an out-of-file first-page offset. `imagemagick.json` records the results.

Qualification-only npm archive:
`https://registry.npmjs.org/@imagemagick/magick-wasm/-/magick-wasm-0.0.44.tgz`

- SHA256: `808e876ed849de942b47dec268d4a8059e68fc2470aa895266bb36754eb968a7`
- SHA512 base64: `J6oieSjlJzamANwtrNAlO+DKnRm49ILtSIWDhtkGn35f/5zS99AXta2gPsZuG9m2JvQbsu3h3CHe14ViXrFD/Q==`
- Wrapper SHA256: `cf1db24f7e63f560118ef434fd581dc9ef66ad099dc417184765cd6076fb4a1b`
- Wasm SHA256: `b7d6710436b150dc5bd1ca04d80b6b7614c54854d7470f888f7e69a2a41b00e4`

The verifier checks wrapper/Wasm hashes before loading. The package and Wasm are
private qualification tooling, not repository assets or product dependencies.
With the verified archive extracted into a private `oracle/package` directory:

```sh
node ../verify_writer.mjs . oracle/package
```

## Independent directory and PCX consumer, retained negative evidence

Python 3.14.0 and Pillow 13.0.0 independently parse every bounded page and prove
exact RGB, palette order, index bytes and DPI. Whole-file Pillow DCX decodes all RGB
pages exactly. Earlier indexed pages do not decode correctly through its DCX
plugin: its inherited indexed PCX reader seeks the palette relative to whole-file
EOF, thus using the final page's palette. `pillow.json` retains the measured wrong
RGB channel counts. No fixture palette was changed to hide this consumer limitation.
The independent ImageMagick whole-container checks above qualify distinct palettes.

```sh
python ../verify_writer.py .
```

## Supported production primitive and limits

The typed backend delegates each page to selected `pcx` 0.2.5 encoders. It emits
magic 987654321, 1024 little-endian directory entries, 1..1023 strictly increasing
page offsets, and a zero unused suffix. Every page is validated before allocation:
RGB24 or indexed8, positive DPI, width 1..65534, height 1..65535, aggregate 4,194,304
pixels, at most 64MiB admitted output, and default 128MiB logical transient budget.
Supplied palette entries/indices are retained; absent palette suffix entries are
black. There is no alpha conversion or implicit quantization in this typed API.

The checked output bound is directory bytes plus each PCX header128, twice the
padded plane-row bytes×height×plane count, and indexed palette trailer769. Audited
selected `pcx` 0.2.5 `writer.rs` and `low_level/rle.rs` use scalar streaming RLE
state, with no heap row/pixel cache. Logical transient pricing includes borrowed
source arrays/palettes, reserved outer output, largest encoded page, directory and
page descriptors, plus 4096 bytes stack headroom. This is explicit logical pricing,
not a global allocator/RSS ceiling. Pages are encoded sequentially; cancellation is
checked during admission, selected encoding and bounded output chunks. Private
sink failures do not authorize publication; callers retain atomic-output ownership.

This packet qualifies the typed backend. It does not enable public export, invent a
multi-page document model, or claim Classic stock compatibility. Existing selected
page import behavior is unchanged.
