# Library evaluation

Reviewed 2026-10-24 against the current checkout and upstream crate documentation.
These are PaintCraft-specific adoption decisions, not benchmark results. A library
appearing in `Cargo.lock` does not mean PaintCraft code directly uses it.

## Decisions

| Library and upstream documentation | Decision | Concrete use and constraints |
|---|---|---|
| [wide](https://docs.rs/wide/latest/wide/) | Implemented and locally measured | Dense RGBA compositor spans in `paintcraft-render/src/lib.rs`. Three SIMD implementations were compared against frozen and specialized scalar loops. NEON, SSE2 and SIMD128 builds select the hoisted u16 kernel; targets without those features retain scalar behavior. Performance is locally measured on M1 Pro. Uniform skip/fill paths and scalar tails remain. See the BF02 decision. |
| [binrw](https://docs.rs/binrw/latest/binrw/) | Preferred candidate for future structured binary formats | Derived endian-aware readers/writers could simplify future PDN/legacy container records. Current PCND ZIP admission cross-checks local/central records, CRCs, overlaps and bounds; replacing it requires equivalent tests and explicit resource limits. Keep `zerocopy` for current bounded fixed-layout header views. |
| [deku](https://docs.rs/deku/latest/deku/) | Defer until a bit-oriented format requires it | Useful for declarative bit-level binary reading/writing. Current image codecs belong to `image`, and the stored ZIP subset is byte-oriented. Avoid adopting both this and binrw for the same records. |
| [smallvec](https://docs.rs/smallvec/latest/smallvec/) | High-priority allocation experiment | Short changed-region lists and compositor key dependency lists in `paintcraft-render/src/cache/key.rs`. Measure actual length distributions before choosing inline capacity; spill allocation, larger enclosing structs and budget accounting still count. |
| [arrayvec](https://docs.rs/arrayvec/latest/arrayvec/) | Use selectively at proven bounds | Fixed-capacity geometry scratch is a candidate. Use fallible insertion when input can exceed capacity; an ordinary array is sufficient when every slot is always used. Arbitrary document layers and path vertices have no small fixed bound. |
| [dashmap](https://docs.rs/dashmap/latest/dashmap/) | Defer pending contention evidence | Potentially useful for an independently keyed immutable-resource registry. Document publication, job admission and cross-entry invariants currently require coordinated state. Sharding does not replace that coordination; do not hold map guards across reentrant operations. |
| [indexmap](https://docs.rs/indexmap/latest/indexmap/) | Use when insertion order is a requirement | Suitable for future user-ordered presets/effect parameter registries. Keep `BTreeMap` in canonical JSON, which requires sorted keys. Keep explicit document order in the document model. |
| [bitvec](https://docs.rs/bitvec/latest/bitvec/) | High-priority memory experiment | Dense binary selection tiles currently store 0/255 in `Vec<u8>` in raster selection masks. Packing can reduce their sample payload to one eighth; it does not imply an eightfold total memory or speed improvement. Preserve sparse/solid tiles, inversion, offsets, R8 row adapters, accounting and cancellation. Flood fill already uses packed `u64` bitmap scratch. |
| [winnow](https://docs.rs/winnow/latest/winnow/) | Defer to a new grammar | Useful parser combinators for a future text/interchange grammar. Current strict JSON has deliberate duplicate-key, integer, surrogate and nesting rules. Replacing it needs a concrete maintenance or diagnostic benefit plus full contract coverage. Fluent parsing is already delegated to Fluent. |
| [bilge](https://docs.rs/bilge/latest/bilge/) | Defer until packed fields exist | Suitable for exact-width packed bitfields in a future format. Current byte-aligned headers do not justify it. Distinct from a set of independent flags; use explicit endian conversion at serialization boundaries. |
| [bytes](https://docs.rs/bytes/latest/bytes/) | Evaluate for shared encoded slices | `Bytes` can share subranges cheaply when encoded file blocks must be retained independently. Existing immutable history leases and raster ownership carry admission/lifetime semantics that it cannot replace. Slicing can pin the entire backing allocation; account retained storage rather than slice length. |
| [bitflags](https://docs.rs/bitflags/latest/bitflags/) | Use for an actual combinable flag contract | Candidate for future plugin/format capability sets. Current enums, stable capability names and booleans should remain unless independent bit combinations improve the API. Choose explicit handling of unknown bits for each serialized contract. |
| [memchr](https://docs.rs/memchr/latest/memchr/) | Low-cost scan experiment | ZIP EOCD search in `paintcraft-formats/src/zip.rs` scans backward over at most the ZIP comment window. `memmem::rfind` or reverse candidate iteration may help. Continue validating comment length and every earlier candidate when a later signature is false; benchmark this bounded workload. |
| [moka](https://docs.rs/moka/latest/moka/) | Retain as a cache comparison candidate | Potential larger concurrent derived-resource cache. Current compositor cache is a bounded Vec/LRU with explicit key and allocation-capacity charges. Moka eviction policy is eventually consistent; cache weights alone cannot enforce PaintCraft's physical allocation/lease budget. Keep admission accounting independent and check browser target support before sharing implementation. |
| [parking_lot](https://docs.rs/parking_lot/latest/parking_lot/) | Contention experiment, no blanket migration | Candidate for hot resource-budget/runtime locks after measuring contention. Current shared budgets use short standard Mutex sections and explicitly recover poisoned locks. Review panic recovery, fairness, lock ordering and browser behavior before changing primitives. |
| [fst](https://docs.rs/fst/latest/fst/) | Defer | Appropriate for a large immutable searchable string catalog. Current font names, tool lists and seven locale catalogs do not establish that need. Its construction/storage complexity should be justified by an actual catalog workload. |
| [thiserror](https://docs.rs/thiserror/latest/thiserror/) | Adopted | Root workspace dependency 2.0.21, consumed by `paintcraft-core`. Derives the existing typed `Error` and `JsonError` implementations while preserving diagnostic display, error codes, operation indexes, equality and error/v1 reporting. No new runtime error-erasure layer. |
| [anyhow](https://docs.rs/anyhow/latest/anyhow/) | Reserve for host-only contextual failures | Appropriate when an application boundary needs a heterogeneous error chain without a stable machine contract. Current CLI dispatch reports PaintCraft codes and exit statuses, so retain typed errors there. Introduce only when a real host orchestration path benefits from context/source chaining; do not erase engine errors or add an unused dependency. |

## First wide experiment

The current compositor iterates dense tile bytes one pixel at a time, applies
layer opacity, then calls `Rgba::source_over`. Straight RGBA8 composition divides
by an alpha-dependent denominator. SIMD multiplication alone will not remove
that cost, and float conversion or reciprocal approximation can change rounding.
`wide` integer operators also wrap, so intermediate bounds must be proved.

The implementation starts with the narrower opaque-destination case, where source-over reduces to
`(source * alpha + destination * (255 - alpha) + 127) / 255` per RGB channel.
Here alpha already includes the existing rounded layer-opacity adjustment.
It uses widened integer lanes and exact division-by-255 arithmetic. Exhaustive
channel/alpha/opacity checks and document oracles verify equivalence to the
straight-alpha formula. Partially transparent destination blocks retain the
general scalar kernel. Three measured SIMD implementations and their selection
are documented in the [BF02 decision](benchmarks/wide-compositor-2026-10-24/DECISION.md).

Acceptance requires:

- Scalar-oracle parity for every source/destination alpha combination and opacity
  boundaries, channel extremes, transparent hidden RGB and rounding boundaries.
- Unaligned slices, short spans, vector tails, clipped tiles, sparse defaults and
  multiple ordered layers. Preserve cancellation checks and layer rounding order.
- Optimized scalar and candidate measurements including result destruction on
  dense mixed-alpha, opaque, short and sparse control workloads. Capture allocation
  and throughput evidence; retain regressions and ties.
- Desktop ARM64/x86-64 and browser build coverage. Check generated code and target
  features: a vector API does not establish SIMD execution, runtime dispatch or
  a portable speedup. Keep a baseline that works without Wasm SIMD features.
- End-to-end brush/zoom timing before connecting a compositor microbenchmark to
  interactive responsiveness. Geometry, scheduling, upload and UI work may dominate.

Follow [the benchmark protocol](development/HYPERFINE_PROTOCOL.md) and existing
[CPU compositor evidence](CPU_COMPOSITOR.md). BF02 now has partial local evidence; cross-target performance and native input-to-display qualification remain pending.

## Dependency footprint at review

Already locked transitively: `arrayvec` 0.7.8, `bitflags` 1.3.2/2.13.2,
`bytes` 1.12.1, `indexmap` 2.14.2, `memchr` 2.8.3, `parking_lot` 0.12.5,
`smallvec` 1.16.3, `winnow` 1.0.4 and `thiserror` 1.0.69/2.0.21.
`thiserror` was adopted directly by the review. The subsequent BF02 work adds
`wide` 1.7.1 in the renderer, with a target-feature-gated u16 default and explicit comparison features. No unused entries
were added to the workspace manifest. Future direct consumers must use
`dependency.workspace = true` and declare only needed features.

Next experiments: extend SIMD only for independently qualified kernels/targets,
then `smallvec` short-list allocation profiles, `bitvec` selection storage and
`memchr` bounded scans.
`parking_lot` and `moka` follow only if profiles show lock or cache pressure.

## Validation of the error change

Core and CLI tests: 67 passed, including CLI error/v1 reporting and allocation
context compatibility. Workspace checking, strict all-target Clippy for core/CLI,
Wasm web checking, formatting and whitespace checks passed. These checks validate
the error migration. Container experiments remain unimplemented and unmeasured;
subsequent SIMD validation is recorded with the BF02 decision.
