# ICO/CUR selected-entry evidence

`verify.py` constructs these fixtures with Python's standard-library byte packing,
CRC and zlib routines. It does not call PaintCraft, `ico`, or Pillow to generate
the container or pixels. There is no third-party artwork. `manifest.json` records
source-file and expected RGBA SHA-256 values.

The mixed directory contains 17, 19, 21, 23 and 25 pixel bitmap entries at depths
1, 4, 8, 24 and 32, plus a 256 pixel RGBA PNG. Nonaligned widths exercise row
padding and AND masks. The 32-bit entries include partial and zero alpha.
The CUR directory uses the same image payloads with distinct explicit hotspots.

Run the independent check with Python containing Pillow:

```sh
python crates/services/paintcraft-formats/tests/icon-fixtures/verify.py
```

The 2026-10-25 run with Pillow 13.0.0 decoded all six ICO entries to exact expected
RGBA hashes. CUR payload identity and hotspot fields were checked independently
as bytes. Pillow's CUR reader cannot decode PNG cursor entries, so this is not a
Windows cursor-consumer test. Default Rust tests decode every ICO and CUR entry
and compare to the same independently generated hashes. `--generate` explicitly
regenerates the fixture files, then runs the independent checks.

This qualifies the declared selected-entry import subset, not arbitrary icon
variants, fresh export, original Windows behavior, or preserving foreign saves.
