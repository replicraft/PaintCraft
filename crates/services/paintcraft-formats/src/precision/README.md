# Explicit float/HDR conversion policy

This module implements policy and mathematical qualification only. It does not
decode EXR/HDR, enable a registry route, preserve floating-point editing or
authorize a foreign Save target. Decoder libraries/features remain unchanged.

## Source color requires evidence or a user assignment

The format adapter supplies captured source SHA-256, dimensions, color evidence
and alpha convention. The policy supports linear Rec.709/D65 and Radiance's
nominal CRT primaries with equal-energy white. Explicit metadata has distinct
provenance from the Radiance default. Unlabelled EXR has **no implicit Rec.709
default**: `Settings.source_assignment` must contain the user's choice. User
assignment is recorded in the binding and reported as an import limitation.

Current OpenEXR/ASWF guidance prefers `colorInteropID`, such as
`lin_rec709_scene`; absent/unrecognized metadata needs an explicit resolution
path. A later adapter must validate recognized IDs, chromaticities and conflicting
metadata before producing this descriptor. This policy does not authenticate
arbitrary descriptors supplied by a Rust caller.

Radiance nominal primaries are R(.64,.33), G(.29,.60), B(.15,.06), white(1/3,1/3).
They differ from sRGB's green primary and D65 white. Conversion uses source
RGB→XYZ, explicit Bradford equal-energy→D65 adaptation, then XYZ→linear Rec.709.
This is a declared relative-color policy, not preservation of every authored
viewing transform or absolute luminance convention.

## Explicit display choices

Exposure is limited to ±16 EV, stored in thousandths of a stop. The receipt and
binding digest name the curve. **Clip is the default**: linear white 1 maps to
SDR white 255. Positive highlights above 1 are clipped and counted. Optional
**Reinhard per channel** applies `v/(1+v)`; linear white 1 consequently maps to
188 after sRGB encoding. It is never substituted silently for Clip. Both paths
apply piecewise sRGB encoding and nearest 8-bit rounding.

Negative source values are rejected by default. Explicit clipping permits them
through the color transform and clips negative destination values to black,
with source/changed-sample counts and a loss code. Nonfinite color/alpha and
finite alpha outside [0,1] always refuse the entire conversion. High dynamic
range and floating precision are never described as retained in RGBA8.

## Alpha and additive zero-opacity color

OpenEXR convention is premultiplied color; straight interpretation is a separate
explicit override. For retained transparency, positive-alpha premultiplied
samples are unassociated before curve conversion. Straight samples retain
hidden RGB at alpha zero. Alpha is quantized linearly, never sRGB encoded. The
quantization loss includes positive sub-byte alpha becoming zero.

Premultiplied alpha zero with nonzero RGB represents additive color. Retaining
that as ordinary straight zero-alpha RGBA would discard its visible contribution,
so Preserve refuses it. Explicit Flatten instead combines premultiplied color
with the chosen opaque backdrop and retains that contribution. Backdrop sRGB8
is decoded to destination-linear Rec.709, composited **before exposure and the
named curve**, and the result becomes opaque. Flattening is reported as a loss.
This conversion cannot preserve HDR compositing against arbitrary future
backgrounds; the eventual preview must show the chosen SDR result.

## Ownership and qualification

`Binding` has private immutable fields and hashes source identity, evidence,
geometry and every setting. Later preview/consent must capture this exact
digest alongside the existing document owner/source guards. Source, space,
alpha interpretation, backdrop, exposure or curve changes require new preparation
and consent. The policy itself performs no file IO or document publication.

The caller admits source samples and decoder scratch separately. Conversion
checks shape/pixel limits, reserves returned RGBA capacity before publication,
checks cancellation per 1024 pixels and returns no partial storage. Returned
bytes retain their logical budget account until dropped. This does not establish
physical RSS, allocator-overhead or decoder containment bounds.

`tests/precision-fixtures/reference.py` generates 35 owned vectors with
100-digit Decimal arithmetic; a separate NumPy solve checks the color matrix.
The fixtures cover color/transfer boundaries, both curves, exposure bounds,
ambiguous color assignment, premultiplied/straight/additive alpha, negatives and
nonfinite/out-of-range refusals. Tests also verify binding changes, consent codes,
retained admission and cancellation after a partial output block.

## Decoder candidates remain separate

Downloaded candidates were `exr` 1.74.2 (BSD-3-Clause, default Rayon can be disabled)
and `zune-hdr` 0.5.2 (MIT/Apache-2.0/Zlib). No workspace dependency was added.
Zune HDR failed an owned 1×1 probe: exponent 160/red mantissa 128 decoded as 0.5
instead of 2,147,483,648 under its lower-edge RGBE convention. Exponent 96 also
wrapped by 32 exponent bits. The inspected official development snapshot still
contains that mask. Existing `image` 0.25.10 HDR matched all ten exponent vectors,
including subnormals and exponent 255, in a temporary qualification build.

Reference Radiance midpoint decoding differs by half a mantissa bin from the
lower-edge convention used by both Rust candidates. That distinction is recorded
separately and cannot explain the exponent-wrap failure. Raw inputs, sources,
versions, package hashes and outputs are retained under
`docs/validation/hdr-policy-2026-10-26/`. Neither candidate is admitted for hostile
files or frontend use by this slice. Bounded header/packet admission, metadata,
exposure/orientation, decoder confinement and actual consumers remain required.

Primary references: [Radiance color definitions](https://github.com/LBNL-ETA/Radiance/blob/master/src/common/color.h),
[Radiance picture representation](https://radsite.lbl.gov/radiance/refer/Notes/picture_format.html),
[OpenEXR color/alpha conventions](https://openexr.com/en/latest/TechnicalIntroduction.html),
[ASWF OpenEXR color identification](https://github.com/AcademySoftwareFoundation/ColorInterop/blob/main/Recommendations/04_OpenEXRFiles/OpenEXRFiles.md),
and [ICC.1:2022 Annex E, Bradford adaptation](https://www.color.org/specification/ICC.1-2022-05.pdf).
