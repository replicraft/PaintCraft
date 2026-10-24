#!/usr/bin/env python3
"""Build the standalone browser editor with the locked wasm-bindgen ABI."""
import json
import pathlib
import shutil
import subprocess

APP = pathlib.Path(__file__).resolve().parent
ROOT = APP.parent.parent
VERSION = "0.2.129"
if shutil.which("wasm-bindgen") is None:
    raise SystemExit(f"Install the browser binding tool: cargo install wasm-bindgen-cli --version {VERSION} --locked")
actual = subprocess.check_output(["wasm-bindgen", "--version"], text=True).strip()
if actual != f"wasm-bindgen {VERSION}":
    raise SystemExit(f"Expected wasm-bindgen {VERSION}, found {actual}")
subprocess.run(["cargo", "build", "--locked", "--release", "--target", "wasm32-unknown-unknown", "-p", "paintcraft-web"], cwd=ROOT, check=True)
# Ask Cargo for the target directory, including CARGO_TARGET_DIR overrides.
metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=ROOT))
wasm = pathlib.Path(metadata["target_directory"]) / "wasm32-unknown-unknown/release/paintcraft_web.wasm"
dist = APP / "dist"
dist.mkdir(exist_ok=True)
subprocess.run(["wasm-bindgen", str(wasm), "--target", "web", "--out-dir", str(dist)], check=True)
shutil.copyfile(APP / "index.html", dist / "index.html")
shutil.copytree(APP / "public", dist, dirs_exist_ok=True)
print(f"Serve with: python3 -m http.server 8080 --directory {dist}")
