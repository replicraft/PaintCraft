# BF02: exact dense-span SIMD

## Decision and platform scope

Enable the widened u16 kernel by default through `simd-u16` on AArch64 NEON,
x86/x86-64 SSE2 and Wasm SIMD128 builds. Targets without those compile-time
features use the scalar opaque-destination specialization. Ordinary web builds
retain the scalar path; SIMD128 requires an explicitly SIMD-enabled build.
Mixed-transparency blocks and short tails keep the general scalar compositor.
Uniform skip/fill paths and cancellation boundaries remain unchanged.

Performance selection was measured on an Apple M1 Pro with 32 GiB RAM, Rust
1.99.0/LLVM 23.1.1, default release target features and no target-cpu=native.
Broader availability was explicitly requested after the native comparison.
SSE2 correctness was tested under x86-64 Rosetta. Wasm builds succeeded with and
without SIMD128. These checks do not establish native x86 or browser throughput.

## Why the implementation matters

Three explicit SIMD approaches were tried against both the frozen production
span compositor at 75ff9e3 and an optimized scalar opaque-destination specialization:

1. Channel-wise u32x4, extracting and recombining RGB independently.
2. Packed u32x4, processing red and blue in independent 16-bit fields.
3. Widened u16x8, processing two RGBA pixels per vector, with two vectors per block.

The first two improved on the old general loop but lost to specialized scalar.
The early u16 prototype also lost: generated code constructed fixed vectors with
six `memset_pattern16` calls in the opaque-block loop. Compile-time fixed vectors
and one opacity vector per span removed those calls. The corrected kernel then
beat both scalar contenders. No instruction count or speed advantage follows
merely from using a vector type.

Every weighted channel sum is at most 65025. The rounded division uses
`t = n + 128; (t + (t >> 8)) >> 8`, whose largest intermediate is 65407.
Thus u16 arithmetic cannot wrap in this domain. Independent u64 straight-alpha
oracles verify the result, including the original layer-opacity rounding.

## Complete-process observations

Each cell is the range of median wall times across three independent Hyperfine
sessions, with 20 timed runs and three warmups per contender. Session two reverses
contender/workload order. Setup, fixture construction, returned-buffer destruction
and process teardown are included. Units differ by workload and are stated here.

| Workload | Completed renders/process | Frozen scalar | Specialized scalar | Packed u32 SIMD | Selected u16 SIMD |
|---|---:|---:|---:|---:|---:|
| 1024 x 1024, five dense layers over opaque pixels | 4 | 87.88–96.76 ms | 59.07–65.63 ms | 68.51–70.89 ms | 40.94–41.95 ms |
| 1025 x 1025, five dense layers, odd edges | 4 | 92.97–93.76 ms | 63.90–64.35 ms | 72.33–73.28 ms | 44.65–46.09 ms |
| 17 x 17, five dense layers | 2000 | 17.48–18.73 ms | 15.09–15.89 ms | 14.32–16.01 ms | 10.03–10.46 ms |

Paired session medians show 28.4–36.1% less process time than specialized scalar
on the two large opaque workloads, and 50.4–56.6% less than the frozen scalar
implementation. The short workload uses 33.5–34.2% less time than specialized
scalar. These are complete CPU composition workloads, not input-to-display traces.

Steady-state Criterion runs corroborate the opaque gains. The corrected u16 pilot
measured about 6.5 ms per 1024-square composition, compared with about 10.7 ms for
specialized scalar, 13.2 ms for packed u32 and 13.9 ms for channel-wise u32 in their
recorded runs. Final selected-source samples are retained separately. Fixtures
are outside Criterion timing; returned output destruction remains inside.

The zero-render 1024-square process calibration was 13.4–14.8 ms depending on
binary/variant, including approximately 14.1 ms for u16. It is never subtracted
from process measurements. This substantial setup fraction explains why kernel
and process improvement percentages differ.

## Controls, regressions and resources

Mixed destination alpha and sparse controls did not show a repeatable >5%
regression against the frozen scalar. No speedup is attributed to unchanged
uniform handling. Original session three did show a 40.2% uniform wall regression:
98.57 ms versus 70.32 ms. CPU time increased 14.6%, instructions about 0.3%, and
the wall/CPU gap also increased. That observation is retained, not discarded or
proved harmless by source inspection.

A fixed three-session follow-up compared uniform and dense-opaque workloads with
the same immutable binaries. Uniform u16 medians were 70.59, 71.15 and 70.04 ms;
frozen scalar medians were 71.46, 69.18 and 72.27 ms. The range was 3.1% less to
2.8% more wall time, without repeating the >5% regression. The dense opaque gain
also repeated in every follow-up. This meets the declared local selection rule;
it does not identify the cause of the original scheduling/CPU variation.

The kernel uses fixed stack/register data and adds no heap allocation. Whole
process peak RSS varies with fixtures and allocator residency. For the aligned
large case, selected u16 medians ranged 33.88–42.09 MiB versus scalar's
37.95–42.10 MiB; the odd case was 48.73–48.92 MiB versus 45.96–49.41 MiB.
There is no general memory-reduction claim. Power, thermal state and unrelated
host activity were not controlled or instrumented.

## Correctness and qualification

- Exhaustive opaque RGB source/destination channel pairs and effective alpha,
  every opacity/source-alpha pair, and every bounded divide-by-255 input.
- Mixed destination alpha, hidden RGB, unaligned slices, zero/short spans,
  vector tails and 128/256 tile edges against an independent u64 oracle.
- Whole-document row-copy/frozen-span/production parity on dense, sparse,
  uniform, clipped and odd-width fixtures; existing region/cache/cancellation tests.
- Scalar fallback tests, 31 required real Metal parity tests, native image workflow
  tests and 69 browser-host library tests.
- x86-64 SSE2 kernel tests under Rosetta; portable and SIMD128 Wasm checks and
  full browser release builds. Native x86 throughput and browser execution remain
  unqualified. Formatting, strict renderer Clippy and workspace checking passed.

The source-matched ARM selection has 1,440 primary process observations and 480
fixed follow-up observations. Separately archived Criterion samples, initial
comparisons, excluded prototypes, compiler output, source/binary hashes, build
metadata and zero-work calibration remain locally under
`benchmarks/results/wide-compositor-2026-10-24/`. Generated results are ignored
by Git; maintained decision records remain tracked here. The subsequent dispatch
extension changes target selection, not the measured ARM arithmetic or loop.

BF02 remains partial: native x86/browser measurements, bandwidth/allocation
profiling, supported future adjustments/FP32 kernels and interactive commit/display
latency require their own evidence. Other `wide` candidates and the current glam
and epaint responsibilities are described in [graphics libraries](../../GRAPHICS_LIBRARIES.md).
