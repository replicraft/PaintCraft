"""Derive phase distributions and ownership counters from separate history traces."""
import argparse
import ast
import csv
import re
import statistics
from pathlib import Path

from run_history import CASES, VARIANTS


def percentile(values, fraction):
    values = sorted(values)
    position = (len(values) - 1) * fraction
    lower = int(position)
    upper = min(lower + 1, len(values) - 1)
    return values[lower] + (values[upper] - values[lower]) * (position - lower)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    phases = []
    ownership = []
    for case in CASES:
        for variant in VARIANTS:
            stem = args.directory / f"{case}-{variant}"
            grouped = {}
            with stem.with_suffix(".csv").open() as source:
                for row in csv.DictReader(source):
                    grouped.setdefault(row["phase"], []).append(int(row["ns"]) / 1e6)
            for phase, values in grouped.items():
                phases.append([case, variant, phase, len(values), statistics.median(values),
                               percentile(values, .95), percentile(values, .99), max(values)])
            text = stem.with_suffix(".txt").read_text()
            for stage in ["before_release", "after_release"]:
                match = re.search(stage + r"=MemoryUsage \{ ([^}]+) \}", text)
                assert match, stem
                counters = ast.literal_eval("{" + re.sub(r"(\w+):", r'"\1":', match[1]) + "}")
                decoded, leased, encoded, metadata = [counters[key] for key in
                    ["decoded_bytes", "leased_decoded_bytes", "encoded_bytes", "metadata_weight"]]
                ownership.append([case, variant, stage, decoded, leased, encoded, metadata,
                                  decoded + encoded + metadata, counters["packed_entries"], counters["logical_payloads"]])
    for name, header, rows in [
        ("phase-summary.csv", ["case", "variant", "phase", "events", "median_ms", "p95_ms", "p99_ms", "max_ms"], phases),
        ("ownership-summary.csv", ["case", "variant", "stage", "decoded_bytes", "leased_decoded_bytes", "encoded_bytes",
                                   "metadata_weight", "known_weight", "packed_entries", "logical_payloads"], ownership),
    ]:
        with (args.directory.parent / name).open("w") as output:
            writer = csv.writer(output, lineterminator="\n")
            writer.writerow(header)
            writer.writerows(rows)


if __name__ == "__main__":
    main()
