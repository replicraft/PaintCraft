# Original source metadata

The document can retain one immutable attachment describing its original source.
PNG new-document imports now create the attachment when iCCP or pHYs is present;
opening PCND2 restores previously stored attachments. Other foreign importers,
palette/index retention, complete EXIF preservation, per-layer provenance and
color-managed conversion remain separate work.

The attachment preserves exact original ICC bytes, original signed resolution
rationals and unit declarations, source file digest and extent, optional selected
page, original sample model, and original/materialized orientation geometry.
An ICC header is checked for container, signature, tag-table and payload bounds.
Its original class, version, color-space and rendering-intent declarations can
be inspected. Curves/LUTs and suitability for a color transform are not qualified.

Source provenance is distinct from working color. The working interpretation
remains unmanaged RGBA8. In particular, retaining an original CMYK or gray ICC
profile does not assign that profile to converted RGB samples. A profile's bytes
and SHA256 are frozen; no installed profile path participates in document identity.
Default-v1 rendering/compositing retains its existing RGB8 policy; the original
ICC is not activated.

Resolution values retain their original numerator and denominator without float
conversion or rational reduction. Nonpositive BMP pixels-per-metre declarations
remain signed provenance and do not become unsigned physical DPI. Unknown raw
unit codes can be retained separately from their interpreted unit.

Materialized orientation requires an actual original tag in the range 1–8 and
the corresponding original/output geometry. The attachment's output dimensions
describe the original conversion, so later crop, resize and rotation do not
rewrite those facts or make the edited document unsavable.

## Identity and lifetime

`paintcraft_document::source_metadata::SourceMetadata::capture` validates facts
and reserves a caller-owned `SharedBudget` lease before allocating retained ICC
storage. The private immutable allocation is shared by document snapshots,
history and restored cold entries. Its content digest includes canonical facts,
policy and exact profile bytes, independent of OS paths or memory addresses.

`Document::with_source_metadata_checked` creates a private semantic revision with
a deterministic new state ID; it preserves document identity. Semantic equality
includes the attachment. Unchanged attachments keep the same state. These APIs
do not publish an editor transaction or acknowledge a conversion/export warning.

History counts shared attachment storage once across hot/cold entries. Exact ICC
bytes remain resident while raster tile history is compressed or spilled, and
the lease ends when the last snapshot releases it. Reported bytes are conservative
logical retention weights, not allocator/RSS interception or hard memory limits.

## PCND persistence

The canonical wire layout is specified in
[PCND format, version 2](PCND_FORMAT.md#version-2-source-attachment).

Documents without an attachment still write the existing byte-compatible PCND1.
Attached documents write PCND2 with `paintcraft.document/v2` and the required
`source-metadata.v1` feature. Older readers must refuse the newer version.
The attachment is a canonical 192-byte header followed by exact ICC bytes, before
the existing layer-count/records. Its own SHA256 and the whole-file SHA256 bind
facts and payload. Parsed ICC header declarations are rederived from the original
bytes when reopening.

Attachments share the existing 8 MiB aggregate semantic-metadata ceiling with
document prelude, title and layer metadata. Readers deduct already consumed
prelude/title bytes before admitting/copying the profile, and accept a lower
caller-owned `pcnd::MetadataAdmission` limit/budget. Failure during later layer
admission releases the attachment lease.

Legacy PCND read wrappers create a per-read bounded realm. Application PCND loads and
PNG imports instead pass their host's shared resource realm. Browser pending
loads, parked interfaces and retained snapshots share one 64 MiB logical metadata
allowance; import previews use that allowance too. This is accounting for these
allocations, rather than a total process or GPU memory cap.

PNG retention stores the exact decompressed iCCP bytes and exact pHYs values and
unit. It does not preserve the original compressed chunk, profile keyword, full
EXIF, palette indices, text chunks or unrelated color declarations. Notices
distinguish retained-but-inactive ICC/resolution from actual omissions. Neither
retaining a profile nor accepting its notice activates color management.

The retained conversion has its own versioned identity binding source bytes,
geometry, RGBA samples and the attachment digest. The original source SHA256
remains the file guard. Layer insertion and paste retain their separate omission
policy; a second picture's profile is not assigned to the whole document.
Warned new-document conversions become unsaved copies. Before adoption, a PCND
preflight checks the current snapshot against the actual aggregate metadata
limit, including the exact boundary. Future layer/title growth must still obey
ordinary save admission.

Foreign document export defaults to refusing attached metadata. Explicit privacy
omission is captured against one frozen document ID, state ID, attachment digest,
format and operation. A changed snapshot or target invalidates that witness.
Adding an automatic known-loss acknowledgment or removing attachments from an
export snapshot does not establish consent. An omission receipt appears only
when an attachment was actually omitted. Other format losses retain their own
consent requirements. PCND Save preserves the attachment; requesting omission
for PCND or a preserving save is refused even for a document without metadata.

Independent Python-authored PCND1/2 fixtures qualify exact profile/fact retention
and raw output bytes. Native tests additionally cover signed density, identity,
checkpoint cancellation, aggregate refusal before profile admission, later-error
lease release, real canvas edits and Undo/Redo, and actual filesystem tile
spilling/cold restoration. These tests do not establish color-transform accuracy,
original-application interoperability or color-managed conversion. PNG samples
additionally have 27 independently authored and FFmpeg-checked fixtures covering
sample models, depth, transparency, Adam7 and hidden RGB. Native, CLI, MCP and
browser host tests exercise import/adoption, project round trips, source guards,
consent, cancellation and resource release. Production Chrome additionally
passes one real PNG retention/save/reopen/privacy flow and representative DDS/MSP
Worker copies with 12 real request-tamper/admission refusals. These preserve exact
project bytes, title and dirty state; output pixels have independent decoders.
Other browser engines, broader source variants and cross-platform qualification
remain separate gates.

Studio's native ICO, CUR and ICNS copies capture one private document before
preparation. Retained source metadata requires its own explicit omission choice;
revocation retires the prepared preview and original policy. Destination review
and publication validate that original policy against the same document and
format. Other actual losses still require their separate acknowledgment. Private
text is rendered once into the export copy while the editable document, history
and source attachment remain intact.

Native controls pass in all seven supported languages, with literal decoded pixel
checks for ICO/CUR/ICNS and exact CUR hotspots. Additional controls cover revoked
choices, changed settings, later text edits, cancellation, source aliases and
changed destinations. The shared icon preview requires a complete visible image
in a prior frame; a partly clipped image does not establish review. These are
local host/UI and filesystem checks, not physical GUI or six-target qualification.

Production Chrome additionally passes real retained-PNG import, PCND2 save/reopen
and ICO/CUR/ICNS downloads with the same omission controls. Independent container
checks confirm omitted ICC/resolution chunks, exact 32-pixel entry samples and
CUR hotspots; resized ICO/CUR entries receive structural checks only. Unchecked
and revoked choices block output, and Cancel/export retain byte-identical PCND
downloads, title and clean state. The first ICO attempt exposed incorrect
forwarding of the metadata receipt as an ordinary loss code; that failure is
retained separately, and the corrected flow has an actual encoding regression.
These checks cover local Chrome, not other browsers or physical devices.

Native recovery discovery, checkpoint validation and claims now use the caller's
shared attachment allowance; legacy recovery wrappers use the process default.
Quota failures and cancellation preserve recovery offers and files. A future
PCND version is retained without falling back to an older snapshot of that
session; genuine corruption retains the existing fallback behavior. Cleanup is
deferred until validation and the final cancellation check succeed. Discovery
failure does not prevent protecting the current document with its own autosave.

Recovery also preserves pre-existing private text and gestures until claim and
capacity admission succeed. Later edits invalidate asynchronous adoption. A
successful Studio recovery finalizes the old document's text once, retaining
Undo, before parking it in its tab. The two causal denial regressions and 40
local desktop controls, 42 recovery-format controls and strict lint checks pass.
These checks do not qualify power-loss behavior on every filesystem or target.
