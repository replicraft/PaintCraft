"""Prepare and explicitly run the adopted BF29 compositor comparison protocol."""
import argparse
import json
import os
from pathlib import Path
import platform
import subprocess

from benchmark_support import sha256, validate_config
from run_hyperfine import execute

ROOT = Path(__file__).resolve().parents[2]
ITERATIONS = {
    "tiny": 65536, "opaque1": 256, "sparse1": 128, "sparse5": 64,
    "sparse20": 16, "sparse100": 4, "dense5": 32, "uniform5": 8,
    "clipped100": 256,
}

CACHE_ITERATIONS = {"sparse1": 256, "sparse5": 128, "sparse20": 64, "sparse100": 16,
                    "dense5": 16, "global5": 16, "global-bottom5": 16, "viewport100": 64}


def output(argv):
    return subprocess.check_output(argv, cwd=ROOT, text=True).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--suite", choices=["spans", "cache"], default="spans")
    parser.add_argument("--case", action="append", dest="cases", help="Run only named cases; repeat to select several")
    args = parser.parse_args()
    cache = args.suite == "cache"
    iterations_by_case = CACHE_ITERATIONS if cache else ITERATIONS
    if args.cases:
        if not set(args.cases) <= iterations_by_case.keys():
            parser.error("unknown case for selected suite")
        iterations_by_case = {case: n for case, n in iterations_by_case.items() if case in args.cases}
    variants = ["full", "tile64", "tile128", "tile256"] if cache else ["baseline", "production"]
    prefix = "cpu-cache" if cache else "cpu-spans"
    relevance = ("Exact regional tile and ordered lower-prefix caching versus full regional rendering; admitted transactions and recorded history included." if cache else
                 "CPU full-composite borrowed sparse rows versus frozen scalar row-copy baseline; no cache/dirty-region qualification.")
    destination = args.output.resolve()
    destination.mkdir(parents=True, exist_ok=False)
    binary = ROOT / ("target/release/examples/cache_replay" if cache else "target/release/examples/composite")
    files = [
        (binary, "binary"), (ROOT / "Cargo.lock", "lockfile"),
        (ROOT / "crates/services/paintcraft-render/examples/support/mod.rs", "generator"),
        (ROOT / "crates/services/paintcraft-render/examples/support/reference.rs", "driver"),
        (ROOT / "crates/services/paintcraft-render/examples/composite.rs", "driver"),
        (ROOT / "crates/services/paintcraft-render/src/lib.rs", "other"),
        (ROOT / "crates/foundation/paintcraft-raster/src/raster.rs", "other"),
        (ROOT / "crates/foundation/paintcraft-raster/src/raster/rows.rs", "other"),
        (ROOT / "crates/foundation/paintcraft-core/src/types/pixel.rs", "other"),
    ]
    if cache:
        render = ROOT / "crates/services/paintcraft-render"
        files.extend((path, "driver") for path in [render / "examples/cache_replay.rs", render / "examples/support/replay.rs"])
        files.extend((path, "other") for path in [render / "src/cache.rs", *sorted((render / "src/cache").glob("*.rs")),
                     ROOT / "crates/foundation/paintcraft-raster/src/raster/versions.rs"])
    artifacts = [{"path": str(path), "sha256": sha256(path), "role": role} for path, role in files]
    host = {
        "platform": platform.platform(), "logical_cpus": os.cpu_count(),
        "rustc": output(["rustc", "-Vv"]), "hyperfine": output(["hyperfine", "--version"]),
        "source_revision": output(["git", "rev-parse", "HEAD"]),
        "worktree_status": output(["git", "status", "--porcelain"]),
        "limitations": "Single developer machine; no controlled power/thermal state, CPU affinity or cross-ISA qualification. RSS is process-wide, not decoded-tile residency.",
    }
    (destination / "host.json").write_text(json.dumps(host, indent=2) + "\n")
    configs = {}
    for case, iterations in iterations_by_case.items():
        config = {
            "schema": "paintcraft.hyperfine-run/1", "mode": "application",
            "case_id": "BF29", "task_id": "T12",
            "relevance": relevance,
            "cwd": str(ROOT),
            "workload": {"id": f"{prefix}-{case}", "mode": "batch", "work_units": iterations,
                         "seed": 42, "cache_state": "new process, warm OS inputs; fixture built once per process",
                         "completion": "all admitted edits, exact renders and output drops finished; fixture setup/first render/history/drop included" if cache else "all exact full renders and output drops finished; fixture setup/drop included"},
            "build": {"source_revision": host["source_revision"] + " plus contender files identified by artifact hashes",
                      "profile": "release", "target": host["rustc"].split("host: ", 1)[1].splitlines()[0], "compiler": output(["rustc", "-V"]),
                      "flags": "Cargo release defaults, opt-level 3, debug line tables, CARGO_INCREMENTAL=0",
                      "environment_notes": host["limitations"]},
            "runs": 20, "warmup": 3, "timeout_seconds": 600, "env": {},
            "artifacts": artifacts, "correctness_commands": [[str(binary), "verify"]],
            "variants": [{"id": variant, "argv": [str(binary), variant, case, str(iterations)]}
                         for variant in variants],
        }
        validate_config(config)
        path = destination / f"{case}-config.json"
        path.write_text(json.dumps(config, indent=2) + "\n")
        configs[case] = (config, path)
    if not args.execute:
        print(f"Prepared configs in {destination}; no benchmark executed")
        return
    resource_flags = ["-l"] if platform.system() == "Darwin" else ["-v"]
    for case in iterations_by_case:
        with (destination / f"calibration-{case}.txt").open("w") as log:
            subprocess.run(["/usr/bin/time", *resource_flags, str(binary), variants[-1], case, "0"],
                           cwd=ROOT, stdout=log, stderr=log, check=True, timeout=600)
    for session, reverse in [(1, False), (2, True), (3, False)]:
        for case, (config, path) in configs.items():
            run = destination / f"session-{session}-{case}"
            execute(config, path, run, "hyperfine", reverse)
            # Native process RSS/cpu observations are supplementary, not a kernel timer.
            for variant in variants:
                with (run / f"resources-{variant}.txt").open("w") as log:
                    subprocess.run(["/usr/bin/time", *resource_flags, str(binary), variant, case, str(iterations_by_case[case])],
                                   cwd=ROOT, stdout=log, stderr=log, check=True, timeout=600)
            # Resource logs were added after the runner's immutable timing manifest.
            with (run / "RESOURCES.sha256").open("w") as manifest:
                for log in sorted(run.glob("resources-*.txt")):
                    manifest.write(f"{sha256(log)}  {log.name}\n")
            print(f"Completed session {session}: {case}", flush=True)


if __name__ == "__main__":
    main()
