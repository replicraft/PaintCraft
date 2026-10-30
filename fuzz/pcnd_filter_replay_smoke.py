"""Separate PCND3 filter inventory: 4112 raw plus 4112 outer-SHA-aware cases."""
import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
from pcnd_semantic_smoke import STATE, mutations
from replay_smoke import MUTATIONS, ROOT, receipt_cases, verify

SEEDS = 16
TOTAL = SEEDS * (MUTATIONS + 1)
BASE = "crates/services/paintcraft-formats/tests/pcnd-filter-fixtures/"


def inventory(manifest):
    if manifest["schema"] != "paintcraft.pcnd-filter-fuzz-seeds/v1":
        raise ValueError("Unknown PCND filter seed schema")
    for key, name in [("oracle", "oracle.json"), ("generator", "generate.py")]:
        if manifest[key]["source"] != BASE + name:
            raise ValueError("PCND filter provenance source changed")
        verify(manifest[key])
    oracle = json.loads(verify(manifest["oracle"]).read_text())["fixtures"]
    rows = manifest["seeds"]
    if len(rows) != SEEDS or len({r["name"] for r in rows}) != SEEDS:
        raise ValueError("PCND filter seed count/duplicate drift")
    expected = {row["file"]: row for row in oracle}
    if len(expected) != SEEDS or {r["name"] for r in rows} != set(expected):
        raise ValueError("PCND filter oracle inventory changed")
    if sum(row["accepted"] for row in rows) != 5:
        raise ValueError("PCND filter positive/negative profile drift")
    for row in rows:
        if row["target"] != "pcnd" or row["source"] != BASE + row["name"]:
            raise ValueError("PCND filter target/source drift")
        original = expected[row["name"]]
        if type(row["accepted"]) is not bool or any(
            row[k] != original[k] for k in ["bytes", "sha256", "accepted"]
        ):
            raise ValueError("PCND filter manifest differs from independent oracle")
        verify(row)
    return rows


def write_inventory():
    """Bind the separately authored fixture oracle; never call a Rust writer."""
    def record(path):
        import hashlib
        data = path.read_bytes()
        return dict(source=path.relative_to(ROOT.parent).as_posix(), bytes=len(data),
                    sha256=hashlib.sha256(data).hexdigest())
    folder = ROOT.parent / BASE
    oracle = folder / "oracle.json"
    rows = json.loads(oracle.read_text())["fixtures"]
    manifest = dict(
        schema="paintcraft.pcnd-filter-fuzz-seeds/v1",
        oracle=record(oracle), generator=record(folder / "generate.py"),
        seeds=[dict(target="pcnd", name=row["file"], source=BASE + row["file"],
                    bytes=row["bytes"], sha256=row["sha256"], accepted=row["accepted"])
               for row in rows],
    )
    inventory(manifest)
    (ROOT / "pcnd-filter-seeds.json").write_text(json.dumps(manifest, indent=2) + "\n")


def retain_failure(parent, paths, row, state, stdout, stderr, *, returncode=None, timeout=None):
    failure = Path(tempfile.mkdtemp(prefix="filter-failure-", dir=parent))
    for generated in paths:
        shutil.copyfile(generated, failure / Path(generated).name)
    def output(value, name):
        if value is None: return ""
        raw = value if isinstance(value, bytes) else value.encode("utf-8")
        (failure / name).write_bytes(raw)
        return raw.decode("utf-8", errors="replace")
    (failure / "failure.json").write_text(json.dumps(dict(
        seed=row, stdout=output(stdout, "stdout.bin"), stderr=output(stderr, "stderr.bin"),
        returncode=returncode, timeout=timeout, state_after=state,
    ), indent=2) + "\n")
    print(f"Retained failing mutation batch: {failure}", flush=True)


def run(replay):
    manifest = json.loads((ROOT / "pcnd-filter-seeds.json").read_text())
    rows = sorted(inventory(manifest), key=lambda r: r["name"])
    # Exact original outcomes prevent a stale reader that rejects all PCND3
    # while accepting legacy inputs from passing aggregate positive counts.
    for row in rows:
        result = subprocess.run([str(replay.resolve()), "pcnd", "0", str(verify(row))],
                                capture_output=True, text=True, timeout=60)
        result.check_returncode()
        wanted = int(row["accepted"])
        expected = (f"target=Pcnd cases=1 accepted={wanted} rejected={1-wanted} "
                    "seed=0x5041494e54435241 mode=deterministic-replay policy=invariants-fail-v1\n")
        if result.stdout != expected:
            raise ValueError(f"Original PCND filter acceptance mismatch: {row['name']}: {result.stdout!r}")
    print("PCND filters original acceptance controls: 16; separate from mutation totals", flush=True)
    result = subprocess.run(
        [str(replay.resolve()), "pcnd", str(MUTATIONS), *[str(verify(r)) for r in rows]],
        capture_output=True, text=True, timeout=60,
    )
    print(result.stdout, end="", flush=True)
    print(result.stderr, end="", flush=True)
    result.check_returncode()
    if receipt_cases("pcnd", SEEDS, result.stdout) != TOTAL:
        raise ValueError("Incomplete raw PCND3 filter replay")
    print(f"PCND filters raw: seeds={SEEDS} cases={TOTAL}; standalone stream", flush=True)
    state, cases, accepted, rejected = STATE, 0, 0, 0
    parent = ROOT.parent / "docs/validation/pcnd3-fuzz"
    parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="filter-semantic-inputs-", dir=parent) as folder:
        for row in rows:
            paths = []
            for index, (data, state) in enumerate(mutations(verify(row).read_bytes(), state)):
                p = Path(folder) / f"case-{index}.pcnd"
                p.write_bytes(data)
                paths.append(str(p))
            try:
                result = subprocess.run([str(replay.resolve()), "pcnd", "0", *paths],
                                        capture_output=True, text=True, timeout=60)
            except subprocess.TimeoutExpired as error:
                try:
                    retain_failure(parent, paths, row, state, error.stdout, error.stderr,
                                   timeout=error.timeout)
                except OSError as storage_error:
                    print(f"Could not retain timed-out input batch: {storage_error}", flush=True)
                raise  # Preserve the original timeout; it is never a passing replay.

            print(f"seed={row['name']} {result.stdout}", end="", flush=True)
            print(result.stderr, end="", flush=True)
            if result.returncode:
                retain_failure(parent, paths, row, state, result.stdout, result.stderr,
                               returncode=result.returncode)
            result.check_returncode()
            match = re.fullmatch(
                r"target=Pcnd cases=(\d+) accepted=(\d+) rejected=(\d+) "
                r"seed=0x5041494e54435241 mode=deterministic-replay policy=invariants-fail-v1\n",
                result.stdout,
            )
            if not match:
                raise ValueError("Unexpected PCND3 filter checksum-aware receipt")
            count, good, bad = map(int, match.groups())
            if count != MUTATIONS + 1 or good + bad != count:
                raise ValueError("Incomplete PCND3 filter checksum-aware batch")
            cases += count
            accepted += good
            rejected += bad
    inventory(manifest)
    if cases != TOTAL or accepted < 5 or accepted + rejected != cases:
        raise ValueError("Incomplete PCND3 filter semantic replay")
    print(f"PCND filters checksum-aware: seeds={SEEDS} cases={cases} accepted={accepted} rejected={rejected}; outer-SHA-only; separate from historical6168/stock7453/PCND3598; no guided/sanitizer claim")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--replay", type=Path)
    parser.add_argument("--write-inventory", action="store_true")
    args = parser.parse_args()
    if args.write_inventory:
        if args.replay: parser.error("Do not regenerate an inventory during replay")
        write_inventory()
    elif args.replay:
        run(args.replay)
    else: parser.error("Choose --replay or --write-inventory")
