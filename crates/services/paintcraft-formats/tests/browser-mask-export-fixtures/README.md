# Independent masked export planes

Python standard-library code authors PCND4 bytes, canonical A8/stack/inventory
SHA-256 streams and literal expected RGBA independently of PaintCraft codecs.
Ten files cover constant/full, fractional128, storedzero, disabledmask and
disabledfilter, each with/without exact original ICC/source attachment. Source
pixels remain opaque black/white. Source attachment is the independent PCND2
placeholder source specimen, not a new foreign TIFF or color-management oracle.

DDS can preserve fractional gray. MSP requires exact opaque black/white, so the
two fractional planes must refuse rather than threshold/dither implicitly.
`oracle.json` records these different existing format profiles explicitly.
Actual Worker/UI execution is a separate qualification step.
