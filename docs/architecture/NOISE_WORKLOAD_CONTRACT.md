# Scoped noise workloads

Status: paired admission and Source6 are implemented. Native, CLI/MCP and a
production Studio browser checkpoint are qualified; remaining browser consumers
and Export8 are still in progress. This document does not itself grant authority.
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
Source6 uses a separate fixed policy: 64×64 native/display patches,
8,388,608 base units and 134,217,728 Noise units per synchronous slice. The public
qualification constructor cannot exceed that Noise slice ceiling; no wire field
selects a larger policy. Actual browser timing/cancellation gates must pass before
ordinary delivery is claimed.

Gaussian before radius-32 Box needs a 65×65 source prefix even for one interior
output pixel, exceeding the old aggregate slice limit. The new policy admits that
prefix without reducing its price. At full 1920×1080, the canvas-aligned64 patch
quote has 510 patches and 32,541,507,584 Noise units for this compound case, below
2^35. The complete native/Chrome qualification matrix now includes this case;
its exact scope and finite timings are recorded in the numerical foundation.
This is not a universal stack allowance. Multiple stages and repeated halos sum
normally; later implementations and callers need their own execution evidence.

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

Source6 carries registry contract 2 and explicit workload negotiation. Export8
remains a separate integration gate. Source2/3/4/5, Export4/5/6/7 and registry contract 1
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

## Encoded visual copies: flat-output1

Source6 has a separate, version-1 encoded-copy operation for PNG/JPEG/BMP
downloads and PNG printing/visible clipboard copies. It does not extend the
Source6 BEGIN fields, grant Export8, or replace ordinary display requests.

The action freezes the real owner, history ticket, source Arc, tool/font/paste
revisions, frontend, skin and output options. A changed private filter draft or
source invalidates delivery. The existing display client must acknowledge close
before the action acquires its single admitted slot. The action must then
acknowledge close before delivery or display resumption. A valid cached committed
texture may resume without a new Worker request. Uncertain close stays quarantined.

`copy-prepare` binds an ordinary level-0 BASE request to a strict output control:
`version`, `purpose`, `format`, `quality`, `matte`, `frontend`, `omitMetadata`,
`bakeFilters`, `maskLength` and `maskSha256`. Controls are at most 4,096 bytes;
unknown fields and invalid values refuse. Downloads and printing cover the whole
frozen canvas. Clipboard copies may use an in-bounds ROI in the original document
coordinate system, preserving source coordinates and filter halos.

Registry availability is checked before the actual source-metadata omission and
filter-bake policies. `AllowedWithPreflight` is not unconditional consent and is
not an unconditional denial: both source-bound policies must still validate.
Visual print/clipboard copies retain their existing exemption from foreign-file
conversion consent. They require PNG, quality 90, white matte and false omission/
bake flags. JPEG alone accepts a different RGB matte; quality is 1 through 100.

After admission, `copy-input` transfers an empty selection or a binary byte mask
of exactly one byte per output pixel. The control binds its length and SHA256.
All bytes must be 0 or 255 before any pixel changes. Masking occurs once after
composition, setting excluded RGBA to zero. Failed encoding retires the private
frame, candidate and binding without a JavaScript cleanup dependency; the accepted
generation remains stale and only a higher generation can start new work.

The encoded envelope binds the complete Source6 owner/source/draft/generation,
operation/view, source/candidate/union facts and work quote. `outputSha256` is the
canonical output-control digest; `bytesSha256` authenticates the encoded bytes.
MIME, extent and one-shot delivery are checked separately. The output account
survives synchronous bindgen conversion. A new generation or client close must
join pending copy finalization, including asynchronous byte hashing, before
releasing the slot. Current-intent checks run again after that hashing.

Limits remain 4 MP total, at most 8,192 pixels per side, and 16 MiB encoded output
minus selection-input length. The fixed-capacity writer refuses before growth.
The existing two Rust output slots retain RGBA and encoded bytes; the existing
16 MiB JavaScript slot covers selection input and encoded transfer. A 32 MiB
appearance reservation covers up to 4 MiB selection input, 12 MiB JPEG RGB and a
declared 16 MiB opaque codec allowance, within the same 64 MiB realm. These are
logical payload declarations, not allocator/RSS bounds. Opaque encoding is not
preemptible; no 50 ms encoding or cancellation guarantee follows.

Codec conversion uses a provisional model of 64 base units per output pixel plus
two base units per selection byte for validation/application. It spends on the
same paired admission through `reserve_base_work`, which still checks both child
ceilings and inherited usage. Noise prices remain 512/4,096; neither component
limit changes. The codec model is not an instruction/cycle bound or an optimized
configuration claim. Unsupported codecs and excessive byte/work quotes explicitly
refuse before output publication.

The raw production Wasm interface passes eleven finite cases, including nine
independently decoded output planes, hostile selection retirement/reuse,
one-shot delivery, consent and real low-base refusal. Native callback/overflow
tests are separate. Receipt
`docs/validation/noise-flat-output-browser/final-freeze.json`, SHA256
`441504e7ed1a0a2abf6406d992ea6a2793d7ba1fba5c9b55f5181723d9f6b870`,
pins that boundary. Actual Main download, clipboard and print delivery still need
their own browser evidence. Picker, flatten and other codec adapters remain open.

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
