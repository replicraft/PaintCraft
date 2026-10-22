# Cancellable bounded flood classification

Flood fill observes the private raster produced by earlier operations in its batch.
It classifies a four-connected region before painting any of that operation's pixels.
Every RGBA channel, including hidden RGB and alpha, is compared against the original
seed value with the specified per-channel tolerance. Selection, canvas and layer
clipping remain editor responsibilities; classification consumes layer-local bounds.

## Cooperative work and scratch

`flood::region` retains its original signature and delegates to the checked path
with a successful callback. `region_checked` accepts cancellation. The explicit
`region_with_scratch_limit_checked` additionally sets a per-call scratch allowance.
Raster `Admission::with_flood_scratch_limit` carries that policy into editor execution
and survives admission cloning. Its default is a provisional 64 MiB.

The allowance covers the sum of retained bitmap, frontier and returned-span vector
capacities. Growth is fallible and checked against remaining declared capacity;
returned capacities are validated. Bitmap size is known and checked before its
allocation, then initialized in chunks of at most 1,024 u64 words (8 KiB). Other
vectors retain clamped amortized growth. Popped frontier slots remain accounted
until their allocation drops. Returned spans remain caller-owned after classification;
there is no global capacity lease or whole-batch scratch admission here.

Cancellation is checked on entry, before vector growth, during bitmap initialization,
at every popped frontier entry, after classification, and after every 1,024 scan/
mark/neighbor work steps. This includes wide rows that produce only one span. The
interval is a work-count safety policy, not a measured wall-clock response bound.
Allocation, OS page faults and the caller's callback can still block. A cancelled
call releases its private vectors and returns an error, never a partial region.

The existing 67,108,864-pixel area bound remains. Public signed clips are validated
in wide arithmetic before narrowing or bitmap indexing. Neighbor stepping checks
signed endpoints instead of overflowing at i64::MIN. These checks do not change
the common rectangle type's contract elsewhere.

## Editor integration and failure

The editor supplies its actual worker cancellation callback and admitted scratch
limit. The classifier's spans are already clipped layer-local coordinates, so they
compile directly into the existing ordered PaintPlan. The duplicate intermediate
vector that translated spans to document coordinates and back is gone. Span storage
drops before paint replay; retained spans and descriptor construction can overlap
and still have separate per-call/per-plan allowances.

Scratch exhaustion is a fixed `resource.limit`, without decoded allocation-denial
context. Freeing history cannot alter that fixed policy, so it must not enter the
native decoded-capacity retry loop. If an earlier operation already allocated private
tiles, classification failure/cancellation discards that prefix through the existing
Session boundary. Owner pixels, ticket, history and saved acknowledgement remain.
No new dependency, command schema, numerical policy or publication path is added.

## Evidence and remaining scope

Independent breadth-first traversal checks all 512 binary 3×3 images over every
seed, two clips and four tolerances (36,864 cases), including hidden RGB/alpha.
Further tests cancel at every observed checkpoint of a wide single-row fill, cancel
large bitmap initialization, deny bitmap/frontier/output growth, retry with sufficient
scratch and verify signed clip endpoints. Editor tests preserve prior staged walls,
negative layer origins and selections, carry fixed scratch policy through clones,
and cancel/fail classification after a private prefix without changing owner/history.
[Source-pinned validation](validation/flood-preparation-2026-10-21/validation.json)
records commands and logs.

These tests establish exact default-v1 behavior and cooperative checkpoints, not
Paint/Windows reference parity, p95/p99 latency, throughput or RSS improvement.
Allocator overhead/rounding and transient reallocations are outside capacity
accounting. The default and interval are not benchmark-selected optima. Whole-batch
planning, aggregate scratch/metadata admission, other command kernels, scheduling
fairness, physical pressure and native cancellation/display tails remain open.


[Read-only paint projections](PAINT_PROJECTION.md) now provide exact logical
pixels for data-dependent preparation without decoded replay. Ordinary painting
shares the bounded index builder, and flood accepts fallible immutable sources.
Aggregate program ownership and upfront whole-batch reservation remain open.
