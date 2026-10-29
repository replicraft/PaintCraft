"""Separate stock raster/TIFF/icon stream; historical seven targets are unchanged."""
import json
from replay_smoke import ROOT, MUTATIONS, receipt_cases, verify
import subprocess

TARGETS = ("raster", "tiff_pages", "icons")
COUNTS = {"raster": 17, "tiff_pages": 6, "icons": 6}
TOTAL = sum(COUNTS.values()) * (MUTATIONS + 1)


def inventory(manifest):
    if manifest["schema"] != "paintcraft.stock-fuzz-seeds/v1":
        raise ValueError("Unknown stock seed schema")
    rows = manifest["seeds"]
    keys = [(row["target"], row["name"]) for row in rows]
    if len(rows) != sum(COUNTS.values()) or len(set(keys)) != len(keys):
        raise ValueError("Stock inventory count or duplicate drift")
    for target, count in COUNTS.items():
        selected = [row for row in rows if row["target"] == target]
        if len(selected) != count or not any(row["accepted"] for row in selected):
            raise ValueError("Stock per-target inventory or positive control drift")
    for row in rows:
        verify(row)
    return rows


def run(replay):
    seeds = inventory(json.loads((ROOT / "stock-seeds.json").read_text()))
    total = 0
    for target in TARGETS:
        selected = sorted((row for row in seeds if row["target"] == target), key=lambda row: row["name"])
        result = subprocess.run([str(replay.resolve()), target, str(MUTATIONS), *[str(verify(row)) for row in selected]], stdout=subprocess.PIPE, text=True, timeout=60)
        print(result.stdout, end="", flush=True)
        result.check_returncode()
        total += receipt_cases(target.replace("_", ""), len(selected), result.stdout)
    for row in seeds:
        verify(row)
    if total != TOTAL:
        raise ValueError("Incomplete stock deterministic replay")
    print(f"Verified {len(seeds)} additional stock seed hashes and {total} deterministic cases; historical seven-target6168 stream separate; no coverage-guided claim")
