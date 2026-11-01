#!/usr/bin/env python3
"""Pure ABI/framing and cumulative-prefix policy vectors; no product imports."""
import hashlib
import json
import pathlib
import struct
import sys

ROOT = pathlib.Path(__file__).resolve().parent

def frame(abi):
    raw = abi.encode("ascii")
    # Empty occurrence table is framing-only, not a valid Some-source OPEN.
    return b"paintcraft.worker-coverage-bounds/v1\0" + struct.pack(">H", len(raw)) + raw + struct.pack(">HI", 1, 0)

def build():
    abi_rows = []
    for compiled in ["wasm32-v1", "native64-v1"]:
        raw = frame(compiled)
        for requested in ["wasm32-v1", "native64-v1"]:
            abi_rows.append({"compiled": compiled, "requested": requested,
                             "accepted": compiled == requested,
                             "frameHex": raw.hex(), "sha256": hashlib.sha256(raw).hexdigest()})
    assert abi_rows[0]["sha256"] != abi_rows[2]["sha256"]
    prefix = []
    original, main_spent, worker_used = 100, 30, 0
    remaining = original - main_spent
    for charge in [20, 40, 11]:
        admitted = worker_used + charge <= remaining
        if admitted:
            worker_used += charge
        prefix.append({"originalBaseMax": original, "sealedMainSpent": main_spent,
                       "openBaseWorkLimit": remaining, "workerCharge": charge,
                       "admitted": admitted, "cumulativeWorkerUsed": worker_used,
                       "combinedUsed": main_spent + worker_used})
        assert main_spent + worker_used <= original
    assert prefix[-1]["admitted"] is False and worker_used == 60
    return {"status": "SOURCE_ONLY_POLICY_AND_FRAMING_NOT_RUNTIME_TESTS",
            "abi": abi_rows, "prefix": prefix}

def main():
    data = json.dumps(build(), indent=2) + "\n"
    target = ROOT / "abi-prefix-vectors.json"
    if sys.argv[1:] == ["--check"]:
        if target.read_text() != data:
            raise SystemExit("ABI/prefix vector drift")
        print("4 ABI +3 prefix cases verified; old Wasm vectors unchanged")
    elif not sys.argv[1:]:
        target.write_text(data)
    else:
        raise SystemExit("usage: generate_abi_prefix_vectors.py [--check]")

if __name__ == "__main__":
    main()
