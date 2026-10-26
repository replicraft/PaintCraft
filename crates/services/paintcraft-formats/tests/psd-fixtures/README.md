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

## Internal fresh RGB8 writer qualification, 2026-10-26

`writer/` contains four tiny newly authored PSD/PSB copies and the immutable
source-Document vectors exported by `tests/psd_write.rs`. The layered scene uses
the independently generated ORA arithmetic fixture, with a Unicode/emoji name;
the second scene has alpha 0/1/127/128/254/255 and nonzero hidden RGB. No foreign
art or original Adobe project is included. All generated specimen pixels and
metadata expectations are covered by the repository license.

The selected `ag-psd` 0.3.0 writer receives a closed RGB8 flat Normal model, exact
raw typed layer channels, explicit no-background/no-trimming options and the
existing core compositor's merged view. The small resource adapter inserts only
ICC resource 1039, carrying the unchanged [shared sRGB profile](../../../../../assets/profiles/README.md),
after validating version/header, empty resource sections and complete outer
layer/composite framing. It does not serialize layer or image data. Ordinary
foreign preserving Save and every public PSD/PSB write route remain disabled.

Independent verification uses the already pinned psd-tools 1.26.0 and
Pillow 13.0.0 under Python 3.14.0. `writer/independent-writer-oracle.json` records
exact file/profile/raw-layer hashes and separate cached-preview/recomposed-layer
comparisons. Every raw layer pixel, alpha, Unicode name, signed offset, order,
visibility and opacity matches the source vectors exactly. ICC bytes match the
shared asset exactly. Both opaque layered composites match exactly.

The alpha-boundary files retain a deliberately measured preview limitation:
psd-tools recognizes/removes the conventional white matte in the selected
writer's merged cache. The decoded cached RGB differs by up to 60 at low alpha,
with exact alpha. Over either opaque black or white its displayed difference is
at most 1. Freshly recomposing the unchanged editable layers differs only in RGB
at alpha 0 (maximum 255); visible RGB, alpha and opaque-backdrop output are exact.
Those original results are retained, not normalized. Cache representation alone
is not classified as loss of exact editable layer data, and no claim of lossless
whole-file rendering or Photoshop compatibility is made. The Adobe specification
identifies merged transparency via a negative layer count; it does not itself
establish this writer's white-matte convention.

The initial API reports `psd-document-title-not-exported` for an actual native
Document title. It refuses unsupported bounds/names, empty stacks, duplicate or
missing roots, oversized aggregate storage and unknown loss consent before any
staged output. Logical declarations cover raw models/clones, selected PackBits
scratch, output capacity/copies and bounded metadata. They are not a global
allocator/RSS cap. Selected encoding checks cancellation before/after its bounded
call, not cooperatively inside that library; staged copy and ICC shifting check
bounded chunks. Logical library panics become codec errors; OS allocation aborts
are not caught. Callers still own source/owner guards and atomic publication of
the staged sink.

Reproduce owned output into an **existing empty** private directory, then run the
independent reader. The opt-in fixture refuses to overwrite existing files:

```sh
PAINTCRAFT_PSD_WRITER_OUTPUT=/private/tmp/your-empty-output \
    cargo test -p paintcraft-formats --test psd_write \
    export_owned_writer_oracles_only_when_explicitly_requested --offline --locked
/private/tmp/paintcraft-psd-oracle/bin/python verify_writer.py \
    /private/tmp/your-empty-output --record
```

`verify_writer.py writer` verifies checked-in originals without a PaintCraft
roundtrip or evidence-file rewrite. `--record` creates a report only when none
exists; it never overwrites prior evidence. Actual Photoshop open/edit/resave and additional foreign consumers
remain separate required gates; PDN/PAINT writers are also still pending. No old
stuck GIMP or Apple ImageIO qualification process was restarted for this work.

Primary contract: [Adobe Photoshop File Formats Specification, November 2019](https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/),
including PSD/PSB lengths, RGB8 channels, layer records, transparency and image
resource 1039. Independent API: [psd-tools](https://psd-tools.readthedocs.io/en/latest/).
