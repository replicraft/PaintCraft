#!/usr/bin/env python3
"""Independent Source8 framing proposal. No product imports/math/adoption calls."""
from pathlib import Path
import copy
import hashlib
import json
import sys

HERE = Path(__file__).resolve().parent
ADMISSION = b"paintcraft.worker-admission/v8\0"
FACT = b"paintcraft.worker-capabilities/v8\0"
UNION = b"paintcraft.worker-capability-union/v8\0"
MASK = dict(feature_id="layer-masks.v1", resource_contract_id="layer-mask-final-alpha-rgba8-v1",
            resource_contract_revision=2, query_revision=1, alpha_revision=1,
            workload_class_id="base-work-v1", workload_class_revision=1)
BLEND = dict(feature_id="layer-blend-modes.v1", resource_contract_id="ordered-layer-blend-rgba8-v1",
             resource_contract_revision=1, arithmetic_revision=1, work_revision=1,
             context_setup_per_root="256", opaque_extra_work="32", general_work="256",
             workload_class_id="base-work-v1", workload_class_revision=1)
DOC, A, B, F, DUP = [bytes([n]) * 16 for n in [0x10, 0x11, 0x22, 0x33, 0x44]]
ZERO = bytes(16)


def n(value, width):
    if type(value) is not int or not 0 <= value < (1 << (8 * width)):
        raise ValueError("unsigned range")
    return value.to_bytes(width, "big")


def text(value):
    data = value.encode("ascii")
    if not 1 <= len(data) <= 96:
        raise ValueError("text extent")
    return n(len(data), 2) + data


def digest(value):
    return hashlib.sha256(value).digest()


def frame(value):
    return dict(hex=value.hex(), sha256=digest(value).hex(), bytes=len(value))


def mask_registration():
    return (text(MASK["feature_id"]) + text(MASK["resource_contract_id"])
            + n(MASK["resource_contract_revision"], 2)
            + n(MASK["query_revision"], 2) + n(MASK["alpha_revision"], 2)
            + text(MASK["workload_class_id"]) + n(MASK["workload_class_revision"], 2))


def blend_registration():
    return (text(BLEND["feature_id"]) + text(BLEND["resource_contract_id"])
            + n(1, 2) * 3 + n(256, 8) + n(32, 8) + n(256, 8)
            + text(BLEND["workload_class_id"]) + n(1, 2))


def admission(mode, origin, workspace=64 << 20, base=268435456, noise=None):
    if mode not in [0, 1] or origin not in [0, 1]:
        raise ValueError("mode/origin")
    if noise is None:
        noise = (1 << 35) if mode else 0
    if not 0 <= workspace <= 64 << 20 or not 0 <= base <= 268435456:
        raise ValueError("limits")
    if not 0 <= noise <= 1 << 35 or (mode == 0 and noise):
        raise ValueError("noise limit")
    return (ADMISSION + n(mode, 1) + n(origin, 1)
            + b"".join(n(v, 8) for v in [workspace, base, noise, base + noise])
            + n(1, 2) + n(64, 2) + n(8388608, 8) + n(134217728 if mode else 0, 8))


def blend_identity(roots):
    if len(roots) > 100000 or len({uid for uid, _ in roots}) != len(roots):
        raise ValueError("root bound/duplicate")
    if any(len(uid) != 16 or tag not in [0, 1, 2] for uid, tag in roots):
        raise ValueError("mode/UUID")
    if not any(tag != 0 for _, tag in roots):
        return None
    return b"paintcraft.layer-blends/v1\0" + DOC + n(len(roots), 4) + b"".join(
        uid + n(tag, 1) for uid, tag in roots)


def numerical(mode, masked):
    rows = []
    if mode:
        rows.append(("add-noise-rgb-v1", "params-v1", "direct-source-noise-rgba8-u512-g4096-v1"))
    if masked:
        rows.append(("invert-rgb-v1", "params-v1", "direct-rgba8-point-v1"))
    rows.sort()
    count = n(len(rows), 2)
    required = b"paintcraft.worker-required/v1\0" + count
    resolution = b"paintcraft.worker-resolution/v1\0" + n(1, 2) + count
    for profile, codec, resource in rows:
        pair = text(profile) + text(codec)
        required += pair
        resolution += pair + text(resource) + n(1, 4)
    # Existing numerical resource revision is u32, independently literal above.
    return len(rows), digest(required), digest(resolution), frame(required), frame(resolution)


def key(row):
    return bytes.fromhex(row["layer"]) + n(row["kind"], 1) + bytes.fromhex(row["filter"])


def ordered(rows):
    rows = sorted(rows, key=key)
    if len({key(r) for r in rows}) != len(rows):
        raise ValueError("duplicate coverage key")
    return rows


def row_tail(row):
    return (b"".join(v.to_bytes(8, "big", signed=True) for v in row["domain"])
            + bytes.fromhex(row["digest"]) + n(row["bands"], 8)
            + n(row["runs"], 8) + n(row["upper"], 8))


def bounds(rows, abi):
    rows = ordered(rows)
    return (b"paintcraft.worker-coverage-bounds/v1\0" + text(abi) + n(1, 2) + n(len(rows), 4)
            + b"".join(key(r) + n(int(r["enabled"]), 1) + row_tail(r) for r in rows))


def inventory(rows):
    rows = sorted(rows, key=lambda r: key(r[0]))
    if len({key(row) for row, _, _ in rows}) != len(rows):
        raise ValueError("duplicate current key")
    return (b"paintcraft.worker-mask-inventory/v1\0" + n(len(rows), 4)
            + b"".join(key(r) + n(int(enabled), 1) + n(int(r["enabled"]), 1)
                       + row_tail(r) + key(source) for r, enabled, source in rows))


def own(rows):
    rows = [r for r in ordered(rows) if r["kind"] == 1]
    raw = b"paintcraft.worker-layer-masks/v1\0" + n(len(rows), 4)
    raw += b"".join(bytes.fromhex(r["layer"]) + n(int(r["enabled"]), 1)
                    + bytes.fromhex(r["digest"]) for r in rows)
    fields = dict(stored=len(rows), enabled=sum(r["enabled"] for r in rows),
                  bands=sum(r["bands"] for r in rows), runs=sum(r["runs"] for r in rows),
                  decodeUpperBytes=sum(r["upper"] for r in rows), identitySha256=digest(raw).hex())
    return fields, raw


def prefix(count, required, resolution, grant, abi):
    return (n(8, 2) + n(4, 2) + n(count, 4) + required + resolution + digest(grant)
            + mask_registration() + blend_registration() + text(abi))


def capability(pre, own_fields, source_bounds, current, blend):
    return (FACT + pre + n(own_fields["stored"], 4) + n(own_fields["enabled"], 4)
            + b"".join(n(own_fields[x], 8) for x in ["bands", "runs", "decodeUpperBytes"])
            + bytes.fromhex(own_fields["identitySha256"]) + source_bounds + current + blend)


def case(name, mode=0, origin=0, own_enabled=None, filter_change=None, abi="wasm32-v1", reverse=False):
    roots = [(A, 0), (B, 2 if mode else 1)]
    if reverse:
        roots.reverse()
    semantic = blend_identity(roots)
    blend = digest(semantic)
    rows = []
    if own_enabled is not None:
        rows.append(dict(layer=B.hex(), kind=1, filter=ZERO.hex(), enabled=own_enabled,
                         domain=[0, 0, 2, 1], digest=("a5" * 32), bands=1, runs=1, upper=128))
    if filter_change:
        rows.append(dict(layer=B.hex(), kind=0, filter=F.hex(), enabled=True,
                         domain=[0, 0, 2, 1], digest=("b6" * 32), bands=1, runs=1, upper=128))
    source = [(r, r["kind"] == 1, r) for r in rows]
    current = copy.deepcopy(source)
    if filter_change == "enable":
        current = [(r, True, src) for r, _, src in current]
    elif filter_change == "duplicate":
        row = next(r for r in rows if r["kind"] == 0)
        new = dict(row, filter=DUP.hex())
        current.append((new, False, row))
    elif filter_change == "remove":
        current = [v for v in current if v[0]["kind"] != 0]
    grant = admission(mode, origin)
    count, required, resolved, required_frame, resolved_frame = numerical(mode, bool(filter_change))
    pre = prefix(count, required, resolved, grant, abi)
    own_fields, own_raw = own(rows)
    wire = bounds(rows, abi)
    source_inventory, current_inventory = inventory(source), inventory(current)
    source_cap = capability(pre, own_fields, digest(wire), digest(source_inventory), blend)
    candidate_cap = capability(pre, own_fields, digest(wire), digest(current_inventory), blend)
    union = UNION + pre + digest(wire) + blend
    return dict(name=name, mode=mode, origin=origin, coverageAbi=abi,
                scope="synthetic framing, not an OPEN/adoption/A8 quote or pixel proof",
                roots=[dict(uuid=x.hex(), mode=y) for x, y in roots],
                sourceRows=rows, sourceInventoryEnabled=[v[1] for v in source],
                candidateRows=[dict(row=r, instanceEnabled=e, sourceKey=key(src).hex()) for r, e, src in current],
                own=own_fields, numericalCount=count,
                frames=dict(required=required_frame, resolution=resolved_frame,
                            admission=frame(grant), blendIdentity=frame(semantic),
                            ownIdentity=frame(own_raw), sourceBounds=frame(wire),
                            sourceInventory=frame(source_inventory), candidateInventory=frame(current_inventory),
                            sourceCapability=frame(source_cap), candidateCapability=frame(candidate_cap), union=frame(union)),
                invariant=dict(blendIdentityUnchanged=True, sourceBoundsUnchanged=True,
                               unionDoesNotDoublePhysicalBudget=True))


def build():
    cases = [case("base-captured-no-own"), case("base-standalone-no-own", origin=1),
             case("noise-captured-no-own", mode=1), case("noise-standalone-no-own", mode=1, origin=1),
             case("own-enabled", own_enabled=True), case("own-disabled", own_enabled=False),
             case("native64-pure-frame", abi="native64-v1"), case("reversed-root-order", reverse=True),
             case("disabled-filter-maskEnabled-true-enable", filter_change="enable"),
             case("disabled-filter-maskEnabled-true-duplicate", filter_change="duplicate"),
             case("disabled-filter-maskEnabled-true-remove", filter_change="remove")]
    assert cases[0]["frames"]["admission"] != cases[1]["frames"]["admission"]
    assert cases[0]["frames"]["sourceCapability"] != cases[6]["frames"]["sourceCapability"]
    assert cases[0]["frames"]["blendIdentity"] != cases[7]["frames"]["blendIdentity"]
    for c in cases[8:]:
        assert c["frames"]["sourceCapability"] != c["frames"]["candidateCapability"]
    assert len({c["frames"]["union"]["sha256"] for c in cases[8:]}) == 1
    rejects = []
    for name, call in [("unknown-mode-tag", lambda: blend_identity([(A, 3)])),
                       ("duplicate-root", lambda: blend_identity([(A, 0), (A, 1)])),
                       ("missing-origin-enum", lambda: admission(0, 2)),
                       ("base-has-Noise-units", lambda: admission(0, 0, noise=1)),
                       ("base-limit-raised", lambda: admission(0, 0, base=268435457))]:
        try:
            call()
        except ValueError as e:
            rejects.append(dict(name=name, rejected=True, reason=str(e)))
        else:
            raise AssertionError(name)
    assert blend_identity([(A, 0), (B, 0)]) is None
    policies = [dict(name="all-Normal-source8", accepted=False, reason="use old minimal source route"),
                dict(name="wasm-endpoint-native64-bound", accepted=False, reason="compiled ABI mismatch"),
                dict(name="candidate-proof-loss", sameBlendIdentity=True, samePixelsPossible=True,
                     sourceOpaqueExtra=32, candidateGeneral=256, reason="recompute actual staged candidate proof"),
                dict(name="captured-origin-remote-TLS-None", renews=False),
                dict(name="Source7-PCND8", accepted=False),
                dict(name="foreign-copy-via-Source8-recognition", accepted=False)]
    return dict(status="PROPOSED_SOURCE8_FRAMING_ONLY_NO_PRODUCT_BODY_OR_EXECUTION", protocol=8,
                registryContract=4, maskRegistration=MASK, blendRegistration=BLEND,
                registrationFrames=dict(mask=frame(mask_registration()), blend=frame(blend_registration())),
                vectors=cases, independentRejections=rejects, policyOnly=policies)


def main():
    data = json.dumps(build(), indent=2) + "\n"
    path = HERE / "vectors.json"
    if sys.argv[1:] == ["--check"]:
        if path.read_text() != data:
            raise SystemExit("Source8 proposed framing vector drift")
    elif not sys.argv[1:]:
        path.write_text(data)
    else:
        raise SystemExit("usage: generate.py [--check]")
    print("11 synthetic frames /5 structural rejections /6 policy-only controls", digest(data.encode()).hex())

if __name__ == "__main__":
    main()
