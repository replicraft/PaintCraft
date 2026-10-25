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

## Writer qualification

`ora_write.rs` tests source immutability, deterministic output, exact layer samples,
all 256 opacity values, XML whitespace/Unicode, private cancellation, sink failures
and resource refusal. Document-title omission requires the exact
`ora-document-title-not-exported` acknowledgement before any output is written.
The writer emits fresh copies; foreign preserving Save remains unavailable.

Run the independent standard-library ZIP/XML/PNG reader explicitly:

```sh
cargo test -p paintcraft-formats --test ora_write independent_python -- --ignored
```

The opt-in `gimp_reopens_edits_and_saves_exported_layers` test uses
`PAINTCRAFT_GIMP` to select a GIMP 3 console executable with Python/PNG/OpenRaster
plugins. It uses an isolated profile, no display, and a 120-second timeout. On Unix,
`PAINTCRAFT_GIMP_PLUGINS` optionally selects the installed plugin directory, so
only the three required plugin folders are queried. A successful oracle must check
actual layer hashes and properties, edit layers, save to XCF and reopen those edits.
A timeout or missing JSON report is a failure, never compatibility evidence.

Fresh ORA writer frontend admission remains disabled pending this external gate.
