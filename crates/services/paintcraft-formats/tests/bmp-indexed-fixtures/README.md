# Uncompressed indexed BMP fixtures

These original fixtures are generated from the explicit palette and index rows
in `generate.py`; they contain no third-party artwork. Run the script with
Python3 and FFmpeg available. `oracle.json` records source hashes, the FFmpeg
version and executable hash. The six valid fixtures' committed `.rgba` files
come from independent FFmpeg decoding and match the explicitly authored rows.

Each image is13×3 pixels. BI_RGB depths1,4,8 each have bottom-up and top-down
variants. Nonzero unused packed bits and row padding must be ignored. RGBQUAD
reserved values137–140 must remain nonalpha; every output sample is opaque.
The three malformed variants declare fewer palette entries while still using
the missing index. The pinned image backend fills those entries with black;
PaintCraft's existing admission already refuses these sources before pixel
expansion; the integration tests qualify that existing guard.

The six `.ico` files reuse these independently authored bitmap rows in an
explicit single-entry ICO directory. Three are valid, with two AND mask bits
set; three retain undeclared indices. The valid `.ico.rgba` expectations retain
RGB under transparency and apply the two authored mask bits. FFmpeg's ICO path
ignores these AND masks: its different `.ico.ffmpeg.rgba` outputs are retained
as evidence, and independently confirm RGB. No independent-tool alpha agreement
is claimed. The selected ico0.6.0 decoder indexes its declared-count table
unchecked, so preflight must reject malformed indices even under an AND mask.

This corpus qualifies this bounded indexed admission slice. It does not
qualify BMP16, additional DIB headers/compressions, color management, original
Windows applications, or broader ICO profiles. No FFmpeg dependency ships with
the application.
