# OpenRaster reader fixtures

`generate.py` creates both archives independently with Python's standard-library
ZIP writer, a minimal PNG writer and arithmetic RGBA samples. No PaintCraft
serialization code or third-party artwork is used. `oracle.json` records layer,
merged-pixel and complete archive SHA-256 values. Regenerate with `python3 generate.py`.

The fixtures exercise Normal layers, negative offsets, visibility, Unicode XML
names/entities, hidden RGB, stored and raw-DEFLATE members, merged preview and
thumbnail. They do not establish original-application compatibility or writer
qualification. GIMP/Krita reopen and edit evidence remains required.

References: [file layout](https://www.openraster.org/baseline/file-layout-spec.html)
and [layer stack](https://www.openraster.org/baseline/layer-stack-spec.html), version
0.0.6. Generated data and generator are covered by the repository license.
