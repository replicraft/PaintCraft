# Selection Snapshot1 transport

Selection Snapshot1 is a locally qualified Raster foundation for transporting a
captured binary selection. It is private input to a future asynchronous mask edit,
not PCND persistence, a public mask-set operation, or a new Source7 message.
Browser mutation controls and a Worker consumer of this transport remain open.

## Captured meaning and wire

The stream starts with `PCSCAP01` and a fixed 128-byte big-endian header. It carries
exact stream length/record count, captured layer world extent, the selection-domain
intersection with that extent, original backing offset, binary default and
inversion. The layer extent and clip remain distinct. An empty intersection is an
active empty selection, never permission to edit an unrestricted target. Inactive
selection is a separate caller precondition and is not encoded.

Each 32-byte record names one backing tile in strict `(ty, tx)` order and declares
its derived clipped dimensions. Solid records contain raw 0 or 255. Dense records
pack clipped row-major binary samples, least-significant bit first, with only the
final byte padded. Missing records mean the raw default; inversion applies after
raw decoding. Coverage outside the clip is zero. Source Solid/Dense representation
is retained without a uniform-detection pass. Unknown tags, overflow, duplicate or
unordered records, wrong dimensions/counts/lengths, nonzero reserved/padding bytes
and trailing bytes refuse adoption.

These fields let the caller check the captured layer/selection geometry. They do
not authenticate a document, owner, source digest or edit intent. A future service
must bind the stream to that immutable action capture before publication; merely
decoding it supplies no editing authority.

## Ownership and continuations

`BinaryMask::snapshot_quote` computes bounds without scanning payloads. Its cursor
borrows the immutable mask and retains fixed traversal state, callbacks and an
account on the supplied `BudgetScope`. It writes bounded caller-owned chunks;
the caller separately reserves output capacity, reconciles actual allocation and
retains that charge through transfer/drop. Chunks stay private until
`finish_header` succeeds and the caller installs the final header.

`SnapshotDecoder` borrows the complete immutable stream. A resumable prefix pass
validates records and quotes retained storage before allocation. Construction
reserves the backing peak on the exact supplied scope, including map-growth
old/new overlap and full 256 by 256 Dense destination storage even for a tiny
clipped payload. Dense accounts take portions of that same declaration. Finish
moves the map and Dense leases into existing immutable BinaryMask storage; it does
not escape a reserved scope through `budget().scope()` or create a new pool.
Uniform zero-record fields normalize the irrelevant backing offset. Failure or
cancellation poisons partial output; it cannot be resumed or published.

## Work and bounds

Constructor callbacks and cumulative remaining work persist across steps. Small
quantums yield Pending without resetting either allowance. The quoted indivisible
headroom must fit before a map lookup/insertion or Dense initialization; these
library calls cannot be preempted by callbacks. This is a named source-action
policy, not a bound on machine instructions or elapsed time.

For grid cells `Q`, maximum records `R = min(Q, stored tiles)` and clipped area `P`,
the sample upper is `S = min(P, 65,536 × R)`. Distinct translated tiles are disjoint,
so each contributes at most 65,536 samples and their sum is at most `P`. This
constant-time bound avoids charging a sparse 1080p field as a dense canvas, without
raising caps or scanning to count Dense records. Record-stream bytes are bounded
by `E = 32R + floor((S + 7R) / 8)` and cursor work by
`1024 + 768Q + 32S + 8E`. Empty clip or absent storage takes the uniform fast path.
Decoder work separately prices validated records, Dense initialization and actual
Dense samples. Fresh-map byte/headroom bounds rely on the pinned insertion-only
construction policy, not capacity equality as proof of arbitrary-map provenance.

## Qualification and remaining work

The local gate passed 112 Raster native tests, including 13 Snapshot1 groups:
six independent literal streams, empty/offset/inversion semantics, malformed
records, exact reserved-scope denial despite global headroom, cancellation and
poisoning, work/peak/retained ownership, sparse-1080p quote causality, zero quantum
and repeated exact-headroom progress. Raster all-target strict checks, actual
Main/codec/render Worker Wasm strict compilation, fixture generation checks,
formatting and source-length checks passed. Wasm compilation is not browser runtime
execution, and these checks establish no latency, allocator/RSS or full-edit claim.

Evidence: `docs/validation/selection-snapshot1-integration/attempt-2/final.json` and
its raw logs; the first attempt remains preserved. Implementation and independent
fixtures live under `crates/foundation/paintcraft-raster/src/selection/mask/snapshot`
and `crates/foundation/paintcraft-raster/tests/selection-snapshot-fixtures`.
The asynchronous editing service, bounded Main adoption/finalization/history path,
real browser qualification and UI activation remain separate work.
