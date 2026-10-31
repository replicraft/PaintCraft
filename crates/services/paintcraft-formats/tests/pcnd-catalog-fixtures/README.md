# Planned PCND6 catalog specimens

These are independent stdlib byte fixtures for the **planned**
`docs/architecture/PCND_FILTER_CATALOG.md` grammar. No PCND6 reader, writer,
Descriptor, catalog negotiation, evaluator or UI support is claimed to exist.
`accepted_planned` and `code_planned` are requirements for future implementation,
not results from running product code. Historical PCND1–5 and numerical fixtures
are read-only and unchanged.

## Reproduce

```sh
PYTHONDONTWRITEBYTECODE=1 python3 crates/services/paintcraft-formats/tests/pcnd-catalog-fixtures/generate.py
```

The default command verifies `oracle.json` byte for byte. Use `--write` only
following an intentional contract review. The script disables Python bytecode
writes before importing the frozen numerical oracle. It imports no product
package, calls no codec binary or FFI, and does not use a PaintCraft serializer.
Every file is small and embedded as hex; no binary fixture directory is generated.

## Wire facts

The generator writes magic 6, a length-delimited manifest immediately after the
magic, UUID/scalar/source/layer fields, catalog-indexed filter records, the
existing optional A8 mask framing, one tiny raw RGBA tile and an outer SHA-256.
Every manifest declaration is sorted by profile ID and used by a stored instance
in positive specimens. IDs and codecs are strict ASCII versioned identifiers;
profiles use scoped `params-v1`. Levels parameters are exactly six bytes:
input black u8, input white u8, gamma hundredths u16BE, output black u8 and
output white u8. Brightness/Contrast is two i16BE values in this new transport;
that does not change old containers' ZigZag parameter encoding.

The stored layer is 2×1, default transparent black, with raw samples
`[64,16,128,0,16,64,255,255]`, including hidden RGB. Document/state/layer and
instance UUID bytes are explicit. Positive rows also include the planned v3
semantic stack digest, which excludes manifest indexes and transport codec IDs.
Existing canonical A8 digest framing is independently serialized for mask input.

## Positive cases and provenance

Eight positives cover:

- Default Levels and disabled identity Levels, still requiring catalog framing.
- Gamma 0.10; gamma 9.99 with inverted output; equal output; and a single input step.
- An ordered Box 1→Levels 2.00 stack with a saved half/full A8 mask and original
  source attachment. Its negative local mask domain retains latent off-layer data.
- Every initial numerical profile in one manifest, including signed
  Brightness/Contrast, point identities and radius-zero Box.

The attachment is copied byte-for-byte from the existing independent
`pcnd-metadata-fixtures/rgb-profile.pcnd` specimen. The manifest pins both parent
and extracted attachment hashes. These original-source facts remain provenance,
not a profile activation, imported-image reproduction or authenticity claim.

`evaluated_layer_rgba` describes appearance **before layer composition**. It does
not silently zero hidden RGB or claim whole-document compositor behavior.
Levels expectations reuse the frozen independent Python numerical oracle, whose
source path and SHA-256 are pinned; this is reuse, not a newly independent Levels
oracle. The tiny Box expectation directly sums the full clamped square with
integer rational rounding, independent of product sliding passes. The remaining
point arithmetic follows their existing exact integer policies. Masks are applied
once to the incoming stage prefix. The coupled layer plane is
`[40,96,255,85,64,128,255,170]`; raw source bytes stay unchanged.

## Negative cases and error categories

Thirty-seven negative controls cover unknown required profiles/codecs (including
disabled instances), duplicate/unused/unsorted declarations, invalid indexes,
zero/excess counts, nonminimal integers, prelude tails/caps, identifier caps,
ASCII/version syntax, malformed Levels/BC domains, exact parameter lengths,
parameter caps, invalid enabled/mask flags or domains, unknown disabled mask
codec, a legacy-only noncanonical v6, trailing body, future version and bad SHA.

All negative bodies have a repaired outer SHA except the intentional
`bad-outer-sha` case. SHA supplies integrity binding, not source authentication.
Thus early catalog controls cannot pass merely because the outer digest is bad.
Over-cap length/count controls use short bodies with excessive declarations;
the format-defined ceiling must be checked before allocating or consuming the
claimed extent. No 65 KiB/8 MiB padding artifacts are needed for these early checks.

- Unknown well-formed required algorithms/codecs: `capability.unsupported`.
- Format-defined count/identifier/prelude/parameter ceilings, malformed framing,
  noncanonical bytes, invalid typed parameters or canonical inventory:
  `codec.failed`, even when a format ceiling resembles a quota.
- Caller-injected resource allowances: `resource.limit`; those need future
  runtime tests with an explicit caller realm and are not invented wire fixtures.

Assessment may report well-formed unknown requirements without granting adoption.
The future reader must resolve every declared requirement, even inactive ones,
before retaining payloads or inflating raster tiles. This corpus cannot itself
establish cancellation, resource-release, compact-writer, runtime or UI completion.
