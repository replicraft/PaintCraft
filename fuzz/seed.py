#!/usr/bin/env python3
"""Verify existing fixture provenance and stage exact seeds without duplicating source assets."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parent
MAX_INPUT = 256 << 10

def main():
    corpus = ROOT / "corpus"
    if corpus.is_symlink():
        raise ValueError("Refusing a symlink corpus directory")
    for seed in json.loads((ROOT / "seeds.json").read_text())["seeds"]:
        source = (REPO / seed["source"]).resolve()
        if not source.is_relative_to(REPO):
            raise ValueError("Seed source must remain in this repository")
        with source.open("rb") as stream:
            data = stream.read(MAX_INPUT + 1)
        if len(data) > MAX_INPUT or len(data) != seed["bytes"] or hashlib.sha256(data).hexdigest() != seed["sha256"]:
            raise ValueError(f"Seed provenance mismatch: {source}")
        if seed["target"] not in {"avif", "heif", "paint", "pdn", "dcx"} or Path(seed["name"]).name != seed["name"]:
            raise ValueError("Invalid target or corpus filename")
        directory = corpus / seed["target"]
        if directory.is_symlink():
            raise ValueError("Refusing symlink target corpus")
        directory.mkdir(parents=True, exist_ok=True)
        destination = directory / seed["name"]
        if destination.is_symlink():
            raise ValueError("Refusing symlink seed destination")
        destination.write_bytes(data)
        print(f"{seed['target']}/{seed['name']} {len(data)} {seed['sha256']}")

if __name__ == "__main__":
    main()
