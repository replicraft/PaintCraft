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

## Fresh writer checks

`icon_write.rs` prepares explicit PNG or 32-bit bitmap entries and checks exact
roundtrips, ordering, duplicate/rectangular sizes, hotspots, resource admission,
and cancellation at every exposed checkpoint. The writer uses `ico` for
directory assembly, reading and writing. Explicit payload adapters keep opaque
PNG entries RGBA and bitmap entries at 32 bits instead of the library's automatic
RGB PNG / 24-bit bitmap choices. Bitmap AND bits are set exactly where alpha is
zero. Fully transparent bitmap entries remain refused because the all-zero alpha
channel is also a legacy 0RGB signal; PNG entries support complete transparency.

```sh
PAINTCRAFT_PILLOW_PYTHON=/path/to/python cargo test -p paintcraft-formats \
    --test icon_write independent_pillow -- --ignored --nocapture
```

The 2026-10-25 run with Pillow 13.0.0 matched all 24 exported entry pixel buffers,
including opaque and fully transparent PNGs. `verify_writer.py` also checks
container coverage, depths, cursor hotspots and every meaningful AND-mask bit.
CUR pixels are checked through equivalent ICO envelopes because Pillow's CUR
reader cannot handle PNG entries. No actual Windows application was run.

The payload policy follows Microsoft's description of
[alpha-blended icons](https://devblogs.microsoft.com/oldnewthing/20101021-00/?p=12483)
and [PNG icon entries](https://devblogs.microsoft.com/oldnewthing/20101022-00/?p=12473).
These sources establish the selected byte profile, not a completed Windows
application qualification. No performance improvement or hard codec scratch/RSS
or cancellation-tail bound is claimed.
