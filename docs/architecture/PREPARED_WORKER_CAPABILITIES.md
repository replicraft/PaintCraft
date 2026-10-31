# Prepared worker capabilities

**Status: planned, not implemented.** This is a contract draft for review. It does
not register preview Source5, export Worker7, or any additional execution support.
The current Source4 and Export6 capabilities remain closed to their original eight
numerical profiles. General PCND6 decoding may support Exposure independently.

## Separate decoding from execution

A generic PCND reader recognizes typed numerical intentions. An execution worker
must additionally resolve every required `(profile_id, codec_id)` through an explicit
priced implementation registry. Registry contract1 describes that resolution contract;
it is not a frozen nine-name allowlist. Each registered entry supplies:

- A strict transport decoder into the numerical profile's typed parameters.
- A compiler/evaluator, including an explicit direct-evaluation path when no program
  compilation is needed, and its immutable numerical semantics.
- A resource-contract identifier and revision covering compilation, retained program
  storage, temporary peak, evaluation scratch and modeled work.
- Quote and preparation hooks that use the admitted parent scope and cumulative
  work counter, with a canonical order for unique program preparation.

Adding an algorithm only to the general reader cannot grant execution support.
Every future registry addition requires implementation and pricing review, while
preserving this contract need not bump the outer protocol for each point algorithm.
An asset-bearing or otherwise incompatible ownership/framing model may require a
new protocol. No claim is made here about supporting the entire upstream catalog.

A bounded capability reply advertises registered pairs and their resource-contract
IDs/revisions. A client checks the required subset; the receiving worker independently
resolves and validates it. An absent decoder, implementation or resource contract
refuses with CapabilityUnsupported, including for disabled or identity intentions.

## Requirements and private candidates

For an actual PCND6 file, source requirements are the exact used on-wire manifest.
The generic reader verifies canonical framing, known codecs, parameters and exact
used inventory. For PCND1..5 there is no catalog manifest: derive *normalized*
requirements from typed legacy filter records, using the canonical numerical IDs
and their scoped `params-v1` transport representation. Do not report this derived
inventory as an on-wire manifest or reinterpret legacy parameter bytes as that codec.

Export7 uses the frozen source inventory. Source5 retains three inventories:

1. Immutable source requirements, unchanged for the lifetime of the opened owner.
2. Requirements of the prospective private candidate after its captured operation.
3. The sorted unique union of those two inventories, used for capability resolution.

Replacing/removing an old filter does not remove the need to adopt its source;
adding a new filter requires capability before candidate construction. A preview-off
request still carries and checks its candidate requirements. Candidate requirements
must be computed from the strict typed operation and actual base, not trusted from
caller declarations. A source may contain 256 distinct pairs under existing PCND6
framing; a single Add/Update can introduce at most one additional pair, so the
Source5 union bound is 257. Multi-operation staging is outside this draft.

All algorithm inventories include disabled filters and zero/default parameters.
Stored disabled masks remain intent too. Mask presence/enabled/domain/digest remain persistent user intent;
mask bytes are not algorithm parameters or compiled-program identity. Program keys
include numerical profile and canonical semantic parameter bytes, independently of
accepted transport codec. Two codecs yielding identical typed parameters share a
program key and semantic identity.

## Canonical capability hashes

SHA256 inputs below are binary bytes. All lengths/counts are unsigned big-endian.
Identifiers are validated ASCII using the catalog identifier rules; numerical profile
length is 1..96 bytes, codec length 1..64. Lists contain unique pairs sorted by the
lexicographic unsigned bytes of `(profile_id, codec_id)`, comparing the first ID
before the second. No JSON serialization, padding, trailing bytes, NUL terminators,
allocation size, registry iteration order or transport preference enters these hashes.

A required-list digest is SHA256 of:

```
ASCII "paintcraft.worker-required/v1\0"
u16 pair_count
repeat pair_count times:
    u16 profile_id_byte_length
    ASCII profile_id
    u16 codec_id_byte_length
    ASCII codec_id
```

An empty list is valid and still has this domain/count digest. Export7 permits at
most 256 pairs; Source5 union permits at most 257. Source5 independently binds the
source, candidate and union required-list digests. Export7 binds its one source list.

A resolution digest is SHA256 of:

```
ASCII "paintcraft.worker-resolution/v1\0"
u16 registry_contract = 1
u16 resolved_pair_count
repeat required pairs in the same canonical order:
    u16 profile_id_byte_length
    ASCII profile_id
    u16 codec_id_byte_length
    ASCII codec_id
    u16 resource_contract_id_byte_length
    ASCII resource_contract_id
    u32 resource_contract_revision
```

Resource contract IDs follow the same validated ASCII identifier syntax and 1..96
byte bound; revisions are positive. An ID/revision fixes the implementation's complete
quote/evaluation contract for that numerical pair, including direct evaluation or
compiler behavior. Any change invalidating previously quoted facts requires a new
revision. This hash binds only the required subset, not every known registry entry:
adding an unrelated registered algorithm does not invalidate an old request. There
must be exactly one resolved entry per required pair, never a caller-supplied fallback.

These transport capability hashes do not replace Document's canonical semantic hash.
Source SHA, filter-stack digest and mask digest retain their existing meanings.

## Export7 framing proposal

New Q07/R07 magics, protocol version7, request448 bytes and response608 bytes.
The common prefix retains the existing field layouts and authority semantics at
request bytes0..256 and response bytes0..416, with the newly identified Q07/R07
magic/version and request-specific values. Unchanged captured authority fields are
copied exactly; old protocol encoders/decoders remain byte-exact. The separate
region64 stays at request256..320 / response416..480, with a new explicitly versioned
region contract2. Existing Export6 `ScopeFacts` and the old protocols' region/catalog
fields retain their exact bytes and meanings; none of their reserved bytes is reused.

The new generic prepared128 occupies request320..448 / response480..608. All
integers are big-endian and all reserved bytes must be zero:

| Offset | Width | Field |
|---|---|---|
| 0 | u16 | Prepared schema1 |
| 2 | u16 | Registry contract1 |
| 4 | u32 | Distinct required numerical/codec pairs |
| 8 | u32 | Stored filter instances, all algorithms |
| 12 | u32 | Enabled filter instances, all algorithms |
| 16 | u32 | Unique prepared program keys |
| 20 | u32 | Reserved zero |
| 24 | u64 | Compilation work |
| 32 | u64 | Retained program bytes |
| 40 | u64 | Preparation peak bytes |
| 48 | u64 | Evaluation peak bytes |
| 56 | u64 | Combined modeled work |
| 64 | 32 bytes | Required-list digest |
| 96 | 32 bytes | Required-subset resolution digest |

Counts at8/12 are stored and enabled **intent** counts, including legacy algorithms
that need no compiled program. Count16 includes only actual unique retained prepared
keys; enabled legacy instances may be nonzero while prepared keys/compile bytes are
zero. Disabled instances require capability, but do not compile merely because they
are stored. Evaluation may omit hidden/off-canvas work only under the registry's
explicit visibility semantics; capability cannot omit their stored requirements.

The same128 bytes are echoed in successful and refused responses where the existing
response contract echoes captured request context. The Main host compares the entire
captured context, all original nonce/settings/source/meta/mask/bake fields, and current
owner/intent. The worker recomputes the inventories, resolution and resource facts
from the actual decoded document before evaluation. No caller context authorizes a
missing implementation or upgrades an old profile.

Version7 is a negotiated capability, not a permissive automatic fallback. Protocol4,
5 and6 stay their separately priced and closed profiles with immutable fixtures.
Exact Source5 message field names, and selection between legacy/generic profiles in
the host service, remain integration decisions; they must preserve the binary facts
and distinct inventories defined here. This document alone adds no accepted messages.

## Failure ordering and resource ownership

1. Validate bounded control framing, protocol/profile, lengths, owner and generation.
2. Authenticate source bytes using the existing digest pass. Do not add a redundant
   full-source pass to discover capabilities. Validate the recognizable source header
   against the selected profile; malformed magic follows the ordinary parser path.
3. Validate borrowed bounded manifest/legacy records and resolve requirements before
   retaining source metadata, masks, layers, tiles or programs. Unknown required
   entries refuse even if disabled/default. PCND3..5 normalization walks borrowed
   interleaved filter records and skips source attachments, canonical masks and encoded
   tile slices; it cannot normalize by first adopting a full document. PCND1/2 have
   no filters and normalize to an empty list. Quote and admit bounded assessment/control
   scratch before any allocated normalized requirement inventory.
4. Reserve the existing operation budget for parser and evaluation phase maxima;
   adopt with the same caller-owned metadata/coverage realms and retain their leases.
   Parse full typed intent and recompute exact used inventories, subset resolution,
   mask/source facts, source-size-dependent quote and captured copy policies.
5. Before a Source5 candidate, independently validate the operation, source base,
   stable references, candidate requirement union and its resource facts. Staging is
   private: no history/publication and no borrowed source mutation.
6. Compile canonical unique keys once per export frame/private preview generation.
   Share the parent scope and cumulative work counter across compilation, regional
   evaluation, thumbnail and final output. Finalize exact source-length facts after
   the immutable serialized snapshot exists, never by substituting the wire maximum.
7. Revalidate captured authority and live generation/owner/explicit intent after
   await and before publication. A response cannot mint bake/privacy authority.

No increases to DDS192MiB, MSP160MiB, parent workspace64MiB or modeled work268435456.
Admission includes actual source/control copies, manifest/registry scratch, program
retention, preparation and regional scratch, output and response bytes in their
actual overlapping phases. Compilation and region work use the same parent counter.
Use the parent factory with minimum child limits; fallback is permitted only if no
current factory exists, with the already reserved operation scope. Never allocate a
fresh independent realm per layer/render to bypass the caller's aggregate quota.

All helper callbacks latch the first cancellation failure. A later successful helper
return cannot erase it. Close/cancel/stale replacement retires workers/frames/program
leases and prevents publication; older outstanding workers must retire before a new
owner reuses the resource slot. One canonical ABI-specific quote must be used on
both producer and consumer; native harness equality is not proof of Wasm32 equality.

## Qualification required before implementation is advertised

Preserve all old protocol packets and fixture bytes. Test old Source4/Export6 with
reader-known Exposure (signed, disabled, zero/default) and require the exact consumer
closure failure. New protocol tests cover normalized legacy requirements, actual
catalog inventory, source/candidate union, decoder-known but implementation/pricing
missing, unknown disabled/default requirements, duplicate/sorted/framing errors,
subset-hash independence from unrelated entries, quote/context/count tampering,
parent quota/cancellation, stale owner/generation, retained frames and explicit-copy
consent revocation. Verify final facts with real MainWasm32-generated requests and
actual WorkerWasm32 evaluation; keep source/history/private intent unchanged on
refusal and make exact rendered samples independent of the implementation oracle.
