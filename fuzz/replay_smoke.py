#!/usr/bin/env python3
"""Stable CI replay of manifest-listed existing seeds, sorted by seed name."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parent
TARGETS = ("avif", "heif", "paint", "pdn", "dcx", "radiance", "exr")
MUTATIONS = 256
INPUT_MAX = 256 << 10


def verify(row):
    source = (REPO / row["source"]).resolve()
    if not source.is_relative_to(REPO):
        raise ValueError("Seed must remain inside this repository")
    with source.open("rb") as stream:
        data = stream.read(INPUT_MAX + 1)
    if (len(data) > INPUT_MAX or len(data) != row["bytes"]
            or hashlib.sha256(data).hexdigest() != row["sha256"]):
        raise ValueError(f"Seed provenance mismatch: {row['source']}")
    return source


REVIEWED_COUNTS = {"avif": 4, "heif": 3, "paint": 3, "pdn": 3, "dcx": 3,
                   "radiance": 4, "exr": 4}


def reviewed_inventory(manifest):
    if manifest["schema"] != "paintcraft.fuzz-seeds/v1":
        raise ValueError("Unknown seed manifest schema")
    seeds = manifest["seeds"]
    if len(seeds) != 24:
        raise ValueError("Update the reviewed 6168-case smoke when changing seed inventory")
    keys = [(row["target"], row["name"]) for row in seeds]
    if len(set(keys)) != len(keys) or any(target not in TARGETS for target, _ in keys):
        raise ValueError("Duplicate or unknown seed target/name")
    for target, count in REVIEWED_COUNTS.items():
        if sum(row["target"] == target for row in seeds) != count:
            raise ValueError("Seed inventory differs from the reviewed per-target counts")
    for row in seeds:
        verify(row)
    return seeds


def receipt_cases(target, seed_count, output):
    match = re.fullmatch(
        r"target=(\w+) cases=(\d+) accepted=(\d+) rejected=(\d+) "
        r"seed=0x5041494e54435241 mode=deterministic-replay policy=invariants-fail-v1\n",
        output)
    if not match:
        raise ValueError("Unexpected replay receipt or missing invariant-failure policy")
    name, cases, accepted, rejected = match.groups()
    cases, accepted, rejected = int(cases), int(accepted), int(rejected)
    if (name.lower() != target or cases != seed_count * (MUTATIONS + 1)
            or accepted + rejected != cases or accepted == 0):
        raise ValueError("Replay coverage/count or positive-control mismatch")
    return cases


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--replay", type=Path, required=True)
    args = parser.parse_args()
    seeds = reviewed_inventory(json.loads((ROOT / "seeds.json").read_text()))
    total = 0
    for target in TARGETS:
        selected = sorted((row for row in seeds if row["target"] == target),
                          key=lambda row: row["name"])
        if not selected:
            raise ValueError(f"No seed for {target}")
        result = subprocess.run([str(args.replay.resolve()), target, str(MUTATIONS),
                                 *[str(verify(row)) for row in selected]],
                                stdout=subprocess.PIPE, text=True, timeout=60)
        print(result.stdout, end="", flush=True)
        result.check_returncode()
        total += receipt_cases(target, len(selected), result.stdout)
    for row in seeds:
        verify(row)
    if total != 6168:
        raise ValueError("Incomplete deterministic replay")
    print("Verified 24 seed hashes and 6168 deterministic cases; no coverage-guided run")


if __name__ == "__main__":
    main()
