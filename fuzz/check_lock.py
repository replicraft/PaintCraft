#!/usr/bin/env python3
"""Ensure the isolated parser harness uses the product's locked source versions."""
from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parent
DEV_ONLY = {"paintcraft-container-fuzz", "libfuzzer-sys", "arbitrary"}


def identity(item):
    return item["name"], item["version"], item.get("source"), item.get("checksum")


def differences(product, qualification):
    identities = {identity(item) for item in product}
    return [identity(item) for item in qualification
            if item["name"] not in DEV_ONLY and identity(item) not in identities]


def main():
    product = tomllib.loads((ROOT.parent / "Cargo.lock").read_text())["package"]
    qualification = tomllib.loads((ROOT / "Cargo.lock").read_text())["package"]
    drift = differences(product, qualification)
    if drift:
        raise SystemExit(f"Parser dependency drift from the product lock: {drift}")
    print("Every parser/transitive package uses the product's exact locked version/source/checksum")


if __name__ == "__main__":
    main()
