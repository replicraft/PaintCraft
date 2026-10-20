"""Derive phase distributions and ownership counters from separate history traces."""
import argparse
import ast
import csv
import io
import re
import statistics
from pathlib import Path

from run_history import CASES, VARIANTS, CODEC_CASES, CODEC_VARIANTS
from summarize_composite import read_run


def percentile(values, fraction):
    values = sorted(values)
    position = (len(values) - 1) * fraction
    lower = int(position)
    upper = min(lower + 1, len(values) - 1)
    return values[lower] + (values[upper] - values[lower]) * (position - lower)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--suite", choices=["ownership", "codecs"], default="ownership")
    args = parser.parse_args()
    cases = CODEC_CASES if args.suite == "codecs" else CASES
    variants = CODEC_VARIANTS if args.suite == "codecs" else VARIANTS
    files = read_run(args.directory)
    phases = []
    ownership = []
    for case in cases:
        for variant in variants:
            stem = args.directory / f"{case}-{variant}"
            grouped = {}
            for row in csv.DictReader(io.StringIO(files[stem.name + ".csv"].decode())):
                grouped.setdefault(row["phase"], []).append(int(row["ns"]) / 1e6)
            for phase, values in grouped.items():
                phases.append([case, variant, phase, len(values), statistics.median(values),
                               percentile(values, .95), percentile(values, .99), max(values)])
            text = files[stem.name + ".txt"].decode()
            workspace = re.search(r"codec_workspace_peak=(\d+) codec_scratch_bytes=(\d+)", text)
            rss = re.search(r"rss_before_release_kib=(?:Some\((\d+)\)|None) rss_after_release_kib=(?:Some\((\d+)\)|None)", text)
            for stage in ["before_release", "after_release"]:
                match = re.search(stage + r"=MemoryUsage \{ ([^}]+) \}", text)
                assert match, stem
                counters = ast.literal_eval("{" + re.sub(r"(\w+):", r'"\1":', match[1]) + "}")
                decoded, leased, encoded, metadata = [counters[key] for key in
                    ["decoded_bytes", "leased_decoded_bytes", "encoded_bytes", "metadata_weight"]]
                ownership.append([case, variant, stage, decoded, leased, encoded, metadata,
                                  decoded + encoded + metadata, counters["packed_entries"], counters["logical_payloads"],
                                  counters.get("compressed_bytes", 0), counters.get("compressed_payloads", 0),
                                  int(workspace[1]) if workspace else 0, int(workspace[2]) if workspace else 0,
                                  rss[1 if stage == "before_release" else 2] if rss else None])
    for name, header, rows in [
        ("phase-summary.csv", ["case", "variant", "phase", "events", "median_ms", "p95_ms", "p99_ms", "max_ms"], phases),
        ("ownership-summary.csv", ["case", "variant", "stage", "decoded_bytes", "leased_decoded_bytes", "encoded_bytes",
                                   "metadata_weight", "known_weight", "packed_entries", "logical_payloads",
                                   "compressed_bytes", "compressed_payloads", "codec_workspace_peak", "codec_scratch_bytes", "current_rss_kib"], ownership),
    ]:
        with (args.directory.parent / name).open("w") as output:
            writer = csv.writer(output, lineterminator="\n")
            writer.writerow(header)
            writer.writerows(rows)


if __name__ == "__main__":
    main()
