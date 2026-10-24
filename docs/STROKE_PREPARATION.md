# Bounded cancellable pencil and brush preparation

Pencil and stroke preparation now traverses borrowed segment windows rather than
copying the segment list. Smooth-stroke work accounting normalizes fixed-point
endpoints while reading them, without a second integer-point vector. Floor division,
shared-joint suppression, cumulative segment/pixel/stamp-row limits and numerical
results are unchanged. Wide deltas avoid signed subtraction overflow in path-length
accounting. Cancellation checks cover long vertex lists and every 1,024 Bresenham
steps, including invisible steps before the path reaches the editable clip.

## Unsmoothed constant-colour union

The editor uses `shape::CheckedSpanSet` for round/square unsmoothed strokes and
shape lines. It preserves the existing once-per-stroke coverage policy: overlapping
stamps produce one blend per covered pixel. It does not implement accumulating flow,
variable-pressure dabs or a universal brush policy. Rows are independent; output
runs are nonoverlapping and ordered in x within a row, with no specified row order.
Committed pixels and canonical document output remain deterministic.

The provisional default scratch allowance is 64 MiB, configurable through
`Admission::with_stroke_scratch_limit`. Declared capacity includes usable row-map
payload, every retained run vector and the merge heap. Growth is fallible and checks
actual returned capacities. Hash controls, allocator overhead/rounding, transient
reallocations, input samples and the separate PaintPlan allowance are not included.
This is a per-operation payload bound, not a global lease or physical RSS ceiling.

Stamp construction checks cancellation for each clipped row. Merge sorts only
chunks of at most 1,024 records between checks. Ordered chunk boundaries permit
direct merging; otherwise a bounded heap merges sorted chunks, checking every
1,024 consumed records and before each sink emission. Finished row vectors return
their declared capacity to this attempt's allowance. The heap remains accounted
while reused. This bounds the standard sort call instead of making a whole large
row an uninterruptible library sort. Intervals are work-count policies, not measured
wall-clock cancellation guarantees.

## Failure and publication

A failed stamp/merge invalidates the private attempt. A sink may already contain
private descriptors and must discard them on error. Fixed scratch exhaustion has no
decoded allocation denial, so freeing history cannot trigger an inappropriate retry.
All geometry reaches the existing bounded PaintPlan before tile replay/publication.
A later failure discards earlier private edits without changing owner/history state.

Earlier checks exposed native preview cleanup that depended on entering the smooth
geometry kernel. Once native staging owns its geometry-cache lock, any staging error
now clears that disposable scratch, including cancellation before the kernel call.
A request cancelled before acquiring that lock still returns immediately. No GUI
state becomes authoritative and no cancelled result bypasses publication guards.
Legacy SpanSet collection APIs remain for callers with their own allocation policy.

## Evidence and scope

Raster tests compare checked/legacy union with an independent stamp-centre oracle,
exercise disordered multi-chunk merging, cancel every observed merge checkpoint and
bound row vectors/merge heap under one allowance. Path tests cancel before reaching
the clip and during vertex accounting, preserve borrowed single/empty/jointed paths
and check fixed-point floor normalization. Editor tests verify retraced half-alpha
stamps blend once and fixed scratch failure releases a private prefix without
changing owner/history. The existing native preview regression covers early cache
retirement. [Validation](validation/stroke-preparation-2026-10-21/validation.json)
records committed source and exact commands/logs.

The [matched comparison](../docs/benchmarks/stroke-preparation-2026-10-21/DECISION.md)
retains statistical union samples and complete CPU-only CLI apply observations,
canonical output equality, slow controls, resource metrics and calibration. This
is a correctness-mandated bounded baseline; performance overhead is reported rather
than selecting an uncancellable/unbounded path because it times faster. Native
input/display/cancellation tails, other geometry/font kernels, whole-batch planning,
aggregate admission and physical-pressure qualification remain open.
