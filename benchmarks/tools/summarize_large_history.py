"""Derive descriptive phase and ownership tables from large-history traces."""
import argparse
import ast
import csv
import io
import re
import statistics
from run_history import LARGE_CASES, LARGE_VARIANTS
from summarize_composite import read_run
from summarize_history import percentile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    from pathlib import Path
    parser.add_argument("traces", type=Path)
    args = parser.parse_args()
    files = read_run(args.traces)
    phases, ownership = [], []
    for case in LARGE_CASES:
        for variant in LARGE_VARIANTS:
            stem = f"{case}-{variant}"
            grouped = {}
            for row in csv.DictReader(io.StringIO(files[stem + ".csv"].decode())):
                grouped.setdefault(row["phase"], []).append(int(row["ns"]) / 1e6)
            for phase, values in grouped.items():
                phases.append([case, variant, phase, len(values), statistics.median(values),
                               percentile(values, .95), percentile(values, .99), max(values)])
            text = files[stem + ".txt"].decode()
            counters = []
            for name, kind in [("memory", "MemoryUsage"), ("store", "SpillUsage")]:
                match = re.search(name + "=" + kind + r" \{ ([^}]+) \}", text)
                assert match, stem
                counters.append(ast.literal_eval("{" + re.sub(r"(\w+):", r'"\1":', match[1]) + "}"))
            memory, store = counters
            assert memory["disk_bytes"] <= store["reserved_bytes"]
            assert store["cleanup_failures"] == 0
            ownership.append([case, variant, memory["decoded_bytes"], memory["leased_decoded_bytes"],
                              memory["encoded_bytes"], memory["metadata_weight"], memory["disk_bytes"],
                              memory["disk_payloads"], store["reserved_bytes"], store["segments"]])
    for name, fields, rows in [
        ("phase-summary.csv", ["case", "variant", "phase", "events", "median_ms", "p95_ms", "p99_ms", "max_ms"], phases),
        ("ownership-summary.csv", ["case", "variant", "decoded_bytes", "leased_decoded_bytes", "encoded_bytes",
                                   "metadata_weight", "disk_bytes", "disk_payloads", "store_reserved_bytes", "store_segments"], ownership),
    ]:
        with (args.traces.parent / name).open("w") as output:
            writer = csv.writer(output, lineterminator="\n")
            writer.writerow(fields)
            writer.writerows(rows)


if __name__ == "__main__":
    main()
