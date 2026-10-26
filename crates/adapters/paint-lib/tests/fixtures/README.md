# Observed PAINT sample and independent pixel oracle

`mspaintlib-sample.paint` is an unchanged copy of
[`samples/sample.paint`](https://github.com/setiri/mspaintlib/blob/e35312bec8e8a09c6a209d2dc0110b885e48c945/samples/sample.paint)
from Setiri LLC's MIT-licensed `mspaintlib`, pinned at commit
`e35312bec8e8a09c6a209d2dc0110b885e48c945`. The accompanying license is
`LICENSE-mspaintlib.txt`. SHA-256:
`a3e947fee374cca80e0e648152667109ac61b57808c9673eb806f52865f141f6`.

This is one independent upstream sample, not a PaintCraft-written round trip.
Its originating Windows/Paint build and UI-layer metadata are not recorded here.
It therefore qualifies the observed pixel/item subset, not general Paint fidelity
or a preserving writer.

`sample-oracle.json` records a separately executed Python `struct`/`zlib` decode:
read the five `iloc` entries directly at byte 226; each entry is `>HHHHII` in
this sample. Inflate the four file-relative method-0 extents with
`zlib.decompressobj(-15)`, requiring EOF and no unused/unconsumed bytes. Each
produces exactly `333 * 271 * 4` bytes. The declared `cmpd`/`uncC` property order
is BGRA, so swap byte positions 0 and 2 per pixel, then SHA-256 the entire output.
All four complete RGBA hashes are asserted by the Rust test. The 16-bit overlay
fill is `[0x6b00, 0x6b00, 0x6b00, 0xffff]` and is retained without narrowing.

The decoder is an independent Rust implementation. The pinned upstream
[`PAINT-FORMAT-SPEC.md`](https://github.com/setiri/mspaintlib/blob/e35312bec8e8a09c6a209d2dc0110b885e48c945/PAINT-FORMAT-SPEC.md)
and MIT source under `src/MsPaintFile` supplied reviewed format observations,
including the corrected component-index swizzle. PaintCraft's Rust decoder does not execute .NET/CLR code or require that runtime;
the separate qualification harness below deliberately runs the pinned C# reader.

## Internal fresh observed-profile writer, 2026-10-26

`writer/{narrow,wide,extreme}.paint` are original PaintCraft-written specimens,
not projects saved by Windows Paint. Each has two distinct original arithmetic
RGBA buffers (3×2 and 2×3), alpha 0/1/127/128/254/255, hidden RGB, distinct
literal item names and signed layer placement. Narrow uses 16-bit overlay fields;
wide uses a 65536×1 canvas and offsets beyond Int16; extreme retains Int32 minimum
and maximum positions. The canvas fill `[0x1234, 0xabcd, 0x7f00, 0xffff]` exercises
all 16 raw bits. These owned bytes, test source and harnesses use the repository
license; upstream source is downloaded privately, not vendored.

Ordinary CI always re-encodes all three owned source models and asserts exact
identity with these independently verified bytes; only optional file writing is
controlled by the output environment variable. Locked-codec output drift requires
review and renewed qualification, not an automatic fixture rewrite.

The writer independently constructs fresh boxes, deterministic IDs, associations
and non-overlapping extents. It does not patch the upstream sample/template.
Length/offset fixups affect only fields the new serializer just emitted. It
retains each layer's own dimensions and all pixels even outside the canvas;
there is no clipping, padding, resizing, premultiplication or flattening. It
refuses hidden layers, per-layer opacity other than 255 and nonempty unsupported
metadata. `item_name` means the literal `infe` field, not a promise that Paint's
UI will use it as a layer name. Visibility/opacity/name extensions remain
unqualified. No format façade or public/preserving writer is enabled by this
backend increment.

### Pixel/color/alpha syntax

The profile is grounded in the pinned original fixture and authored format
observations, not inferred solely from returned RGBA bytes. `cmpC` declares
raw `defl` for the whole image; `colr/nclx` declares primaries 1, transfer 13,
matrix 0 and full range. `cmpd` identifies red/green/blue/alpha components as
4/5/6/7. `uncC` version 0 `gene` uses unsigned 8-bit components, explicit
component-index order 2/1/0/3, pixel interleave, no subsampling and no row/tile
padding; `pixi` declares four 8-bit components. BGRA is therefore derived from
these fields, rather than guessed from channel names or a file extension.
The primary `iovl` retains its exact RGBA16 fill, canvas dimensions and signed
offsets; ordered `dimg` references give the bottom-to-top inputs. Per-layer
`ispe` associations refer to their actual dimensions, including non-canvas sizes.

Only `dimg` relationships are emitted; there is no `prem` association or unknown
alpha transformation. As an independently audited HEIF signaling reference,
[libheif v1.20.2 image_item.h](https://github.com/strukturag/libheif/blob/v1.20.2/libheif/image-items/image_item.h)
initializes premultiplied alpha to false, and
[context.cc](https://github.com/strukturag/libheif/blob/v1.20.2/libheif/context.cc)
sets it true for explicit `prem` references. Their source SHA-256 values are
`6d1fdb237e45b9ff013829177d2bf5e83337f4b2ac1f7126ed6b5a6f3335d8a5`
and `ee582631e6369f3013d9e5abef4afd27ba809fee4e9f4a4116a5f1fbfde83abe`.
This is static signaling evidence, not a libheif decode test or Windows Paint
qualification. `uncC` packing flags are not presented as a premultiplication flag.
The writer takes an explicit straight encoded-sRGB RGBA8 contract; unlabelled,
high-depth or foreign color data needs a separately qualified conversion first.

The emitted `ftyp` mirrors the observed `mif1/gcmi/isoa/miaf` list. Those brands
are source-profile observations, not proof of generic MIAF conformance or a
pinned Windows Paint build. The original sample's originating Windows/Paint
version is still not recorded.

### Independent positive evidence and retained limitations

`verify_writer.py` uses Python 3.14.0, standard `struct`/`zlib`, and an executable
built from unchanged pinned mspaintlib sources. Its result is
`writer/independent-oracle.json`. The Python implementation checks complete box
sizes/scopes, IDs/types, every item/property association and essential bit,
method-0/method-1 extents, alias/overlap freedom, complete raw-DEFLATE EOF/length,
actual component mapping, every original RGBA byte, UTF-8 names, canvas fill16,
canvas dimensions and signed layer positions. It also verifies that neither the
new specimens nor the original sample change during reading.

The original C# reader was independently compiled with installed SDK 10.0.401
for net10.0, because that runtime was available. The upstream library itself
specifies net8.0; its source files were not edited, but this runtime retargeting
is part of the qualification boundary. Actual runtime was .NET 10.0.12, Arm64.
There are no NuGet package references. The project clears package sources and
commands put CLI state, package cache, intermediate and build output in a
private task directory. This adds no .NET runtime or SDK dependency to PaintCraft.
The original four-layer sample first passed as a control with all four existing
independent pixel hashes and exact raw overlay fill. All three new files then
passed exact C# pixel/order/extent/offset checks, including differing layer sizes
and wide/extreme positions. Original `IovlItem.Parse` retains every fill16 value.

Two original-reference limitations are deliberately preserved in the report:

- High-level `PaintDocument.CanvasFill` narrows each raw fill field with `>> 8`.
  Exact raw fill16 is verified separately; high-level fill fidelity is not claimed.
- `BoxReader.ReadNullTerminatedAscii` casts each UTF-8 byte to a character, so the
  Japanese/emoji literal item name becomes mojibake in high-level C# `Name`.
  The ASCII control remains exact. Python verifies the correct on-disk UTF-8
  name separately. Neither original source, authored names nor wire bytes were
  changed to force name parity.

These results qualify bounded structure and byte conversion for this observed
profile. They do not demonstrate Windows Paint open/edit/resave, general modern
Paint metadata, UI layer-name behavior or whole-document reference roundtrips.
The public native-writing gate remains closed pending the original application.

### Bounds and reproducible commands

The initial typed profile caps layers at 64, each side at 65536, canvas pixels
and aggregate layer pixels separately at 4,194,304, each UTF-8 NUL-free item name
at 1024 bytes, metadata declaration at 1 MiB and private output at 64 MiB. Caller
limits can tighten them. One raw-DEFLATE stream per layer receives bounded
64 KiB BGRA chunks. Source bytes, output capacity, metadata/scratch and a 1 MiB
logical codec allowance are admitted before allocation. The fixed miniz state
and conservative output-growth audit are the same selected `flate2` 1.1.10 /
`miniz_oxide` 0.9.1 path documented in the
[PDN fixture audit](../../../pdn-lib/tests/fixtures/README.md).
The private append ceiling independently prevents growth beyond the reservation.
These declarations do not establish an allocator/RSS cap or catch OS allocation
aborts. Cancellation checks surround bounded codec calls, metadata loops and
staged-sink chunks; there is no internal codec cancellation callback or hard
latency claim. Tests include 64 layers, incompressible multi-chunk growth, all
observed encoding cancellation checkpoints, unsupported semantics and failed
private sinks. The caller still owns source/owner guards and atomic publication.

Reproduce with an already installed .NET 10 SDK. Output and upstream code stay
private; no global SDK/package/configuration changes are required:

```sh
task_dir="$(mktemp -d)"
mkdir "$task_dir/vectors"
PAINTCRAFT_PAINT_WRITER_OUTPUT="$task_dir/vectors" \
    cargo test -p paintcraft-paint --test write \
    export_owned_writer_vectors_only_when_requested --offline --locked
curl -fsSL https://codeload.github.com/setiri/mspaintlib/tar.gz/e35312bec8e8a09c6a209d2dc0110b885e48c945 \
    -o "$task_dir/upstream.tar.gz"
python3 - "$task_dir/upstream.tar.gz" <<'PY'
import hashlib, pathlib, sys
assert hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest() == "6661e83b496d8f7606bf4b96a7e7a51e405a09d82924ba2e2270518b6b7dc503"
PY
tar -xzf "$task_dir/upstream.tar.gz" -C "$task_dir"
export DOTNET_CLI_HOME="$task_dir/cli-home"
export DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_SKIP_FIRST_TIME_EXPERIENCE=1 DOTNET_NOLOGO=1
export NUGET_PACKAGES="$task_dir/packages"
dotnet build crates/adapters/paint-lib/tests/fixtures/writer-reference/WriterOracle.csproj \
    "-p:UpstreamRoot=$task_dir/mspaintlib-e35312bec8e8a09c6a209d2dc0110b885e48c945" \
    "-p:BaseIntermediateOutputPath=$task_dir/obj/" \
    "-p:MSBuildProjectExtensionsPath=$task_dir/obj/" \
    --configfile crates/adapters/paint-lib/tests/fixtures/writer-reference/NuGet.Config \
    --output "$task_dir/reference" --nologo
python3 crates/adapters/paint-lib/tests/fixtures/verify_writer.py \
    "$task_dir/vectors" "$task_dir/reference/WriterOracle.dll"
```

Passing `crates/adapters/paint-lib/tests/fixtures/writer` as the first verifier
argument checks the immutable checked-in outputs. The verifier only reads inputs
and prints JSON; no upstream source, tool binary or generated build cache is
checked in.
