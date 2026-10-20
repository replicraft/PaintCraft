"""Summarize completed BF29 Hyperfine 2 evidence without qualifying a winner."""
import argparse
import csv
import hashlib
import json
from pathlib import Path
import statistics
import tarfile


def read_run(path):
    if path.is_dir():
        return {p.name: p.read_bytes() for p in path.iterdir() if p.is_file()}
    with tarfile.open(path, "r:gz") as archive:
        result = {}
        for member in archive.getmembers():
            if member.isfile():
                result[Path(member.name).name] = archive.extractfile(member).read()
        return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path, nargs="+")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    paths = sorted(p for root in args.root for p in root.glob("session-*") if p.is_dir() or p.name.endswith(".tar.gz"))
    grouped = {}
    for path in paths:
        files = read_run(path)
        # Verify original raw exports, logs and execution records before deriving metrics.
        for line in files["MANIFEST.sha256"].decode().splitlines():
            expected, name = line.split("  ", 1)
            assert hashlib.sha256(files[name]).hexdigest() == expected, (path, name)
        execution = json.loads(files["execution.json"])
        assert execution["execution_status"] == "completed", path
        assert execution["artifacts_before"] == execution["artifacts_after"], path
        config = json.loads(files["config.json"])
        case = config["workload"]["id"].removeprefix("cpu-spans-").removeprefix("cpu-cache-")
        iterations = config["workload"]["work_units"]
        raw = json.loads(files["hyperfine.json"])
        assert raw["schema_version"] == 2, "Review changed Hyperfine export schema explicitly"
        for variant in raw["results"]:
            assert len(variant["measurements"]) == 20
            assert all(row["exit_code"] == 0 for row in variant["measurements"])
            data = grouped.setdefault((case, variant["name"]), {"sessions": 0, "iterations": iterations, "measurements": []})
            assert data["iterations"] == iterations
            data["sessions"] += 1
            data["measurements"].extend(variant["measurements"])
    fields = ["case", "variant", "sessions", "runs", "work_units_per_process", "median_process_ms", "mean_process_ms",
              "stddev_process_ms", "min_process_ms", "max_process_ms", "work_units_per_second",
              "median_peak_rss_MiB", "max_peak_rss_MiB", "median_cpu_ms", "median_cpu_cycles", "median_instructions"]
    with args.output.open("w") as output:
        writer = csv.DictWriter(output, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        for (case, variant), data in sorted(grouped.items()):
            assert data["sessions"] == 3, f"Incomplete sessions for {case}/{variant}"
            measurements = data["measurements"]
            values = lambda key: [m[key]["value"] for m in measurements]
            wall = values("time_wall_clock")
            rss = values("memory_peak_resident")
            writer.writerow({
                "case": case, "variant": variant, "sessions": 3, "runs": len(wall), "work_units_per_process": data["iterations"],
                "median_process_ms": statistics.median(wall)*1000, "mean_process_ms": statistics.mean(wall)*1000,
                "stddev_process_ms": statistics.stdev(wall)*1000, "min_process_ms": min(wall)*1000,
                "max_process_ms": max(wall)*1000, "work_units_per_second": data["iterations"]/statistics.median(wall),
                "median_peak_rss_MiB": statistics.median(rss)/1048576, "max_peak_rss_MiB": max(rss)/1048576,
                "median_cpu_ms": statistics.median(values("time_cpu"))*1000,
                "median_cpu_cycles": statistics.median(values("cpu_cycles")), "median_instructions": statistics.median(values("instructions")),
            })


if __name__ == "__main__":
    main()
