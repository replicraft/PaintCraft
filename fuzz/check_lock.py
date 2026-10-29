#!/usr/bin/env python3
"""Ensure the isolated parser harness uses the product's locked source versions."""
from pathlib import Path
import json
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parent
DEV_ONLY = {"paintcraft-container-fuzz", "libfuzzer-sys", "arbitrary"}


def identity(item):
    return item["name"], item["version"], item.get("source"), item.get("checksum")


def differences(product, qualification):
    identities = {identity(item) for item in product}
    return [identity(item) for item in qualification
            if item["name"] not in DEV_ONLY and identity(item) not in identities]


def manifests(manifest):
    # --no-deps reads Cargo's resolved workspace inheritance, package renames,
    # and target dependency tables without updating/resolving the lock graph.
    result = subprocess.run(
        ["cargo", "metadata", "--offline", "--no-deps", "--format-version", "1",
         "--manifest-path", str(manifest)],
        check=True, capture_output=True, text=True,
    )
    return json.loads(result.stdout)["packages"]


def missing_edges(packages, qualification):
    local = {(p["name"], p["version"]): p for p in packages if p.get("source") is None}
    missing = []
    for locked in qualification:
        if locked.get("source") is not None:
            continue
        manifest = local.get((locked["name"], locked["version"]))
        if manifest is None:
            # Do not silently skip an unknown local parser crate.
            missing.append((locked["name"], "missing path manifest"))
            continue
        edges = {edge.split(" ", 1)[0] for edge in locked.get("dependencies", [])}
        for dependency in manifest["dependencies"]:
            if dependency.get("kind") is None and not dependency.get("optional", False):
                # Cargo metadata's name is the actual package; rename is its
                # local Rust alias. Lockfiles record the actual package name.
                name = dependency["name"]
                if name not in edges:
                    missing.append((locked["name"], name))
    return sorted(set(missing))


def path_manifests(*roots):
    packages = []
    pending = list(roots)
    seen = set()
    while pending:
        manifest = Path(pending.pop()).resolve()
        if manifest in seen:
            continue
        found = manifests(manifest)
        packages.extend(found)
        seen.update(Path(p["manifest_path"]).resolve() for p in found)
        for package in found:
            for dependency in package["dependencies"]:
                if dependency.get("path"):
                    pending.append(Path(dependency["path"]) / "Cargo.toml")
    return packages


def main():
    product = tomllib.loads((ROOT.parent / "Cargo.lock").read_text())["package"]
    qualification = tomllib.loads((ROOT / "Cargo.lock").read_text())["package"]
    drift = differences(product, qualification)
    if drift:
        raise SystemExit(f"Parser dependency drift from the product lock: {drift}")
    try:
        packages = path_manifests(ROOT.parent / "Cargo.toml", ROOT / "Cargo.toml")
    except (OSError, subprocess.CalledProcessError, json.JSONDecodeError) as error:
        detail = error.stderr.strip() if isinstance(error, subprocess.CalledProcessError) else str(error)
        raise SystemExit(f"Could not read Cargo manifests for parser edge validation: {detail}") from error
    missing = missing_edges(packages, qualification)
    if missing:
        raise SystemExit(f"Parser lock misses required normal dependency edges: {missing}")
    print("Every parser/transitive package uses the product's exact locked version/source/checksum; required normal path dependency edges are present")


if __name__ == "__main__":
    main()
