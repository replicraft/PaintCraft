# Independently serialized PCND source metadata

`scripts/pcnd_source_metadata_reference.py` writes these files directly in Python.
It neither invokes the Rust codec nor derives bytes from a Rust save. The shared
version-one reference writer keeps its original output when no attachment is
supplied; `legacy-v1.pcnd` is the existing 165-byte conformance specimen.

The three version-two specimens have an empty transparent 2 × 3 canvas and fixed
document/state UUIDs. Their attachment describes a synthetic TIFF RGB source,
page 7, original dimensions 3 × 2 and orientation 6 already materialized. The
source digest names an explicit placeholder byte string in the generator, not an
external TIFF file or an original application's output.

- `rgb-profile.pcnd` retains the exact existing `assets/profiles/sRGB2014.icc`
  bytes, plus resolution and orientation facts.
- `resolution-only.pcnd` has no ICC payload. Exact signed resolution rationals
  are 30000/100 and -3780/1, inch units with original TIFF unit code 2. The
  negative declaration must remain provenance rather than become physical DPI.
- `orientation-only.pcnd` omits resolution and ICC; their reserved wire fields
  stay zero.

The attachment header and original ICC bytes have their own SHA-256; the complete
PCND stream has its normal trailing SHA-256. `oracle.json` records every fixture
digest and the original ICC digest. Structural profile retention does not assign
that profile to working pixels or demonstrate a color transform.
