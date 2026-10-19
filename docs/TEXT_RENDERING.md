# Document text rendering

Desktop text now uses Cosmic Text 0.19.0 for shaping, fallback and layout, with
Swash 0.2.10 for hinted grayscale glyph coverage. Selawik Regular and Bold are
bundled under OFL 1.1; native installed-font loading happens in the background.
The searchable family picker and text renderer share one fontdb 0.23 database.
Headless GUI tests use only bundled fonts, keeping their output independent of
the machine's installed collection.

Font size is in points at 96 document pixels per inch (11 pt is 14⅔ pixels),
independent of monitor scaling and viewport zoom. Bold selects a matching weight;
italic uses a face or Cosmic's synthetic italic. Underline, strikeout, alignment
and background fill affect the staged document. Text color and background follow
the active colors while editing. The text tool accepts Unicode; available glyphs
and fallback depend on the loaded font collection.

## Atomic publication and compatibility

Fonts are resolved before a transaction. The additive v2 `raster.mask_spans`
operation stores sorted, disjoint `[x0, y, x1, coverage]` rows with inclusive x1
and coverage 1–255. It has no font-library dependency. The engine multiplies
coverage by paint alpha with nearest rounding and applies the ordinary source-over
policy, layer origin, canvas/selection clipping and staged-memory admission.
Background fill precedes the glyph mask in the same atomic batch.

Preview executes those same operations on an unpublished snapshot. Placing text
creates one undo step. PCND saves the resulting pixels, so reopening does not
need the selected font installed and does not rerasterize differently.
The legacy v2 `raster.text` operation keeps its exact font8x8 semantics for existing
CLI scripts and fixtures. The new mask operation is rejected in v1 batches.

The parser and engine reject unsorted/overlapping spans, zero coverage and invalid
coordinates. Limits are 262,144 spans and 16,777,216 covered pixels; the desktop
rasterizer additionally caps its temporary coverage map at 4,194,304 pixels and
clears its Swash cache beyond 2048 glyph entries. The tool's existing 1024-character
limit remains. Work/admission failures leave the document unchanged.

Keeping placed text editable until save is still an explicitly labelled Settings
prototype. Persistent text objects, selection/caret editing, font-resource capture,
IME qualification and atomic save-time rasterization remain separate work.

## Library comparison and decision

| Choice | Role | Assessment for PaintCraft |
|---|---|---|
| [Parley](https://github.com/linebender/parley) | Rich text layout and shaping integration | Useful for paragraph/rich-text work, but still needs a rendering backend and editor integration; adding a second layout stack is unnecessary now |
| [Swash](https://github.com/dfrg/swash) | Glyph scaling, hinting and rasterization | Chosen through Cosmic Text; its alpha format supplies grayscale coverage rather than RGB display-subpixel fringes |
| [Fontdue](https://github.com/mooman219/fontdue) | Font parsing, glyph rasterization and basic layout | A viable simple rasterizer; our shaping, fallback and editing integration would still need additional components |
| [Cosmic Text](https://github.com/pop-os/cosmic-text) | Shaping, fallback, layout and editing abstractions, with Swash rendering | Chosen: fits the immediate document-text workflow and includes HarfRust shaping plus the existing fontdb direction |

The previous document text path used font8x8, not ab-glyph. The UI toolkit has its
own text renderer; this change concerns pixels written into documents.

## Microsoft Paint antialiasing evidence

The supplied photo establishes the text toolbar, not the exact Windows Paint
rendering mode. No primary source examined establishes whether that particular
Paint build uses grayscale, ClearType, GDI or a particular DirectWrite setup.
Microsoft documents [grayscale and ClearType coverage](https://learn.microsoft.com/en-us/windows/win32/api/dwrite_1/ne-dwrite_1-dwrite_text_antialias_mode)
and [Direct2D/DirectWrite alpha behavior](https://learn.microsoft.com/en-us/windows/win32/direct2d/direct2d-and-directwrite).
Those capabilities alone do not establish Paint's choice. PaintCraft deliberately
uses hinted grayscale coverage for portable RGBA images. Exact Paint parity
requires pixel captures from a pinned Windows build/font/DPI configuration.
