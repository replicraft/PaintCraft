#!/usr/bin/env python3
"""Independent stdlib framing vectors; synthetic digests/bounds, NOT A8 quotes."""
import hashlib
import json
import pathlib
import struct
import sys

ROOT = pathlib.Path(__file__).resolve().parent

def n(value, width):
    if not isinstance(value, int) or isinstance(value, bool) or not 0 <= value < 1 << (8 * width):
        raise ValueError("unsigned field")
    return value.to_bytes(width, "big")

def text(value):
    data = value.encode("ascii")
    if not 0 < len(data) <= 96:
        raise ValueError("identifier")
    return n(len(data), 2) + data

def digest(data):
    return hashlib.sha256(data).digest()

def identity(rows):
    ordered = sorted(rows, key=lambda row: row[0])
    if len({row[0] for row in ordered}) != len(ordered):
        raise ValueError("duplicate layer")
    return b"paintcraft.worker-layer-masks/v1\0" + n(len(rows), 4) + b"".join(
        uuid + n(enabled, 1) + coverage for uuid, enabled, coverage in ordered
    )

def bounds(rows):
    # (layerUUID,kind,filterUUID,enabled,domain,A8digest,bands,runs,upper)
    ordered = sorted(rows, key=lambda row: row[:3])
    if len({row[:3] for row in ordered}) != len(ordered):
        raise ValueError("duplicate occurrence")
    result = b"paintcraft.worker-coverage-bounds/v1\0" + text("wasm32-v1") + n(1, 2) + n(len(rows), 4)
    for uuid, kind, fid, enabled, domain, coverage, bands, runs, upper in ordered:
        if kind not in (0, 1) or enabled not in (0, 1) or (kind == 1 and fid != bytes(16)):
            raise ValueError("coverage key/flag")
        result += uuid + n(kind, 1) + fid + n(enabled, 1)
        result += struct.pack(">qqqq", *domain) + coverage + n(bands, 8) + n(runs, 8) + n(upper, 8)
    return result

def admission(noise, origin=0):
    base, extra = 268435456, (1 << 35) if noise else 0
    return b"paintcraft.source7-admission/v2\0" + n(int(noise), 1) + n(origin, 1) + b"".join(
        n(value, 8) for value in [64 << 20, base, extra, base + extra]
    ) + n(1, 2) + n(64, 2) + n(8388608, 8) + n(134217728 if noise else 0, 8)

def appearance_prefix(required, resolution, grant, ids):
    feature, resource, workload = ids
    return (n(3, 2) + required + resolution + digest(grant)
            + text(feature) + text(resource) + n(2, 2) + n(1, 2) + n(1, 2)
            + text(workload) + n(1, 2))

def inventory(rows):
    # (coverage occurrence tuple, filter-instance enabled, source occurrence key)
    ordered = sorted(rows, key=lambda item: item[0][:3])
    if len({item[0][:3] for item in ordered}) != len(rows):
        raise ValueError("duplicate inventory key")
    result = b"paintcraft.worker-mask-inventory/v1\0" + n(len(rows), 4)
    for row, instance_enabled, origin in ordered:
        uuid, kind, fid, enabled, domain, coverage, bands, runs, upper = row
        ouuid, okind, ofid = origin
        result += uuid + n(kind, 1) + fid + n(instance_enabled, 1) + n(enabled, 1)
        result += struct.pack(">qqqq", *domain) + coverage + n(bands, 8) + n(runs, 8) + n(upper, 8)
        result += ouuid + n(okind, 1) + ofid
    return result

def self_inventory(rows):
    return [(row, 1, row[:3]) for row in rows]

def capability(required, resolution, grant, ids, rows, own, source_bounds, current_inventory):
    enabled = sum(row[1] for row in rows)
    own_bands, own_runs, own_upper = own
    return (
        b"paintcraft.worker-capabilities/v3\0" + appearance_prefix(required, resolution, grant, ids)
        + n(len(rows), 4) + n(enabled, 4) + n(own_bands, 8) + n(own_runs, 8) + n(own_upper, 8)
        + digest(identity(rows)) + source_bounds + digest(inventory(current_inventory))
    )

def union_capability(required, resolution, grant, ids, source_bounds):
    return (b"paintcraft.worker-capability-union/v1\0"
            + appearance_prefix(required, resolution, grant, ids) + source_bounds)


def build():
    a, b, f = bytes.fromhex("11" * 16), bytes.fromhex("22" * 16), bytes.fromhex("33" * 16)
    coverage = bytes.fromhex("a5" * 32)
    # Digest of an empty tuple list from a REAL numerical hash is intentionally
    # not assumed: synthetic opaque inputs keep this framing test independent.
    required, resolution = bytes.fromhex("01" * 32), bytes.fromhex("02" * 32)
    ids = ("layer-masks.v1", "layer-mask-final-alpha-rgba8-v1", "base-work-v1")
    scenarios = []
    for name, uuid, enabled, upper, noise, alias in [
        ("empty-algorithms-enabled-empty", a, 1, 0, False, False),
        ("disabled-intent", a, 0, 0, False, False),
        ("same-coverage-other-layer", b, 1, 0, False, False),
        ("changed-admitted-bound", a, 1, 128, False, False),
        ("noise-owned", a, 1, 0, True, False),
        ("serialized-alias-occurrences", a, 1, 128, False, True),
    ]:
        rows = [(uuid, enabled, coverage)]
        occurrence = [(uuid, 1, bytes(16), enabled, (0, 0, 2, 1), coverage, 0, 0, upper)]
        if alias:
            occurrence.append((uuid, 0, f, enabled, (0, 0, 2, 1), coverage, 0, 0, upper))
        grant = admission(noise)
        frames = {
            "identity": identity(rows), "coverageBounds": bounds(occurrence), "admission": grant,
            "capability": capability(required, resolution, grant, ids, rows, (0, 0, upper), digest(bounds(occurrence)), self_inventory(occurrence)),
        }
        assert bounds(occurrence) == bounds(list(reversed(occurrence)))
        scenarios.append({
            "name": name, "algorithmCount": 0, "syntheticRequired": required.hex(),
            "syntheticResolution": resolution.hex(), "storedOwn": 1, "enabledOwn": enabled,
            "syntheticOwnDecodeUpper": upper, "syntheticAllDecodeUpper": sum(r[-1] for r in occurrence),
            "serializedOccurrences": len(occurrence), "noise": noise,
            "frames": {key: {"hex": value.hex(), "sha256": digest(value).hex(), "bytes": len(value)}
                       for key, value in frames.items()},
        })
    assert len({row["frames"]["capability"]["sha256"] for row in scenarios}) == len(scenarios)
    try:
        identity([(a, 1, coverage), (a, 0, coverage)])
    except ValueError:
        pass
    else:
        raise AssertionError("duplicate UUID accepted")
    # One source own mask + one active FilterMask; candidate edits are separate
    # contexts anchored to one immutable wire source. Bounds/data remain synthetic.
    own_row = (a, 1, bytes(16), 1, (0, 0, 2, 1), coverage, 1, 1, 128)
    filter_row = (a, 0, f, 1, (0, 0, 2, 1), coverage, 1, 1, 128)
    fixture = []
    for source_name, source_mask_enabled, source_instance_enabled in [("active-filter-mask-source", 1, 1), ("disabled-filter-mask-source", 0, 1), ("initially-disabled-filter-enabled-mask", 1, 0)]:
        filter_row = (*filter_row[:3], source_mask_enabled, *filter_row[4:])
        source_rows = [own_row, filter_row]
        own_identity = [(a, 1, coverage)]
        source_bounds = digest(bounds(source_rows))
        grant = admission(False)
        base_inventory = [(own_row, 1, own_row[:3]), (filter_row, source_instance_enabled, filter_row[:3])]
        duplicate_row = (a, 0, bytes.fromhex("44" * 16), *filter_row[3:])
        states = {
            "base": base_inventory,
            "duplicate": base_inventory + [(duplicate_row, source_instance_enabled, filter_row[:3])],
            "remove": self_inventory([own_row]),
            "filter-toggle-enabled": [(own_row, 1, own_row[:3]), (filter_row, 1 - source_instance_enabled, filter_row[:3])],
        }
        source_frame = capability(required, resolution, grant, ids, own_identity,
                                  (1, 1, 128), source_bounds, base_inventory)
        union_frame = union_capability(required, resolution, grant, ids, source_bounds)
        for name, current in states.items():
            frame = capability(required, resolution, grant, ids, own_identity,
                               (1, 1, 128), source_bounds, current)
            assert (frame == source_frame) == (name == "base")
            assert union_capability(required, resolution, grant, ids, source_bounds) == union_frame
            assert inventory(current) == inventory(list(reversed(current)))
            fixture.append({
                "name": source_name + "/" + name, "storedFilterMaskEnabled": filter_row[3], "sourceFilterInstanceEnabled": source_instance_enabled, "sourceLogicalOccurrences": 2,
                "candidateLogicalOccurrences": len(current),
                "candidateBands": sum(item[0][6] for item in current),
                "candidateRuns": sum(item[0][7] for item in current),
                "candidateLogicalInheritedBounds": sum(item[0][8] for item in current),
                "immutableSourceDecodeReservation": 256,
                "actualSharedBackings": 1,
                "ownIdentitySha256": digest(identity(own_identity)).hex(),
                "sourceCoverageSha256": source_bounds.hex(),
                "sourceCapabilitySha256": digest(source_frame).hex(),
                "candidateInventory": {"hex": inventory(current).hex(), "sha256": digest(inventory(current)).hex()},
                "candidateCapability": {"hex": frame.hex(), "sha256": digest(frame).hex()},
                "unionCapability": {"hex": union_frame.hex(), "sha256": digest(union_frame).hex()},
            })
    origin_vectors = []
    rows = [(a, 1, coverage)]
    occurrence = [(a, 1, bytes(16), 1, (0, 0, 2, 1), coverage, 0, 0, 0)]
    source_bounds = digest(bounds(occurrence))
    for noise in [False, True]:
        for origin, authority in [(0, "captured-job"), (1, "standalone-view-jobs")]:
            grant = admission(noise, origin)
            frames = {"admission": grant,
                "capability": capability(required, resolution, grant, ids, rows, (0, 0, 0), source_bounds, self_inventory(occurrence)),
                "union": union_capability(required, resolution, grant, ids, source_bounds)}
            origin_vectors.append({"noise": noise, "workAuthority": authority,
                "frames": {key: {"hex": raw.hex(), "sha256": digest(raw).hex(), "bytes": len(raw)} for key, raw in frames.items()}})
    assert len({v["frames"]["admission"]["sha256"] for v in origin_vectors}) == 4
    assert len({v["frames"]["capability"]["sha256"] for v in origin_vectors}) == 4
    assert len({v["frames"]["union"]["sha256"] for v in origin_vectors}) == 4
    # Wider count regression without allocating 65536 rows: canonical prefix
    # literal demonstrates there is no legacy u16 wrap/acceptance ceiling.
    wide_count = n(65536, 4)
    assert wide_count.hex() == "00010000"
    authority = []
    for stored, mode in [(False, False), (True, False), (True, True), (False, True)]:
        permitted = (not stored) or mode
        authority.append({
            "storedNoise": stored, "mode": "noise" if mode else "base",
            "prospectiveReopenExample": not stored and mode,
            "openPermitted": permitted,
            "admissionHex": admission(mode).hex(),
            "admissionSha256": digest(admission(mode)).hex(),
        })
    assert [x["openPermitted"] for x in authority] == [True, False, True, True]
    used = 0
    accounting = []
    for phase, charge in [("probe", 12), ("adopt", 13), ("quote-only-control", 3), ("begin-control", 7), ("denied-frame", 16)]:
        before = used
        admitted = before + charge <= 50
        if admitted:
            used += charge
        accounting.append({"phase": phase, "owner": "one-open-owner", "before": before,
                           "illustrativeCharge": charge, "admitted": admitted, "after": used})
    assert used == 35 and not accounting[-1]["admitted"]
    return {"status": "PROPOSED_FRAMING_ONLY_NOT_EXECUTION_AUTHORITY", "vectors": scenarios, "originVectors": origin_vectors, "candidateMutationVectors": fixture, "count65536_u32be": wide_count.hex(), "authorityVectors": authority, "illustrativeAccountingTrace": accounting}

def main():
    output = json.dumps(build(), indent="\t") + "\n"
    target = ROOT / "framing-vectors.json"
    if sys.argv[1:] == ["--check"]:
        if target.read_text() != output:
            raise SystemExit("framing vector drift")
        print("6 framing +12 mutation +4 origin +4 stored-Noise mode +1 accounting trace verified; synthetic digests/bounds only")
    elif not sys.argv[1:]:
        target.write_text(output)
    else:
        raise SystemExit("usage: generate_vectors.py [--check]")

if __name__ == "__main__":
    main()
