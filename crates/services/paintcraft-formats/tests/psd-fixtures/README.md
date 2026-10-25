# Independent PSD/PSB RGB8 fixtures

`generate.py` uses Python's standard library (`struct`, `zlib`) and arithmetic
samples. No PaintCraft or ag-psd writer generates these files. No third-party
artwork is included.

The sixteen fixtures cover PSD and PSB, each with raw, PackBits RLE, ZIP and
ZIP prediction compression, both with raster layers and with only a composite.
The layer records use background-to-foreground order. Duplicate Unicode names,
signed offsets, hidden state, partial opacity, alpha samples, persistent source
IDs, default blending ranges and an XMP resource exercise distinct properties.
The composite is deliberately a separate arithmetic image so a decoder cannot
pass the layer checks by substituting the merged preview.

Regenerate and independently verify:

```sh
python3 generate.py
python3 -m venv /tmp/paintcraft-psd-oracle
/tmp/paintcraft-psd-oracle/bin/pip install psd-tools==1.26.0 Pillow==13.0.0
/tmp/paintcraft-psd-oracle/bin/python verify.py
```

`oracle.json` records file and RGBA hashes plus expected layer properties.
Verification passed with psd-tools 1.26.0 and Pillow 13.0.0 on 2026-10-25.
This checks independent parsing and exact pixels for this declared subset.
It does not establish Photoshop reopen compatibility, writing, advanced-layer
support, color-managed equivalence, or a hard process memory/time limit.
