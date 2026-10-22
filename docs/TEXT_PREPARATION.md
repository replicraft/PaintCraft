# Cancellable text preparation

The deterministic bitmap-text command now streams its clipped spans into the
existing bounded PaintPlan instead of collecting an intermediate geometry Vec.
`font::visit_layout` accepts a fallible sink and checks the worker token at entry,
for every character/glyph row, before every emitted span and at completion. Newline
and entirely offscreen text still check cancellation and unsupported characters.
The sink must discard private partial output on any error.

Origins/cell arithmetic use i128 and only clipped coordinates narrow to i64.
Whole-pixel scaling, least-significant-bit glyph orientation, glyph/run emission
order, newline/column advances and unsupported-character diagnostics remain the
same for the admitted text domain. The checked visitor enforces the existing
1024-character/1–16 scale limits. The compatibility collector remains available
for callers that own their allocation policy.

Resolved grayscale masks use `validate_mask_spans_checked`, sharing the existing
coordinate, sorted/disjoint, nonzero-coverage, span-count and pixel-work policies.
Validation checks at entry, every 1024 spans and completion. Editor cumulative
work accounting and clipping also check every 1024 spans, including rows rejected
as offscreen. Span endpoints are subtracted in i128. Parser callers retain the
original noncancellable validator, which delegates to the same policy.

Text still reaches operation-wide admitted paint replay and atomic session
publication. A failed sink, unsupported glyph, metadata quota or cancellation
cannot publish an earlier private batch prefix. Metadata denial remains a fixed
resource error without decoded-capacity retry context. No dependency, command
schema, font selection, numerical policy or file format changes.

The bitmap visitor is distinct from the frontend's installed-font Cosmic Text/
Swash pipeline. This slice does not add editable text, change native antialiasing,
qualify IME or make all shaping/font loading preemptible. egui's retained drawing
geometry, inputs, legacy collectors, installed-font resources and aggregate batch/
service memory remain outside this plan's allowance.

Six new tests compare emitted order against the existing collector and required
pixels against independent glyph-bit sampling over scales, clips and origins.
They cancel every observed visitor/validator checkpoint, stop at the first failed
sink, test signed endpoint origins and offscreen unsupported text, cancel late
mask traversals and preserve owner/history/leases after private prefixes. Fixed
text metadata exhaustion and unsupported glyph rollback also pass.

[The validation record](validation/text-preparation-2026-10-21/validation.json)
pins commands, sources and exact compressed logs. Work-count intervals and streamed
geometry are correctness baselines: callback/visitor overhead, throughput, RSS and
native input/display/cancellation tails are not measured or qualified by this
slice. Earlier benchmark records remain frozen to their own sources. Whole-batch
planning and aggregate admission remain the next resource-management work.
