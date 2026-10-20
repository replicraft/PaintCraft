# Active backend measurement protocol

This scoped owner applies to implemented compositor/cache/CPU partition workloads.
It adopts the relevant rules from the reference pack's benchmark protocol without
importing its unimplemented modules or dormant benchmark cases. The active case
registry is `benchmarks/cases.json`; drivers and execution artifacts live under
`benchmarks/`. This document does not turn a successful measurement into a release
or historical application-compatibility claim.

## Preconditions and provenance

Freeze equivalent required output, candidates, source policy and resource limits.
Run independent correctness and applicable cancellation/admission/race checks
before timing. Build optimized binaries first. Retain source revision, dirty source
identities, Cargo.lock, fixture/driver/binary hashes, compiler/target/profile,
features and relevant environment. Never time compilation or dependency download
as raster work. A process driver consumes/destroys real output and checks liveness;
its checks are separate from independent full oracle preflight.

Caller-owned worker pools have explicit thread limits. No hidden global pool or
nested codec/harness pool may silently expand the intended budget. Record pool
construction separately from steady render work, and include it in process time.
Record no-op/tiny/clipped cases and regressions rather than averaging unlike cases.

## Statistical and process tiers

Use a pinned statistical harness for operation latency and applicable allocation
profiles. Retain its raw per-sample export, configuration, summaries and outliers.
Criterion's raw sample JSON/CSV is used for new partition comparisons; its optional
Rayon analysis feature is disabled. Divan 0.1.21 remains useful for allocation
summaries but does not expose internal samples through its CLI. Existing Divan-only
artifacts therefore do not satisfy this raw-sample requirement; their archived
Hyperfine process evidence remains usable at its explicitly measured scope.

Use the existing Hyperfine runner for meaningful process comparisons. Application
decisions require at least 20 timed runs / 3 warmups in each of three independent
sessions, including forward and reverse order. Record explicit shell mode and
installed version. Prebuilt drivers batch the same number of completed work units
for all variants. Capture zero-work calibration without subtracting it from timing.
Report cold construction/teardown and its material fraction, even when it prevents
a short case from demonstrating a kernel benefit at process level. Never claim
process means as interactive p99 or display-frame rate.

Retain raw process exports, correctness/resource logs, failures, configs and
execution metadata. Verify immutable artifact identities before/after execution.
Lossless archives preserve original bytes and inner manifests. Keep exploratory
and failed attempts labelled as excluded. A decision states adoption, ties,
rejections, memory/CPU costs, platform limits and remaining qualification.

## Limits and repeat triggers

CPU/OS/thread count, memory, power/thermal/background-load limits and available
adapters accompany results. One developer host does not qualify other targets.
RSS, cache weights, live allocations and decoded source residency are different
metrics. GPU completion, durable saves and interactive latency require their own
actual completion boundaries and traces. A Rayon throughput result proves neither
scheduler priority nor responsiveness during interactive/background contention.

Repeat relevant comparisons after material kernel/layout/scheduler/ISA/compiler
or dependency changes, altered fixtures/policies or observed regressions. Do not
rerun dormant future cases for unrelated edits. Full BF06 still needs interactive
interference/cancellation-age evidence; current CPU partition measurements qualify
only the recorded bulk workloads. The original roadmap remains authoritative for
those unfinished gates.
