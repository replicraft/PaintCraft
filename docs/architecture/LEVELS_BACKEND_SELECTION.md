# Prepared Levels backend selection

Status: qualified standalone preparation implementation. Levels still has no
registered Descriptor, native container, automation schema or frontend control.
The numerical policy remains `levels-rgb-v1`; this is an execution choice.

## APIs, storage and accounting

`paintcraft_effects::levels` remains the explicit u32 reference API, with its
existing quotes, limits and callback behavior. `levels::u64` supplies a separate
u64/u128 API with distinct Scratch, Quote, Limits and WorkUsed types. Both call
one private monomorphized sweep and fixed-integer engine through sealed word
operations. No dynamic dispatch, unsafe storage cast, heap arithmetic or large
integer copying is added. Transparent single-field scratch wrappers pin their
layout to the actual workspace checked by the engine.

`PreparedLevels` selects u64 when the **original u32 quote is at least 1,000,000**;
smaller workloads retain u32. This is a sampled heuristic, not a proof that every
parameter set or CPU improves. Its two separate non-inlined construction helpers
construct only the selected scratch; there is no large scratch-enum switch or
second simultaneous workspace.

The retained table remains 256 bytes. The byte quote uses the selected scratch's
actual local size. Native scratch is 6,696 bytes for either backend; qualified
Wasm scratch is 6,676 for u32 and 6,696 for u64. Caller output, Rust bookkeeping,
allocator overhead and physical RSS remain separate from payload accounting.

Raw operation counters have different word widths. `CompileWork::U32` and
`CompileWork::U64` keep that distinction explicit, including through the prepared
owner's report and backend getter. No untagged counter is reinterpreted as a
different unit. Shared render-job admission conservatively reserves the maximum
of the two raw quotes as an **abstract preparation allowance bounding either
counter**, not a conversion between equal-cost operations or elapsed time. Each
selected kernel independently enforces its own typed raw limits. Existing job
work/byte ceilings are not enlarged, and no new realm or fallback is created.

## Qualified code and measured scope

The final shared-engine code passes 44 Effects tests, both complete independent
oracle/resource suites, nine prepared-program tests, native all-target GPU
compilation checks including the example, and actual Web-consumer Wasm strict
checks. GPU compilation is not GPU execution of this filter. Raw Wasm execution
against the actual new Effects artifacts independently matches all 329,668
channel records, literal LUTs, ties and failure/publication controls.

After functionality passed, actual native and Wasm library artifacts were built
and checked against all 188 crossover LUT hashes before timing. Each platform
records 376 backend/case rows with 15 rotated samples. Actual libraries, not the
earlier copied prototype, supply these measurements. On the local Apple M1 Pro:

| Gamma 9.99, full endpoints | Actual u32 median ms | Actual u64 median ms |
| --- | ---: | ---: |
| Native | 15.019 | 2.194 |
| Chrome Wasm | 15.290 | 10.000 |

All 31 sampled cases selected by the 1M rule improved median elapsed time on both
platforms. That does not establish universal improvement. Refactored Wasm u64 is
slower than the earlier prototype's 6.665 ms; the prototype result is not reused
as implementation evidence. In selected case `g0999-e5`, the observed Wasm
p95/maximum is 0.680 ms for u64 versus 0.285 ms for u32, despite better median
0.215 versus 0.265 ms. The outlier remains in the evidence. No tail-latency or UI
performance guarantee follows. Clock-floor cases and failed attempts are retained.

The full measurement limitations and exact rows are in
`docs/validation/levels-optimization/refactor/measurement/REPORT.md`; the frozen
receipt is `final-freeze.json` in that directory, SHA-256
`68b444c55177070dd9ad94d1163358ec88df12f2c5e35fbb34208c3a685f1894`.
The functional source/log receipt is
`docs/validation/levels-optimization/refactor/functional-freeze.json`.
Before canonical release filenames were overwritten, 24 exact historical linked artifacts
were copied with verified bytes and distinct inodes, and their relocation map
was retained. Historical prototype sources, binaries and receipts stay immutable.

## Remaining work

This completes a prepared-table component, not the editable filter. The later
[eager job API](PREPARED_POINT_PROGRAMS.md#frozen-job-reuse) supplies bounded reuse;
host wiring, catalog persistence, typed commands/history/inspection and all
native/browser/CLI appearance consumers still need integration. Existing old
worker profiles stay explicitly closed. Actual UI, save/reopen, bake consent and
CPU/Metal display parity must be qualified before reporting an eighth filter.
