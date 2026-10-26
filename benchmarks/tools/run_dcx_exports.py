#!/usr/bin/env python3
"""Actual DCX CLI delivery baseline; no optimization comparison or OS cache flush."""
import argparse
from datetime import datetime, timezone
import csv
import json
import os
from pathlib import Path
import platform
import shlex
import shutil
import subprocess
import sys
import time

from benchmark_support import read_json, sha256
from run_palette_assets import build_receipt

ROOT = Path(__file__).resolve().parents[2]
CASES = {
    "64-palette": "indexed-exact",
    "1000x486-sparse-opaque": "rgb24",
    "1000x486-dense-opaque": "rgb24",
    "1000x486-palette": "indexed-exact",
}


def command(manifest, case):
    return [manifest["cli"], "export-dcx", case["project"], "--frontend", "studio",
            "--pcx-mode", case["mode"], "--expect-input-sha256", case["project_sha256"],
            "--output", case["output"], "--json"]


def one(manifest_path, case_id, timeout=30):
    manifest = read_json(manifest_path)
    assert manifest["schema"] == "paintcraft.dcx-delivery/1"
    case = next(row for row in manifest["cases"] if row["id"] == case_id)
    output = Path(case["output"])
    assert output.parent.resolve() == manifest_path.parent.resolve() / "work"
    assert output.suffix == ".dcx" and not output.exists()
    try:
        result = subprocess.run(command(manifest, case), capture_output=True, text=True,
                                timeout=timeout, check=True)
        receipt = json.loads(result.stdout)
        assert receipt["outcome"] == "success", receipt
        assert output.is_file() and output.stat().st_size > 4100
    finally:
        # Each run owns one initially absent file in the newly created private
        # benchmark directory. Destruction is inside the timed subprocess scope.
        output.unlink(missing_ok=True)


def logged(argv, path):
    with path.open("x") as log:
        subprocess.run([str(v) for v in argv], cwd=ROOT, stdout=log,
                       stderr=subprocess.STDOUT, check=True, timeout=300)


def run(args):
    source_head = subprocess.check_output(
        ["git", "rev-parse", "--verify", args.build_source_head + "^{commit}"],
        cwd=ROOT, text=True,
    ).strip()
    profile = build_receipt(args.build_jsonl, args.cli)
    plan = {"cases": CASES, "runs": args.runs, "warmups": 2,
            "scope": "Python wrapper startup + CLI startup/read/admission/composite/encode/durable publication/JSON + output unlink",
            "cache": "first-in-session observation after correctness preflight; then repeated fresh CLI processes with warm filesystem cache; no OS cache flush",
            "profile": profile}
    if not args.execute:
        print(json.dumps(plan, indent=4))
        return
    assert args.agent_builds_idle, "coordinate all agents before measurements"
    assert args.runs >= 20
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    for directory in ["bin", "fixtures", "projects", "work", "oracle"]:
        (output / directory).mkdir()
    cli = output / "bin/paintcraft-cli"
    shutil.copy2(args.cli, cli)
    cli.chmod(0o555)
    shutil.copy2(args.build_jsonl, output / "build.jsonl")
    rows = {row["case"]: row for row in csv.DictReader((args.fixtures / "fixtures.csv").open())
            if row["format"] == "PNG"}
    manifest = {"schema": "paintcraft.dcx-delivery/1", "cli": str(cli), "cases": []}
    metadata = {**plan, "utc": datetime.now(timezone.utc).isoformat(),
                "head_before": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
                "status_before": subprocess.check_output(["git", "status", "--short"], cwd=ROOT, text=True),
                "platform": platform.platform(), "machine": platform.machine(),
                "python": sys.version, "rustc": subprocess.check_output(["rustc", "-Vv"], text=True),
                "hyperfine": subprocess.check_output(["hyperfine", "--version"], text=True).strip(),
                "environment_limit": "Agent builds idle by explicit coordination; external user load, thermal/power state and background services uncontrolled.",
                "cancellation_limit": "No signal latency measured. DCX RPC cancellation is separately correctness-tested; each timed CLI has a 30s process timeout, not a product deadline.",
                "build_source_head": source_head,
                "build_source_diff_to_run_head": subprocess.check_output(["git", "diff", "--stat", source_head, "HEAD"], cwd=ROOT, text=True),
                "sha256": {"cli": sha256(cli), "lock": sha256(ROOT / "Cargo.lock"),
                           "driver": sha256(Path(__file__)), "build_jsonl": sha256(args.build_jsonl)}}
    for name, mode in CASES.items():
        row = rows[name]
        png = args.fixtures / f"{name}.png"
        assert sha256(png) == row["encoded_sha256"]
        source = output / "fixtures" / png.name
        shutil.copy2(png, source)
        project = output / "projects" / f"{name}.pcnd"
        logged([cli, "import", source, "--frontend", "studio", "-o", project, "--json"],
               output / f"import-{name}.json")
        inspected = subprocess.check_output([str(cli), "inspect", str(project), "--json"], text=True)
        (output / f"inspect-{name}.json").write_text(inspected)
        inspection = json.loads(inspected)["document"]
        case = {"id": name, "mode": mode, "project": str(project),
                "project_sha256": sha256(project), "state_id": inspection["state_id"],
                "source_png": str(source), "source_png_sha256": sha256(source),
                "source_rgba_sha256": row["source_sha256"],
                "width": int(row["width"]), "height": int(row["height"]),
                "output": str(output / "work" / f"{name}.dcx"),
                "oracle_output": str(output / "oracle" / f"{name}.dcx")}
        manifest["cases"].append(case)
        argv = command(manifest, {**case, "output": case["oracle_output"]})
        logged(argv, output / f"export-{name}.json")
        case["oracle_sha256"] = sha256(Path(case["oracle_output"]))
    manifest_path = output / "manifest.json"
    manifest_path.write_text(json.dumps(manifest, indent=4) + "\n")
    oracle = ROOT / "apps/paintcraft-cli/tests/dcx-consumer-fixtures/verify_benchmark.mjs"
    oracle_argv = ["node", str(oracle), str(manifest_path), str(args.oracle_package.resolve())]
    logged(oracle_argv, output / "independent-preflight.json")
    (output / "environment.json").write_text(json.dumps(metadata, indent=4) + "\n")
    sample_argv = lambda name: [sys.executable, str(Path(__file__).resolve()), "one", str(manifest_path), name]
    first = []
    for name in CASES:
        start = time.perf_counter_ns()
        subprocess.run(sample_argv(name), check=True, timeout=35)
        first.append({"case": name, "nanoseconds": time.perf_counter_ns() - start})
    (output / "first-in-session.json").write_text(json.dumps(first, indent=4) + "\n")
    argv = ["hyperfine", "--shell=none", "--runs", str(args.runs), "--warmup", "2",
            "--export-json", str(output / "hyperfine.json")]
    for name in CASES:
        argv.extend(["--command-name", name, shlex.join(sample_argv(name))])
    logged(argv, output / "hyperfine.log")
    raw = json.loads((output / "hyperfine.json").read_text())["results"]
    assert len(raw) == len(CASES)
    for row in raw:
        if "measurements" in row:
            assert len(row["measurements"]) == args.runs
            assert all(v["exit_code"] == 0 for v in row["measurements"])
        else:
            assert len(row["times"]) == args.runs
            assert all(code == 0 for code in row["exit_codes"])
    assert not list((output / "work").iterdir()), "timed output destruction incomplete"
    assert sha256(cli) == metadata["sha256"]["cli"]
    assert sha256(ROOT / "Cargo.lock") == metadata["sha256"]["lock"]
    for case in manifest["cases"]:
        assert sha256(Path(case["project"])) == case["project_sha256"]
        argv = command(manifest, {**case, "output": case["oracle_output"]})
        Path(case["oracle_output"]).unlink()
        logged(argv, output / f"postflight-{case['id']}.json")
        assert sha256(Path(case["oracle_output"])) == case["oracle_sha256"]
    logged(oracle_argv, output / "independent-postflight.json")
    metadata["head_after"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    metadata["status_after"] = subprocess.check_output(["git", "status", "--short"], cwd=ROOT, text=True)
    metadata["completed"] = True
    (output / "environment.json").write_text(json.dumps(metadata, indent=4) + "\n")
    print(output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="operation", required=True)
    sample = commands.add_parser("one")
    sample.add_argument("manifest", type=Path)
    sample.add_argument("case")
    benchmark = commands.add_parser("run")
    for option in ["cli", "build-jsonl", "fixtures", "oracle-package", "output"]:
        benchmark.add_argument("--" + option, type=Path, required=True)
    benchmark.add_argument("--build-source-head", required=True)
    benchmark.add_argument("--runs", type=int, default=20)
    benchmark.add_argument("--execute", action="store_true")
    benchmark.add_argument("--agent-builds-idle", action="store_true")
    args = parser.parse_args()
    if args.operation == "one":
        one(args.manifest, args.case)
    else:
        run(args)


if __name__ == "__main__":
    main()
