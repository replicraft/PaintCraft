#!/usr/bin/env python3
"""Measure prebuilt native viewport texture readiness; never builds in timed runs."""
import argparse
import datetime
import gzip
import hashlib
import json
import os
import platform
import shlex
import shutil
import subprocess
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--suite", choices=["viewport", "output-pool", "multires", "gpu-scaled", "gpu-regions", "gpu-batches", "single-layer"], default="viewport")
parser.add_argument("--binary", required=True, type=Path)
parser.add_argument("--output", required=True, type=Path)
args = parser.parse_args()
root = Path.cwd()
binary = args.binary.resolve()
out = args.output.resolve()
out.mkdir(parents=True, exist_ok=False)
single_suite = args.suite == "single-layer"
pool_suite = args.suite == "output-pool"
batch_suite = args.suite == "gpu-batches"
region_suite = args.suite == "gpu-regions"
gpu_scaled_suite = args.suite in ["gpu-scaled", "gpu-regions", "gpu-batches"]
multires_suite = args.suite in ["multires", "gpu-scaled", "gpu-regions", "gpu-batches"]
if single_suite:
    test = "gpu_display::single_benchmark::native_single_layer_process"
    cases = ["tiny", "dense", "large", "opacity", "multiple", "partial"]
    modes = ["ordered", "single"]
    verify_variant = "verify"
    criterion_variant = "criterion"
    criterion_prefix = "native_single_layer"
elif multires_suite:
    test = "cpu_rendering::display_benchmark::native_multires_process"
    cases = ["tiny", "dense", "layers20", "layers100", "large", "uniform"] if batch_suite else ["tiny", "dense", "large", "uniform"] if gpu_scaled_suite else ["tiny", "dense", "large"]
    modes = ["gpu64", "gpu64batch", "gpu64regions", "gpu64regionsbatch"] if batch_suite else ["gpu32", "gpu32regions", "gpu64", "gpu64regions"] if region_suite else ["cpu", "gpu32", "gpu64"] if gpu_scaled_suite else ["full", "bounded", "cached"]
    verify_variant = "batches-verify" if batch_suite else "regions-verify" if region_suite else "gpu-verify" if gpu_scaled_suite else "verify"
    criterion_variant = "batches-criterion" if batch_suite else "regions-criterion" if region_suite else "gpu-criterion" if gpu_scaled_suite else "criterion"
    criterion_prefix = "native_gpu_batches" if batch_suite else "native_gpu_regions" if region_suite else "native_gpu_scaled" if gpu_scaled_suite else "native_multires"
else:
    test = "gpu_display::region_benchmark::native_viewport_process"
    cases = ["quarter", "large", "resize"] if pool_suite else ["all", "quarter", "pan"]
    modes = ["unpooled", "pooled"] if pool_suite else ["full", "region"]
    verify_variant = "pool-verify" if pool_suite else "verify"
    criterion_variant = "pool-criterion" if pool_suite else "criterion"
    criterion_prefix = "native_output_pool" if pool_suite else "native_viewport"


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


sources = sorted({
    Path("Cargo.toml"), Path("Cargo.lock"), Path("rust-toolchain.toml"),
    Path(__file__).resolve().relative_to(root),
    *Path("crates").rglob("*.rs"), *Path("crates").rglob("*.wgsl"),
    *Path("crates").rglob("Cargo.toml"), *Path("apps").rglob("*.rs"),
    *Path("apps").rglob("Cargo.toml"),
})
listing = subprocess.check_output([str(binary), "--list"], text=True)
assert f"{test}: test" in listing.splitlines(), "exact benchmark test missing from executable"
manifest = {
    "exact_test": test,
    "baseline": "one completed GPU submission per layer versus bounded ordered batches; both source policies" if batch_suite else "existing single-view-per-layer GPU source retention; same exact scaled renderer" if region_suite else "actual cached CPU multiresolution path and completed managed upload" if gpu_scaled_suite else "existing native-resolution CpuRenderer/cache followed by independent box filter" if multires_suite else None,
    "utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "platform": platform.platform(), "processor": platform.processor(),
    "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
    "binary": str(binary), "binary_sha256": sha(binary),
    "sources_sha256": {str(p): sha(p) for p in sources},
    "rustc": subprocess.check_output(["rustc", "-Vv"], text=True),
    "hyperfine": subprocess.check_output(["hyperfine", "--version"], text=True).strip(),
    "build": "CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 cargo test -p paintcraft --lib --release --no-run --message-format=json",
    "suite": args.suite, "cases": cases, "variants": modes,
    "canvas": {"tiny": [32,32], "dense": [512,256], "large": [4096,2160], "uniform": [4096,2160], "layers20": [512,256], "layers100": [512,256]} if multires_suite else {"quarter": [512, 256], "large": [4096, 2160], "resize": [4096, 2160]} if pool_suite else [512, 256],
    "base": "dense 5/20/100 layers for dense/layers20/layers100; sparse implicit white for tiny/large/uniform" if batch_suite else "dense five layers for dense; sparse implicit white for tiny/large" if multires_suite else "dense5 for quarter; single sparse implicit white layer for large/resize" if pool_suite else "dense5", "frames": 8 if multires_suite else 16, "new_points_per_frame": 8,
    "warmups": 3, "runs": 20, "sessions": 3,
    "submission_window": {"layers":16,"source_bytes":1048576,"pending_key_threshold":1048576,"additional_admission_bytes":4194304} if batch_suite else None,
    "layers": {"tiny":1,"dense":5,"layers20":20,"layers100":100,"large":1,"uniform":1} if batch_suite else None,
    "sampling_levels": {"tiny":1,"dense":1,"large":2,"uniform":2,"layers20":1,"layers100":1} if multires_suite else None,
    "uniform_workload": "Eight real typed opacity edits (128..135), no raster changes; other cases grow eight-point brush input each frame." if gpu_scaled_suite else None,
    "scope": "Exact completed native scaled displays with serial versus bounded (16 layers, 1 MiB source window, 1 MiB pending-key threshold) ordered submission, with single-view and regional source retention. Includes 1/5/20/100-layer inputs, cold setup and teardown; not asynchronous completion, OS latency or physical admission." if batch_suite else "Completed native GPU display with single-view versus bounded multiple-region source retention at both output-tile edges. Same exact pixel requirements, setup, teardown and GPU completion waits. Not asynchronous transfer, an atlas, physical GPU budgeting or OS latency." if region_suite else "Cached exact CPU scaled display/managed upload versus GPU native-block/wide-sum display/native registration at 32 and 64 output-tile edges. Completed GPU work and cold setup/teardown included; independent full-native/filter verification; no OS latency or global physical-budget qualification." if gpu_scaled_suite else "Full-native-ROI composite/box-filter baseline versus bounded and cached exact CPU display production, native App ColorImage acceptance and completed egui managed-texture upload. Cold device/renderer/font/document setup and teardown included. Not GPU scaled composition, OS presentation or event latency." if multires_suite else "Completed compositor-buffer pool versus fresh allocation" if pool_suite else "Actual desktop preview staging, completed GPU composition/display conversion and App native texture acceptance. Required visible pixels equal full output cropped to requested bounds. Cold fixture/device/renderer construction and teardown included. Not OS presentation or event latency.",
    "environment": "Local macOS arm64 developer host; adapter/limits in correctness logs. No affinity, power, thermal or background-load controls. No intentionally concurrent builds, tests or other measurements. Process startup calibration retained, never subtracted.",
}
if single_suite:
    manifest.update(
        baseline="existing completed native ordered compositor, then display conversion",
        canvas={"tiny":[32,32], "dense":[512,256], "large":[4096,2160],
                "opacity":[512,256], "multiple":[512,256], "partial":[512,256]},
        region="full canvas except large, which requests a 1024x768 native viewport",
        base="one dense layer for dense/partial; five dense layers for multiple; sparse white otherwise",
        frames=8, layers={"tiny":1,"dense":1,"large":1,"opacity":1,"multiple":5,"partial":1},
        uniform_workload="opacity performs eight real layer-opacity commands; other cases stage growing brush previews",
        scope="Completed native-resolution textures and actual App registration. Direct source-to-display versus the existing intermediate compositor; both use identical source retention. Cold device/pipeline/fixture setup and teardown included. No OS input/display latency or physical GPU budget claim.",
        memory="Untimed /usr/bin/time -l runs per case/variant; RSS is process-wide, not opaque GPU allocation accounting.",
    )
(out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")


def env_for(case, variant):
    env = os.environ.copy()
    env.update(PAINTCRAFT_GEOMETRY_BASE="dense5", PAINTCRAFT_VIEWPORT_CASE=case,
               PAINTCRAFT_VIEWPORT_VARIANT=variant, PAINTCRAFT_REQUIRE_GPU="1")
    return env


def verify(case, label):
    with (out / f"correctness-{label}-{case}.log").open("w") as log:
        subprocess.run([str(binary), "--exact", test, "--ignored", "--nocapture"],
                       env=env_for(case, verify_variant), stdout=log, stderr=subprocess.STDOUT,
                       check=True)
    result = (out / f"correctness-{label}-{case}.log").read_text()
    assert "running 1 test" in result and "1 passed; 0 failed;" in result, "oracle test did not execute exactly once"


def identities():
    assert sha(binary) == manifest["binary_sha256"], "binary changed during measurement"
    assert {str(p): sha(p) for p in sources} == manifest["sources_sha256"], "source changed"


for case in cases:
    verify(case, "before")
    print("preflight", case, "complete", flush=True)
if single_suite:
    for case in cases:
        for variant in modes:
            with (out / f"memory-{case}-{variant}.log").open("w") as log:
                subprocess.run(["/usr/bin/time", "-l", str(binary), "--exact", test,
                                "--ignored", "--nocapture"], env=env_for(case, variant),
                               stdout=log, stderr=subprocess.STDOUT, check=True)

for case in cases:
    with (out / f"criterion-{case}.log").open("w") as log:
        subprocess.run([str(binary), "--exact", test, "--ignored", "--nocapture"],
                       env=env_for(case, criterion_variant), stdout=log,
                       stderr=subprocess.STDOUT, check=True)
    print("Criterion", case, "complete", flush=True)
for path in (root / "target/criterion").rglob("*.json"):
    if any(p.startswith(criterion_prefix) for p in path.parts) and path.parent.name == "new":
        dest = out / "criterion" / path.relative_to(root / "target/criterion")
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(path, dest)


def process(session, case, variants):
    commands = [shlex.join([
        "/usr/bin/env", "PAINTCRAFT_GEOMETRY_BASE=dense5", "PAINTCRAFT_REQUIRE_GPU=1",
        f"PAINTCRAFT_VIEWPORT_CASE={case}", f"PAINTCRAFT_VIEWPORT_VARIANT={mode}",
        str(binary), "--exact", test, "--ignored",
    ]) for mode in variants]
    export = out / f"hyperfine-{session}-{case}.json"
    with export.with_suffix(".log").open("w") as log:
        subprocess.run(["hyperfine", "--shell=none", "--warmup", "3", "--runs", "20",
                        "--export-json", str(export), *commands], stdout=log,
                       stderr=subprocess.STDOUT, check=True)
    # Keep exact original bytes and a compact equal-valued copy under file limits.
    raw = export.read_bytes()
    compact = (json.dumps(json.loads(raw), separators=(",", ":")) + "\n").encode()
    assert json.loads(raw) == json.loads(compact)
    export.with_suffix(".json.gz").write_bytes(gzip.compress(raw, mtime=0))
    export.write_bytes(compact)
    export.with_suffix(".identity.json").write_text(json.dumps({
        "original_sha256": hashlib.sha256(raw).hexdigest(),
        "compact_sha256": hashlib.sha256(compact).hexdigest(),
        "equal_json_values": True,
    }, indent=2) + "\n")


for session in range(1, 4):
    for case in cases:
        process(session, case, modes if session != 2 else list(reversed(modes)))
        print("session", session, case, "complete", flush=True)
    process(session, "calibration", ["calibration"])
    for case in cases:
        verify(case, f"after-{session}")
    identities()
identities()
print("All raw samples, correctness postflights and identities verified", flush=True)
