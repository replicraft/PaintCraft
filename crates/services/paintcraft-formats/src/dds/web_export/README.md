# Provisional browser advanced DDS worker boundary

This is a private preparation seam, not browser export admission. Native
`PCDEC002` and the existing browser JXL decoder protocol remain unchanged. The
legacy default browser DDS route is unchanged. UI/JS adoption and production
browser qualification are still pending.

The explicit provisional advanced profile admits a canvas of at most 1,048,576
pixels, each dimension at most 65,536, at most eight flat layers, at most 2,097,152
logical layer pixels, at most 12MiB of resident padded tiles and 64KiB of source
metadata. All data is kept or the job is refused; no layer or mip is truncated.
These limits are not a claim of the largest possible browser profile.

`SourcePlan::capture` checks immutable document data before snapshot allocation.
The future caller must obtain the JS service reservation before `snapshot`.
Snapshot serialization uses existing **raw** `pcnd::write_to` with a fixed 16MiB
bounded sink; no compact/tile compression, rendering or DDS encoding runs there.
The source SHA binds those complete bytes, including every layer and metadata.

The worker uses the existing PCND parser with a caller layer bound. Root-count
admission precedes per-layer names and record/raster allocation. Its staged tile
bound charges whole padded tiles before inflation (12MiB / four bytes per pixel).
Titles are limited to 4,096 characters/16KiB; accepted names to 1,024 characters,
with at most eight names. Even a last invalid string can allocate at most 16KiB
before refusal. With the tile inventory capped at 48 256×256 tiles, names,
records, coordinates and parser scratch fit comfortably within the conservative
8MiB parser-metadata allowance. The old PCND wrappers retain their 100,000-layer
policy; this browser limit does not narrow them.

After source binding/profile checks, the worker calls the shared document DDS
facade once. Its composite drops before actual top-mip decoding reuses that RGBA
allowance. A nearest-sampled preview is at most 192×192. Preview samples remain
in the output's declared transfer: the future main UI must apply its existing
sRGB display encoding to Linear8 thumbnail RGB, preserving alpha. The response
contains the entire DDS including generated mips; preview omission is not an
export loss. Only the facade's actual losses are reported.

The declared browser job reservation is 416MiB of logical payload allowances:
256MiB for the entire Worker linear-memory ceiling, five 16MiB source/capture/
request/digest-copy allowances, five `(8MiB + 4KiB)` encoded/result/Blob copy
allowances, 12MiB of retained source tiles, plus bounded preview copies and
metadata. This leaves more than 27MiB of declaration margin. The worker's own
pre-render logical payload quote must fit 96MiB, including source bytes, source
tiles, facade workspace, parser allowance and response/preview scratch. Neither
number is aggregate browser RSS, a GPU bound, or an allocator/stack guarantee.
The actual Worker module must still enforce its existing 256MiB memory maximum.
Worst selected BC High/filter cases, actual allocation-copy measurements and
real 1MP Wasm/browser boundaries remain required before profile promotion.

## Version-one byte layout

Integers are big-endian. Reserved bytes and inactive settings must be canonical.
The request is a 128-byte header followed by the declared PCND snapshot:

| Bytes | Field |
|---|---|
| 0..8 | `PCDDSQ01` |
| 8..10 | protocol 1 |
| 10..12 | zero |
| 12..16 | snapshot byte length |
| 16..24 | width, height |
| 24..65 | owner: session UUID, state UUID, u64 epoch, Studio=3 |
| 65..81 | nonce UUID |
| 81..113 | source SHA-256 |
| 113..121 | canonical eight-byte settings |
| 121 | source flags: flatten, hidden RGB |
| 122..128 | zero |

Settings contain encoding (RGBA8/BC1/BC2/BC3), transfer (sRGB/Linear8), alpha
(straight except opaque BC1), actual level count, filter (Nearest/BoxStored),
effort (Normal/High), and two zero bytes. RGBA effort canonicalizes to Normal;
one-level filtering to Nearest. Decoding refuses noncanonical wire values.

The response is a 288-byte header followed by DDS bytes, thumbnail RGBA and
optional refusal text, in that order:

| Bytes | Field |
|---|---|
| 0..8 | `PCDDSR01` |
| 8..10 | protocol 1 |
| 10..12 | status, zero |
| 12..24 | captured width, height, snapshot length |
| 24..28 | encoded length |
| 28..32 | thumbnail width, height (u16 each) |
| 32..73 | exact captured owner |
| 73..89 | nonce |
| 89..121 | source SHA-256 |
| 121..129 | settings |
| 129..193 | encoded SHA-256, thumbnail SHA-256 |
| 193 | five known loss bits |
| 194..198 | quantized RGB sample count |
| 198..263 | version byte length, 64-byte zero-padded ASCII version |
| 263..265 | bounded UTF-8 refusal length |
| 265..266 | captured source flags |
| 266..288 | zero |

Successful replies must match source/owner/nonce/options/version, both result
digests, exact thumbnail geometry, actual DDS header/complete-chain inventory
and canonical loss/statistics constraints. The host does not independently
re-render to prove the worker's quantization count. Canonical completed refusals
carry no DDS, preview, loss or quantization data. Malformed/unknown completion
is not a release acknowledgement: future JS integration must retain the existing
fail-closed quarantine rules and must never treat `terminate()` as a join.
