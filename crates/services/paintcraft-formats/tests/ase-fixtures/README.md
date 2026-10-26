# ASE palette fixtures

The active F14 asset slice uses `ag-psd` 0.3.0 after bounded structural preflight.
It retains palette resources, not document images. Groups, order, duplicate names,
Unicode, RGB/CMYK/Gray/LAB model tags, Global/Spot/Normal types and finite f32
channel bits survive without color conversion. Names require strict UTF-16 and
null termination; nested groups and unknown blocks are refused.

`generate.py` uses only Python standard-library big-endian struct packing for
`mixed.ase` and `empty.ase`. The mixed specimen contains all four models, all
three types, grouped/root colors, an empty group, duplicate names, Japanese and
non-BMP text, and negative zero. Fixtures are repository-authored under MIT.
`swatch-generated.ase` is independently written by MIT-licensed
[swatch 0.4.0](https://github.com/nsfmc/swatch), using the owned values in
`oracle.py`. No upstream fixture or implementation code is copied.
`sources.json` records byte counts, hashes and generation routes.

Regenerate and independently check:

```sh
python3 -m venv /tmp/paintcraft-ase-oracle
/tmp/paintcraft-ase-oracle/bin/pip install swatch==0.4.0
python3 crates/services/paintcraft-formats/tests/ase-fixtures/generate.py
/tmp/paintcraft-ase-oracle/bin/python crates/services/paintcraft-formats/tests/ase-fixtures/oracle.py \
    --write-fixture crates/services/paintcraft-formats/tests/ase-fixtures/swatch-generated.ase
python3 crates/services/paintcraft-formats/tests/ase-fixtures/generate.py
cargo test -p paintcraft-formats --lib ase::
```

`oracle.py --input <export.ase>` verifies actual PaintCraft output against the
mixed fixture's expected groups, names, types, models and every channel's f32
bits. It also checks empty groups and negative zero. The optional oracle is not
silently included in default Rust test counts and is not Adobe-application
qualification. The 2026-10-25 Python 3.14 / swatch 0.4.0 check passed all six
swatches. No display color accuracy, CMYK/LAB conversion, scratch-memory hard
limit or UI/cancellation latency claim follows from this resource interchange.

The adapter admits at most 1 MiB encoded data, 8192 blocks, 4096 swatches, 1024
flat groups, 1024 UTF-16 units per name and 256 KiB of aggregate name data.
Checks surround upstream codec calls and run per admitted block/resource item;
upstream calls do not have an internal cooperative cancellation callback.
