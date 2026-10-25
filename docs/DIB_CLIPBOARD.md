# Packed Windows DIB clipboard images

Windows clipboard reads now prefer registered PNG, then `image/png`, DIBV5, DIB
and TIFF. The existing bounded snapshot transport freezes advertised bytes before
any PaintCraft image decoder runs. A malformed preferred representation fails
rather than silently trying another format. This extends the implemented Windows
reader; native Windows clipboard execution and cross-application/reference fidelity
remain unqualified.

## Declared decoder subset

The `paintcraft-formats` adapter accepts packed 40-byte BITMAPINFOHEADER, 108-byte
BITMAPV4HEADER and 124-byte BITMAPV5HEADER data. A DIBV5 transport must have the
124-byte header. It supports top-down and bottom-up rows, DWORD padding, and:

| Input | Declared policy |
| --- | --- |
| BI_RGB 1/4/8-bit | Expand the declared RGB palette; reject out-of-range indices |
| BI_RGB 16-bit | RGB555, without alpha |
| BI_RGB 24-bit | BGR samples with padded rows |
| BI_RGB 32-bit | Ignore reserved high bytes, import opaque, report alpha limitation |
| BI_BITFIELDS 16-bit | RGB555 or RGB565, without alpha |
| BI_BITFIELDS 32-bit | Standard RGB888 masks; optional full high-byte alpha mask |

Other header versions, RLE, embedded PNG/JPEG, arbitrary/overlapping masks and
16-bit alpha masks are rejected. Indexed palettes cannot exceed their bit domain;
true-color optimization palettes are limited to 256 entries. Header, palette,
stride, sample range and nonzero declared image size must agree. Packed V5 profile
ranges must lie after the complete bitmap bits and within the frozen input.

Dimensions and the caller's decoded-pixel limit are checked before constructing
the `image` BMP decoder. Palette references are validated with cancellation checks
per row and every 4096 samples. The native caller retains its 16,777,216-pixel
limit and pre-acquired output grant; encoded native input remains capped at 64 MiB.
The shared formats API also retains the existing generic decode ceiling.

## Decode ownership and offsets

Pixel decoding uses the selected `image` crate, not a second bitmap codec. A small
`BufRead + Seek` adapter provides a 14-byte BMP file header on the stack over the
borrowed DIB body. Its explicit pixel offset avoids relying on inferred V4/V5 mask
layout. The first pixel, row padding, alpha and hidden RGB have independent byte
fixtures. The reader can seek across the header boundary and safely reject
negative/overflowing seeks; beyond-end reads return EOF.

No second complete encoded image vector is constructed. The formats decoder never
resolves a linked color-profile path. The Windows data provider/OS may perform
opaque work during acquisition; that work is outside this decoder and the Rust
payload lease. Decoded scratch, allocator overhead and aggregate physical-memory
and wall-clock cancellation tails remain outside this qualification.

The transport recognizes DIB header signatures and retains its length/sequence
checks. The application separately gates the BMP backend for the captured frontend,
then invokes the strict DIB adapter. This does not enable `.dib` file opening or
browser DIB clipboard flavors; those routes have separate admission contracts.

## Explicit import limitations

The ordinary/new-layer Paste dialog requires consent for any reported limitation.
The descriptions are available in all seven UI languages.

- `dib-alpha-unspecified`: a 32-bit image without a declared alpha mask is imported
  opaque. Reserved bytes are not guessed to be transparency.
- `dib-alpha-mode-unspecified`: explicit coverage masks retain their samples,
  interpreted as straight alpha. The header does not establish whether its
  producer premultiplied RGB; no magnitude heuristic or implicit unpremultiply is
  applied. Native source-specific alpha behavior still needs reference capture.
- `dib-metadata-ignored`: color-space, profile, resolution or other declared bitmap
  metadata is not retained in the pasted raster.
- `palette-indices-expanded`: indexed inputs become RGBA pixels.

A failed/cancelled decode or unaccepted limitation never starts placement or
publishes pixels. Accepted pixels use the existing frozen-source positioning and
one-entry history path. Save/reopen and Undo tests cover the native handoff;
clipboard bytes and profile paths are not stored in the document by this adapter.

## Evidence and remaining scope

Eight formats tests use independently constructed DIB bytes for row direction,
padding, palette/16-bit conversion, V4/V5 mask offsets, alpha and hidden RGB,
invalid headers/dimensions/compressions/masks, every truncation of a valid V5
sample, pixel denial, profile overlap and observed cancellation points. Reader
checks also prove that body buffers remain borrowed. Platform tests cover exact
DIB/DIBV5 transport preference and refusal to downgrade malformed DIBV5.

Native handoff tests cover every Classic skin and Studio, synchronous and worker
publication, opaque-alpha consent, exact pixels, PCND reopening and Undo. Actual
English/Japanese dialog checks verify translated limitations, disabled Paste
before consent and cancellation after private placement. All-locale catalog and
font-glyph checks cover the new messages.

Strict platform checks compile the Windows reader/tests for x86-64 and ARM64;
Wasm Clippy checks the portable decoder. Release jobs are configured to execute
DIB decoder tests on both Windows architectures, but those jobs have not run from
this checkout. Native clipboard providers, premultiplication/ICC/DPI behavior,
physical device/input latency and pinned Paint/Paint.NET fidelity remain
unqualified. Linux/FreeBSD readers and additional DIB variants remain work.
No measured speedup is claimed.

The layout references are Microsoft's [BITMAPINFOHEADER](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-bitmapinfoheader)
and [BITMAPV5HEADER](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-bitmapv5header).
Code and synthetic fixtures are PaintCraft implementations; no upstream codec
implementation was copied. The workspace unsafe-code prohibition is unchanged.

The follow-up passes 379 desktop library tests (nine explicit captures/checks
ignored), 105 formats tests, 75 platform tests (two native checks ignored) and
49 shared UI tests (one capture ignored). Strict host all-target, Windows
x86-64/ARM64 platform and Wasm Clippy checks pass. Rust formatting, the 800-line
source limit, all 988 messages in seven locales and the release test YAML pass.
Logs and source hashes are local in ignored `docs/validation/dib-clipboard-2026-10-25/`.
