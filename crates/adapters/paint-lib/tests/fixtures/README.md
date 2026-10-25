# Observed PAINT sample and independent pixel oracle

`mspaintlib-sample.paint` is an unchanged copy of
[`samples/sample.paint`](https://github.com/setiri/mspaintlib/blob/e35312bec8e8a09c6a209d2dc0110b885e48c945/samples/sample.paint)
from Setiri LLC's MIT-licensed `mspaintlib`, pinned at commit
`e35312bec8e8a09c6a209d2dc0110b885e48c945`. The accompanying license is
`LICENSE-mspaintlib.txt`. SHA-256:
`a3e947fee374cca80e0e648152667109ac61b57808c9673eb806f52865f141f6`.

This is one independent upstream sample, not a PaintCraft-written round trip.
Its originating Windows/Paint build and UI-layer metadata are not recorded here.
It therefore qualifies the observed pixel/item subset, not general Paint fidelity
or a preserving writer.

`sample-oracle.json` records a separately executed Python `struct`/`zlib` decode:
read the five `iloc` entries directly at byte 226; each entry is `>HHHHII` in
this sample. Inflate the four file-relative method-0 extents with
`zlib.decompressobj(-15)`, requiring EOF and no unused/unconsumed bytes. Each
produces exactly `333 * 271 * 4` bytes. The declared `cmpd`/`uncC` property order
is BGRA, so swap byte positions 0 and 2 per pixel, then SHA-256 the entire output.
All four complete RGBA hashes are asserted by the Rust test. The 16-bit overlay
fill is `[0x6b00, 0x6b00, 0x6b00, 0xffff]` and is retained without narrowing.

The decoder is an independent Rust implementation. The pinned upstream
[`PAINT-FORMAT-SPEC.md`](https://github.com/setiri/mspaintlib/blob/e35312bec8e8a09c6a209d2dc0110b885e48c945/PAINT-FORMAT-SPEC.md)
and MIT source under `src/MsPaintFile` supplied reviewed format observations,
including the corrected component-index swizzle. No .NET/CLR code is executed.
