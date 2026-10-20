"""Retain Criterion renderer/history samples in explicitly ordered, bounded invocations."""
import argparse
import csv
import math
import json
import os
from pathlib import Path

from benchmark_support import sha256
from run_composite import PARALLEL_ITERATIONS, ROOT
from run_hyperfine import run_logged, write_json


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--suite", choices=["partitions", "history"], default="partitions")
    parser.add_argument("--configs", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--order", choices=["forward", "reverse"], default="reverse")
    parser.add_argument("--execute", action="store_true")
    args = parser.parse_args()
    binary = args.binary.resolve()
    history = args.suite == "history"
    if history:
        from run_history import CASES
        cases = list(CASES)
    else:
        cases = list(PARALLEL_ITERATIONS)
    config_path = args.configs / ("sparse256-config.json" if history else "tiny-config.json")
    prototype = json.loads(config_path.read_text())
    assert prototype["case_id"] == ("BF18" if history else "BF06")
    group = "recorded-history" if history else "partitions"
    samples = 20 if history else 100
    variants = [row["id"] for row in prototype["variants"]]
    if args.order == "reverse":
        cases.reverse()
        variants.reverse()
    commands = [(case, variant, [str(binary), "--bench", f"{group}/{case}/{variant}", "--exact"])
                for case in cases for variant in variants]
    if not args.execute:
        print(json.dumps({"mode": "plan-only", "commands": [row[2] for row in commands]}, indent=2))
        return
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    env = os.environ.copy()
    env["CRITERION_HOME"] = str(output / "samples")
    before = sha256(binary)
    metadata = {"order": args.order, "binary": str(binary), "binary_sha256": before,
                "config_sha256": sha256(config_path),
                "commands": [row[2] for row in commands], "completed": 0,
                "boundary": "exact-filtered Criterion measurements; full lifecycle including recorded correctness validation and destruction" if history else "exact-filtered Criterion measurements; all scalar preflight/setup outside measurement"}
    write_json(output / "execution.json", metadata)
    try:
        for case, variant, command in commands:
            code = run_logged(command, ROOT, env, 120, output / f"{case}-{variant}.log")
            if code:
                raise RuntimeError(f"Criterion failed: {case}/{variant}: {code}")
            sample = output / f"samples/{group}" / case / variant / "new/sample.json"
            data = json.loads(sample.read_text())
            assert len(data["times"]) == len(data["iters"]) == samples
            assert all(math.isfinite(t) and t > 0 for t in data["times"])
            assert all(n > 0 for n in data["iters"])
            with sample.with_name("raw.csv").open() as raw:
                assert len(list(csv.DictReader(raw))) == samples
            metadata["completed"] += 1
            write_json(output / "execution.json", metadata)
            print(f"Completed {case}/{variant}", flush=True)
        assert sha256(binary) == before
        metadata["execution_status"] = "completed"
    except BaseException as error:
        metadata["execution_status"] = "failed"
        metadata["error"] = str(error)
        raise
    finally:
        write_json(output / "execution.json", metadata)
        lines = [f"{sha256(path)}  {path.relative_to(output).as_posix()}"
                 for path in sorted(output.rglob("*"))
                 if path.is_file() and path.name != "MANIFEST.sha256"]
        (output / "MANIFEST.sha256").write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
