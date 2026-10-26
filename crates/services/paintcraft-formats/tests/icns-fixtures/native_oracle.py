"""Native iconutil qualification. Writes only an explicit fresh temporary dir."""
import hashlib
import importlib.metadata
import json
import platform
import subprocess
import sys
from pathlib import Path
from PIL import Image
ROOT = Path(__file__).parent
assert importlib.metadata.version("Pillow") == "13.0.0"
if len(sys.argv) != 3:
    raise SystemExit("Usage: native_oracle.py <fresh Rust output directory> <new temporary report directory>")
written = Path(sys.argv[1])
out = Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=False)
manifest = json.loads((ROOT / "sources.json").read_text())
report = {"architecture": platform.machine(), "qualification_date": "2026-10-25", "macos": subprocess.check_output(["sw_vers", "-productVersion"], text=True).strip(),
          "tool": "/usr/bin/iconutil", "tool_sha256": hashlib.sha256(Path("/usr/bin/iconutil").read_bytes()).hexdigest(),
          "pixel_reader": "Pillow 13.0.0", "cases": []}
for name in ["isolated-icp4.icns", "isolated-icp5.icns", "isolated-icp6.icns", "modern.icns"]:
    fresh = name == "modern.icns"
    source = (written if fresh else ROOT) / name
    target = out / f"{source.stem}.iconset"
    result = subprocess.run(["/usr/bin/iconutil", "-c", "iconset", str(source), "-o", str(target)], capture_output=True, text=True)
    expected = manifest["files"]["modern.icns" if fresh else name]["entries"]
    if fresh:
        expected = [item for item in expected if item["tag"] not in {"icp4", "icp5", "icp6"}]
    case = {"source": ("fresh/" if fresh else "fixtures/") + source.name, "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
            "exit": result.returncode, "stderr": result.stderr, "fresh_writer": fresh,
            "exported_files": sorted(path.name for path in target.glob("*")), "entries": []}
    case["exported_images"] = []
    for file in sorted(target.glob("*.png")):
        with Image.open(file) as image:
            case["exported_images"].append({"file": file.name, "size": list(image.size), "rgba_sha256": hashlib.sha256(image.convert("RGBA").tobytes()).hexdigest()})
    for item in expected:
        filename = f"icon_{item['logical']}x{item['logical']}" + ("@2x" if item["scale"] == 2 else "") + ".png"
        entry = {"type": item["tag"], "logical": item["logical"], "scale": item["scale"], "expected_file": filename,
                 "expected_rgba_sha256": item["rgba_sha256"]}
        file = target / filename
        if not file.exists():
            entry["status"] = "requested logical size/density not exported"
        else:
            with Image.open(file) as image:
                actual = image.convert("RGBA").tobytes()
                entry["actual_size"] = list(image.size)
                entry["actual_rgba_sha256"] = hashlib.sha256(actual).hexdigest()
                entry["status"] = "exact" if entry["actual_rgba_sha256"] == item["rgba_sha256"] else "different"
                if not fresh:
                    sys.dont_write_bytecode = True
                    from generate import pixels
                    original = pixels(item["side"], item["seed"])
                    if len(original) == len(actual):
                        differences = [i for i in range(0, len(original), 4) if original[i:i+4] != actual[i:i+4]]
                        entry["changed_pixels"] = len(differences)
                        entry["changed_alpha_pixels"] = sum(original[i+3] != actual[i+3] for i in differences)
                        entry["first_expected_rgba"] = list(original[:4])
                        entry["first_actual_rgba"] = list(actual[:4])
        case["entries"].append(entry)
        if fresh:
            assert entry["status"] == "exact", entry
    if fresh:
        assert result.returncode == 0
    report["cases"].append(case)
(out / "report.json").write_text(json.dumps(report, indent=4) + "\n")
print(json.dumps(report, indent=4))
