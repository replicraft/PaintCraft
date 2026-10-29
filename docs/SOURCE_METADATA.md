# Original source metadata

The document can retain one immutable attachment describing its original source.
This is a storage foundation. Foreign raster importers do not yet create the
attachment automatically; opening PCND2 restores previously stored attachments.
Palette/index retention, complete EXIF preservation, per-layer provenance and
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

Legacy read wrappers create a per-read bounded realm. This does not qualify
aggregate memory across application tabs, documents, Web imports or MCP sessions.
Actual importer adoption must pass the host's shared resource realm and preserve
the raw source identity/consent guards.

Foreign document export paths refuse attached metadata until preservation or an
explicit captured omission/privacy policy is qualified. Adding an automatic
known-loss acknowledgment or removing attachments from an export snapshot does
not establish consent. PCND Save preserves the attachment.

Independent Python-authored PCND1/2 fixtures qualify exact profile/fact retention
and raw output bytes. Native tests additionally cover signed density, identity,
checkpoint cancellation, aggregate refusal before profile admission, later-error
lease release, real canvas edits and Undo/Redo, and actual filesystem tile
spilling/cold restoration. These tests do not establish color-transform accuracy,
original-application interoperability or new importer metadata preservation.
