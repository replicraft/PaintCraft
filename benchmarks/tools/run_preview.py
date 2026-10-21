"""Measure production preview snapshots versus frozen collect/compare behavior."""
import argparse
import json
import os
from pathlib import Path
import platform
import subprocess

from benchmark_support import sha256, validate_config
from run_hyperfine import execute

ROOT = Path(__file__).resolve().parents[2]
TEST = "controller::preview_cache::benchmark::preview_capture_process"
CASES = {"short": 100_000, "long": 512, "queue": 128, "changing": 512}


def output(argv):
    return subprocess.check_output(argv, cwd=ROOT, text=True).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True, help="Prebuilt release paintcraft library test executable")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--execute", action="store_true")
    args = parser.parse_args()
    binary = args.binary.resolve()
    destination = args.output.resolve()
    destination.mkdir(parents=True, exist_ok=False)
    source = ROOT / "apps/paintcraft/src"
    files = [(binary, "binary"), (Path("/usr/bin/env"), "binary"),
             (ROOT / "Cargo.lock", "lockfile"),
             (source / "controller/preview_cache/benchmark.rs", "generator"),
             (Path(__file__).resolve(), "driver")]
    files.extend((source / path, "other") for path in [
        "controller.rs", "controller/preview_cache.rs", "controller/paint_queue.rs",
        "controller/background.rs", "app/rendering.rs"])
    files.extend((ROOT / "crates/editing/paintcraft-tools/src" / path, "other")
                 for path in ["lib.rs", "path_admission.rs", "preview_revision.rs"])
    artifacts = [{"path": str(path), "sha256": sha256(path), "role": role} for path, role in files]
    host = {"platform": platform.platform(), "logical_cpus": os.cpu_count(),
            "rustc": output(["rustc", "-Vv"]), "hyperfine": output(["hyperfine", "--version"]),
            "source_revision": output(["git", "rev-parse", "HEAD"]),
            "worktree_status": output(["git", "status", "--porcelain"]),
            "limitations": "One developer machine, uncontrolled power/thermal state and affinity; process RSS includes fixture and native pool; no input/frame/GPU latency qualification."}
    (destination / "host.json").write_text(json.dumps(host, indent=2) + "\n")

    def command(case, variant):
        return ["/usr/bin/env", f"PAINTCRAFT_PREVIEW_CASE={case}",
                f"PAINTCRAFT_PREVIEW_VARIANT={variant}", str(binary), "--ignored", "--exact", TEST, "--nocapture"]

    configs = {}
    for case, repeats in CASES.items():
        config = {
            "schema": "paintcraft.hyperfine-run/1", "mode": "application",
            "case_id": "BF29", "task_id": "T12",
            "relevance": "Derived preview snapshot reuse versus repeated collect/compare; native source functions without window rendering. No compositor throughput or frame latency claim.",
            "cwd": str(ROOT),
            "workload": {"id": f"preview-{case}", "mode": "batch", "work_units": repeats,
                         "seed": 0, "cache_state": "fresh process; deterministic path fixture plus one initial snapshot; no raster or GPU execution",
                         "completion": "all capture/comparison iterations plus snapshot/gesture/editor/pool destruction completed; setup and test harness included"},
            "build": {"source_revision": host["source_revision"] + " with artifact hashes",
                      "profile": "release", "target": host["rustc"].split("host: ", 1)[1].splitlines()[0],
                      "compiler": output(["rustc", "-V"]),
                      "flags": "Cargo release defaults, opt-level 3, CARGO_INCREMENTAL=0; library test binary",
                      "environment_notes": host["limitations"]},
            "runs": 20, "warmup": 3, "timeout_seconds": 120, "env": {}, "artifacts": artifacts,
            "correctness_commands": [command(case, "verify")],
            "variants": [{"id": variant, "argv": command(case, variant)} for variant in ["baseline", "cached"]],
        }
        validate_config(config)
        path = destination / f"{case}-config.json"
        path.write_text(json.dumps(config, indent=2) + "\n")
        configs[case] = (config, path)
    if not args.execute:
        print(f"Prepared {destination}; no measurements executed")
        return
    resource_flags = ["-l"] if platform.system() == "Darwin" else ["-v"]
    for session, reverse in [(1, False), (2, True), (3, False)]:
        for case, (config, path) in configs.items():
            run = destination / f"session-{session}-{case}"
            execute(config, path, run, "hyperfine", reverse)
            for variant in ["baseline", "cached"]:
                with (run / f"resources-{variant}.txt").open("w") as log:
                    subprocess.run(["/usr/bin/time", *resource_flags, *command(case, variant)],
                                   cwd=ROOT, stdout=log, stderr=log, check=True, timeout=120)
            with (run / "RESOURCES.sha256").open("w") as manifest:
                for log in sorted(run.glob("resources-*.txt")):
                    manifest.write(f"{sha256(log)}  {log.name}\n")
            print(f"Completed session {session}: {case}", flush=True)


if __name__ == "__main__":
    main()
