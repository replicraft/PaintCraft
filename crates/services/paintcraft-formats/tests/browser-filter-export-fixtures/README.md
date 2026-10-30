# Independent browser filter-copy inputs

`python3 generate.py` uses only Python standard-library byte packing, UUID and
SHA-256. It writes four canonical PCND3 files without using a PaintCraft reader,
serializer, renderer or filter evaluator. The source pixels are opaque black then
opaque white. Enabled Invert produces white then black; disabled Invert preserves
the source pixels but still carries editable intentions that a foreign copy loses.

`oracle.json` binds every complete input, literal expected RGBA, and the published
canonical filter inventory digest. The digest writer independently packs document,
state, root order, filter-instance identity/enable/descriptor, placement, opacity,
visibility, finite layer dimensions and implicit default. It is not bake authority.

The optional attachment is copied exactly from the separately authored
`pcnd-metadata-fixtures/rgb-profile.pcnd`; its parent SHA is pinned in the oracle.
It is original-source provenance, unrelated to the current generated canvas, and
never activates a color transform. Keeping these cases monochrome permits the
same exact source/expected-pixel oracle for both DDS RGBA8 and strict MSP2 export.
