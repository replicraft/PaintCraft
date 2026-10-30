# Independent PCND4 filter-mask specimens

`generate.py` uses only Python standard-library integers/UUID bytes/SHA-256;
its parent is the already independently authored PCND1 literal raster specimen.
No PaintCraft writer or A8 constructor supplies expected bytes. `oracle.json`
records every whole-file SHA, accepted status, finite domain and literal samples.

Seven valid files cover canonical fractional A8 bands, disabled filter+mask,
stored zero coverage, mixed unmasked/masked instances, repeated identical
payloads, and an independently specified visible opaque half-strength plane, plus exact retained original ICC/source metadata. Repeated payloads are independent decode allocations even when a host
could have shared one mask. Raw source pixels include hidden RGB and remain
unchanged. Seventeen negative files recompute the outer SHA to exercise codec,
flag, canonical ordering/merging, domain, truncation and version boundaries.

The A8 codec is `a8-bands-v1` (wire ID1): signed domain endpoints followed by
band count; each band stores unsigned domain-relative y offset, positive height,
and nonzero run count; runs store unsigned domain-relative x offset, positive
width and coverage1–255. Outside all bands/domain coverage is zero. Adjacent
equal runs/bands must already be merged. A stored zero mask retains its domain.

These are persistence/admission specimens, not browser, CMS or visual-render
qualification. Unmasked PCND1/2/3 bytes retain their existing framing. PCND4
readers must receive an explicit shared coverage realm before mask allocations.

Historical classification note: `future-v5.pcnd` and its oracle/generator error
label are retained byte-for-byte from the PCND4 qualification checkpoint, when
version5 was unknown. The current PCND5 reader recognizes that header but rejects
this unchanged body as `codec.failed` because it contains no Box descriptor.
The independent PCND5 corpus now contains `future-v6.pcnd` as the actual unknown
version control. Do not rewrite the historical file/hash stream to relabel it.
