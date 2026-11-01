# Scoped noise workloads

Status: approved design; implementation and qualification are in progress.
No new protocol or higher workload allowance is enabled by this document.
The [numerical foundation](NOISE_FILTER_FOUNDATION.md) is qualified separately.
Its Uniform/Gaussian prices remain 512/4,096 modeled units per candidate tuple.

## Why a distinct grant is needed

One nonspatial noise stage on the default 1000×486 canvas has a conservative
two-pass quote of 497,664,000 Uniform units or 3,981,312,000 Gaussian units,
before composition. Both exceed the browser's existing 268,435,456-unit job
limit; Gaussian exceeds the native/CLI/MCP 1,000,000,000-unit limit as well.
Smaller execution tiles address per-step bounds, not this aggregate limit.
Neither reducing the reviewed unit prices nor increasing a legacy global limit
is the selected solution.

The new workload class is `noise-source-work-v1`, revision 1. Each admitted job
owns two labeled cumulative components:

| Component | Browser/export maximum | Native/CLI/MCP maximum |
|---|---:|---:|
| Base work | 268,435,456 | 1,000,000,000 |
| Noise work | 34,359,738,368 | 34,359,738,368 |

Base work still includes all existing filters, preparation, coverage/control,
composition and resolve work. Only the registered `DirectNoise` tuple price
enters the noise component. Disabled or neutral noise cannot authorize extra
base work. Each total is a checked sum, not an independently spendable allowance.
Policy selection is trusted endpoint behavior; a browser request cannot select
native limits. Child limits are componentwise minima and retain prior usage.

The 64 MiB appearance byte realm and existing output/source/metadata limits
remain unchanged. Old protocols retain their 8,388,608-unit per-step limit.
Source6 is qualifying a separate fixed policy: 64×64 native/display patches,
8,388,608 base units and 134,217,728 Noise units per synchronous slice. The public
qualification constructor cannot exceed that Noise slice ceiling; no wire field
selects a larger policy. Actual browser timing/cancellation gates must pass before
ordinary delivery is claimed.

Gaussian before radius-32 Box needs a 65×65 source prefix even for one interior
output pixel, exceeding the old aggregate slice limit. The new policy admits that
prefix without reducing its price. At full 1920×1080, the canvas-aligned64 patch
quote has 510 patches and 32,541,507,584 Noise units for this compound case, below
2^35. This is an independently checked geometry quote, not an observed complete
frame or universal stack allowance. Multiple stages and repeated halos sum
normally; final execution and actual slice cost require their own evidence.

## One owned job and explicit lifetime

`NoiseAdmission::new_root` creates the legacy base admission and noise sidecar
together from the supplied existing scope. It refuses an installed parent; there
is no attach-to-parent, reset or implicit larger fallback API. Capture reuses a
matching installed pair, or lazily constructs a root only when both are absent.
An installed legacy-only scope cannot be upgraded inside a renderer/export helper.

Children, prepared bindings and worker submissions retain the same base counter,
noise counter and byte scope. Combined reservations validate both components and
debit neither on denial; completed work is never refunded. Nested installation
requires the same owners, applies minimum limits and restores context on return,
error and unwind. Rayon and asynchronous work capture both contexts on submission
and install both on the executing thread. An absent captured context clears an
unrelated worker context.

Charge the new sidecar, Arc/control storage and retained frame handles before
allocation under the repository's declared logical ownership model. Their real
per-ABI sizes belong to the new contract. No new byte pool, unbounded retained
clones or RSS guarantee is implied. Keep old factory/quote/frame layouts intact;
surface any unavoidable ABI or resource-model change for explicit versioning.

## Protocol and caller boundaries

Source6 and Export8, with registry contract 2 and explicit workload negotiation,
will carry the new authority. Source2/3/4/5, Export4/5/6/7 and registry contract 1
retain their existing ceilings and closed behavior. PCND6 and the noise six-byte
parameter codec do not need a container bump for this execution policy.

Capture the class ID/revision, component limits, checked total and binding digest
before source adoption. Quotes expose independently derived base/noise/total facts;
execution recomputes them from validated policies and actual reverse-planned
regions. Untrusted component claims are never accepted as subtraction authority.
Requirements, source/candidate/union digests, masks, source privacy, generation,
target, nonce and bake consent remain separately bound. Quote-only requests do
not create execution counters, frame pixels or prepared programs.

Native preview jobs select their root from frozen source and prospective typed
edits. CLI/MCP select after strict typed capture and before installing a parent;
MCP keeps its executing-thread and session-lock boundaries. Reparse of different
mutable input after grant is not an equivalent capture. Ordinary synchronous
clipboard, print and export paths require admitted background work before taking
the larger noise allowance. Until migrated, refusal must be explicit.

## Completion evidence

The preserved twelve-profile baseline has 59 cases per native/Wasm platform and
eight genuine Main export packets. Compare every field after integration,
including all historical 43 cases, layouts, work, callbacks, accounts, caches,
snapshot facts and contexts. A schema or tiny image pass cannot replace this.

New tests must cover component minima, depleted parents, simultaneous spending,
legacy-parent refusal, missing/mismatched grants, context restoration, actual
storage release, reverse halos, one debit per stage and malicious requests that
attach disabled noise to an excessive legacy workload.

Usability gates include default 1000×486 and actual 1920×1080 workloads for both
distributions, mono/color, and amounts 0/10/100. Test 4 MP admission where existing
memory limits allow. Targets to measure are default completion within 2 seconds,
1080p within 5 seconds, cancellation acknowledgment within 100 ms and browser
execution slices within 50 ms. These are acceptance targets, not observed results
or universal deadlines. Real App/Main/Worker adoption and interaction need their
own evidence; kernel statistics cannot establish responsive UI behavior.
