"""Separate historical PCND seed inventory and deterministic replay."""
import argparse
import json
from pathlib import Path
import subprocess
from replay_smoke import ROOT, MUTATIONS, receipt_cases, verify

SEEDS = 14
TOTAL = SEEDS * (MUTATIONS + 1)


def inventory(manifest):
    if manifest["schema"] != "paintcraft.pcnd-fuzz-seeds/v1":
        raise ValueError("Unknown PCND seed schema")
    rows = manifest["seeds"]
    if len(rows) != SEEDS or len({row["name"] for row in rows}) != SEEDS:
        raise ValueError("PCND seed inventory count/duplicate drift")
    if sum(row["accepted"] for row in rows) != 10:
        raise ValueError("PCND positive/negative profile drift")
    for row in rows:
        if row["target"] != "pcnd":
            raise ValueError("Unknown PCND seed target")
        verify(row)
    return rows


def run(replay):
    rows = inventory(json.loads((ROOT / "pcnd-seeds.json").read_text()))
    rows = sorted(rows, key=lambda row: row["name"])
    result = subprocess.run(
        [str(replay.resolve()), "pcnd", str(MUTATIONS), *[str(verify(row)) for row in rows]],
        stdout=subprocess.PIPE, text=True, timeout=60,
    )
    print(result.stdout, end="", flush=True)
    result.check_returncode()
    cases = receipt_cases("pcnd", SEEDS, result.stdout)
    for row in rows:
        verify(row)
    if cases != TOTAL:
        raise ValueError("Incomplete separate PCND replay")
    print(f"Raw PCND: verified {SEEDS} separate seed hashes and {TOTAL} deterministic cases; historical6168/stock7453 streams unchanged; no coverage-guided claim")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--replay", type=Path, required=True)
    run(parser.parse_args().replay)


if __name__ == "__main__":
    main()
