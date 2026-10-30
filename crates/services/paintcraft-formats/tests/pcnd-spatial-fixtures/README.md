Independent PCND5 source-preservation fixtures
============================================

`python3 generate.py` verifies all generated PCND files and `oracle.json` byte for
byte. `--write` deliberately regenerates this folder only. The Python stdlib
writer does not import PaintCraft, use a product serializer, or evaluate blur.
The retained ICC attachment comes from the existing independently authored
PCND2 `rgb-profile.pcnd`; its parent file, profile and header hashes are recorded.

This set has34 specimens:16 valid and18 invalid. Valid cases preserve radius0/2/32,
disabled/radius0 Box, optional half/zero/disabled masks, multiple Boxes/repeated
mask payloads, old point stages before/after Box, masks on old point stages,
negative layer origin, hidden source RGB, and exact original ICC metadata.
Stack/mask digests use independent canonical byte streams, not product outputs.

Container5/document5 is selected for ANY stored Box, including disabled/radius0.
It uses the PCND4 per-instance mask-presence grammar even if all masks are absent.
Algorithm7 carries exactly one unsigned radius byte0–32 and is accepted only in5.
Unknown8 refuses even when disabled. Version3/4 still reject algorithm7. Version5
without Box is noncanonical; actual unknown container6 is separately refused.
Invalid masks, enabled flags, ranges, integrity and trailing bytes remain errors.

The source raster is2×1 straight RGBA8, separate from any evaluated appearance.
The source color attachment remains provenance, not an activated working profile.
These are wire/model/admission specimens, not visual or CMS qualification. The
8MiB aggregate metadata ceiling and explicit shared coverage admission remain.
The Box descriptor adds no pixel payload. All preexisting1–4 fixtures are untouched.
