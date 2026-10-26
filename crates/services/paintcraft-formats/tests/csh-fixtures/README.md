# CSH shape-resource specimens

These MIT repository-authored specimens are generated with Python standard-library
struct packing. No upstream fixture or implementation is copied. `paths.csh`
contains five shapes, duplicate names/IDs, Japanese/non-BMP names, nonzero bounds,
all five operation states, both even/odd encodings, nonzero fill, open/closed
subpaths, linked/unlinked knots, fractional/negative coordinates, clipboard and
initial-fill records. It has 45 ordered records, 10 subpaths and 20 knots.
`empty.csh` contains no shapes. `placement.csh` independently packs an
integer rectangle, a genuine two-knot cubic lens with a curved closing edge,
a subtract preset and a multiple-subpath preset. Placement tests use an analytic
interior oracle for the lens, exact rectangle pixels, orientation, fill rules and
explicit refusal of the latter two unsupported presets. These checks do not
claim Adobe rendering parity. `sources.json` records byte counts and hashes;
`expected.json` states all decoded geometry/metadata for the optional reader.

The wire representation follows the selected Rust `ag-psd` 0.3.0 CSH container
reader and [Adobe's path record specification](https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/).
Our adapter checks resource allowances, extents, strict names and record topology
before that reader runs. Fresh writing uses its container/name/ID headers with
an exact record-body adapter to preserve information its path writer discards.
Source name terminators/padding may be canonicalized; bounds and record bytes
remain exact. No output is published by a default test run.

Regenerate and run the Rust check:

```sh
python3 -B crates/services/paintcraft-formats/tests/csh-fixtures/generate.py
cargo test -p paintcraft-formats --test csh_resources --locked
```

For the additional JavaScript implementation check, install the pinned upstream
[ag-psd 33.0.0](https://github.com/Agamnentzar/ag-psd) in a separate environment:

```sh
mkdir -p /tmp/paintcraft-csh-oracle
pnpm --dir /tmp/paintcraft-csh-oracle add --save-exact --ignore-scripts ag-psd@33.0.0
node crates/services/paintcraft-formats/tests/csh-fixtures/oracle.cjs \
    /tmp/paintcraft-csh-oracle/node_modules/ag-psd \
    crates/services/paintcraft-formats/tests/csh-fixtures
```

Check actual PaintCraft exports too, with a fresh output path whose parent exists:

```sh
PAINTCRAFT_CSH_EXPORT_DIR=/tmp/new-paintcraft-csh-exports \
    cargo test -p paintcraft-formats --test csh_resources --locked
node crates/services/paintcraft-formats/tests/csh-fixtures/oracle.cjs \
    /tmp/paintcraft-csh-oracle/node_modules/ag-psd /tmp/new-paintcraft-csh-exports
```

The output directory must not already exist. The test creates it and uses
exclusive file creation; previous evidence is never overwritten. The optional
oracle refuses other JavaScript package versions, verifies the source specimen
hashes, compares every decoded metadata/geometry value against the owned
expectation and independently compares exact bounds/ordered record bytes with
the source. The latter covers origin and selector 6, which the JavaScript model
otherwise drops.

On 2026-10-25, Node 24.10.0 / JavaScript ag-psd 33.0.0 reopened both source
specimens and fresh Rust outputs successfully. `paths.csh` is 1,482 source bytes
and 1,466 fresh-output bytes; the difference is canonical name padding, not
missing paths. Rust also verifies semantic equality after writing. The JavaScript
and Rust readers share ag-psd ancestry, so this is additional implementation
evidence, not an independent-lineage oracle or Adobe application qualification.
This does not validate rendering/Boolean geometry, a hard scratch-memory bound,
cancellation latency, all CSH variants, or a public frontend workflow.
