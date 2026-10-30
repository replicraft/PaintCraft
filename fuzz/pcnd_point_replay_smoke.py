"""Separate independent point-profile corpus: 7453 raw + 7453 outer-SHA cases."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import tempfile
from pcnd_filter_replay_smoke import retain_failure
from pcnd_semantic_smoke import STATE, mutations
from replay_smoke import MUTATIONS, ROOT, receipt_cases, verify

BASE = "crates/services/paintcraft-formats/tests/pcnd-point-fixtures/"
MANIFEST = "pcnd-point-seeds.json"
SEEDS, POSITIVE = 29, 23
TOTAL = SEEDS * (MUTATIONS + 1)


def inventory(manifest):
    if manifest["schema"] != "paintcraft.pcnd-point-fuzz-seeds/v1":
        raise ValueError("Unknown PCND point-profile seed schema")
    for key, name in [("oracle", "oracle.json"), ("generator", "generate.py")]:
        if manifest[key]["source"] != BASE + name:
            raise ValueError("PCND point-profile provenance path changed")
        verify(manifest[key])
    rows = manifest["seeds"]
    expected = {r["file"]: r for r in json.loads(verify(manifest["oracle"]).read_text())["fixtures"]}
    if len(rows) != SEEDS or len(expected) != SEEDS or len({r["name"] for r in rows}) != SEEDS:
        raise ValueError("PCND point-profile count or duplicate drift")
    if any(type(r.get("accepted")) is not bool for r in rows):
        raise ValueError("PCND point-profile outcomes must be boolean")
    if {r["name"] for r in rows} != set(expected) or sum(r["accepted"] for r in rows) != POSITIVE:
        raise ValueError("PCND point-profile independent outcomes drift")
    for row in rows:
        if row["target"] != "pcnd" or row["source"] != BASE + row["name"]:
            raise ValueError("PCND point-profile target/path changed")
        original = expected[row["name"]]
        if type(row["accepted"]) is not bool or any(row[k] != original[k] for k in ["bytes", "sha256", "accepted"]):
            raise ValueError("PCND point-profile differs from independent oracle")
        verify(row)
    return rows


def write_inventory():
    import hashlib
    def record(path):
        data = path.read_bytes()
        return dict(source=path.relative_to(ROOT.parent).as_posix(), bytes=len(data), sha256=hashlib.sha256(data).hexdigest())
    folder = ROOT.parent / BASE
    oracle = folder / "oracle.json"
    rows = json.loads(oracle.read_text())["fixtures"]
    manifest = dict(schema="paintcraft.pcnd-point-fuzz-seeds/v1", oracle=record(oracle), generator=record(folder / "generate.py"),
                    seeds=[dict(target="pcnd", name=r["file"], source=BASE+r["file"], bytes=r["bytes"], sha256=r["sha256"], accepted=r["accepted"]) for r in rows])
    inventory(manifest)
    (ROOT / MANIFEST).write_text(json.dumps(manifest, indent=2)+"\n")


def run(replay):
    manifest = json.loads((ROOT / MANIFEST).read_text())
    rows = sorted(inventory(manifest), key=lambda r: r["name"])
    suffix = "seed=0x5041494e54435241 mode=deterministic-replay policy=invariants-fail-v1\n"
    for row in rows:
        result = subprocess.run([str(replay.resolve()), "pcnd", "0", str(verify(row))], capture_output=True, text=True, timeout=60)
        result.check_returncode()
        good = int(row["accepted"])
        if result.stdout != f"target=Pcnd cases=1 accepted={good} rejected={1-good} " + suffix:
            raise ValueError(f"PCND point-profile original outcome mismatch: {row['name']}: {result.stdout!r}")
    result = subprocess.run([str(replay.resolve()), "pcnd", str(MUTATIONS), *[str(verify(r)) for r in rows]], capture_output=True, text=True, timeout=60)
    print(result.stdout, end="", flush=True)
    result.check_returncode()
    if receipt_cases("pcnd", SEEDS, result.stdout) != TOTAL:
        raise ValueError("Incomplete raw PCND point-profile replay")
    raw = re.fullmatch(r"target=Pcnd cases=(\d+) accepted=(\d+) rejected=(\d+) " + re.escape(suffix), result.stdout)
    if not raw or int(raw[2]) < POSITIVE:
        raise ValueError("Raw point-profile stream lost original positive controls")
    state, cases, accepted, rejected = STATE, 0, 0, 0
    parent = ROOT.parent / "docs/validation/pcnd-point-fuzz"
    parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="point-semantic-inputs-", dir=parent) as folder:
        for row in rows:
            paths = []
            for index, (data, state) in enumerate(mutations(verify(row).read_bytes(), state)):
                path = Path(folder) / f"case-{index}.pcnd"
                path.write_bytes(data)
                paths.append(str(path))
            try:
                result = subprocess.run([str(replay.resolve()), "pcnd", "0", *paths], capture_output=True, text=True, timeout=60)
            except subprocess.TimeoutExpired as error:
                try:
                    retain_failure(parent, paths, row, state, error.stdout, error.stderr, timeout=error.timeout)
                except OSError as storage_error:
                    print(f"Failed to retain timeout inputs: {storage_error}", flush=True)
                raise
            print(f"seed={row['name']} {result.stdout}", end="", flush=True)
            if result.returncode:
                retain_failure(parent, paths, row, state, result.stdout, result.stderr, returncode=result.returncode)
            result.check_returncode()
            match = re.fullmatch(r"target=Pcnd cases=(\d+) accepted=(\d+) rejected=(\d+) " + re.escape(suffix), result.stdout)
            if not match or int(match[1]) != MUTATIONS+1 or int(match[2])+int(match[3]) != MUTATIONS+1:
                raise ValueError("Incomplete PCND point-profile checksum-aware batch")
            cases += int(match[1]); accepted += int(match[2]); rejected += int(match[3])
    inventory(manifest)
    if cases != TOTAL or accepted < POSITIVE or accepted+rejected != cases:
        raise ValueError("Incomplete PCND point-profile checksum-aware stream")
    print(f"PCND point profiles original={SEEDS}; raw={TOTAL}; checksum-aware={cases} accepted={accepted} rejected={rejected}; separate from historical seven-target6168, stock7453, base-PCND3598, PCND3 4112 and PCND4-mask6168; no guided/sanitizer claim")


if __name__ == "__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--replay",type=Path)
    parser.add_argument("--write-inventory",action="store_true")
    args=parser.parse_args()
    if args.write_inventory and not args.replay: write_inventory()
    elif args.replay and not args.write_inventory: run(args.replay)
    else: parser.error("Choose --replay or --write-inventory")
