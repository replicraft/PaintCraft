"""Separate checksum-aware PCND container-boundary mutation smoke.

Only the outer SHA256 is recomputed. Inner source-attachment hashes and encoded
pixel payloads remain subject to the unchanged product parser validation.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile
from pcnd_replay_smoke import SEEDS, TOTAL, inventory
from replay_smoke import INPUT_MAX, MUTATIONS, ROOT, verify

STATE = 0x5041494E54435241
MASK = (1 << 64) - 1


def mutations(source, state):
    if len(source) < 32 or hashlib.sha256(source[:-32]).digest() != source[-32:]:
        raise ValueError("Checksum-aware seeds must have a valid outer SHA256")
    original = source[:-32]
    for iteration in range(MUTATIONS + 1):
        body = bytearray(original)
        state ^= (state << 13) & MASK
        state ^= state >> 7
        state ^= (state << 17) & MASK
        if iteration:
            if iteration % 4 == 0:
                del body[state % (len(body) + 1):]
            elif iteration % 4 == 1 and body:
                body[state % len(body)] ^= ((state >> 32) & 255) | 1
            elif iteration % 4 == 2 and len(body) + 32 < INPUT_MAX:
                body.append(state & 255)
            elif body:
                body[state % len(body)] = 255
        yield bytes(body) + hashlib.sha256(body).digest(), state


def run(replay):
    rows = sorted(inventory(json.loads((ROOT / "pcnd-seeds.json").read_text())),
                  key=lambda row: row["name"])
    state, cases, accepted, rejected = STATE, 0, 0, 0
    # Batches bound command-line arguments; RAII removes only these generated
    # mutation inputs. Logs/receipts are retained by the caller, not hidden here.
    parent = ROOT.parent / "docs/validation/pcnd-fuzz"
    parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="semantic-inputs-", dir=parent) as folder:
        for row in rows:
            paths = []
            for index, (data, state) in enumerate(mutations(verify(row).read_bytes(), state)):
                path = Path(folder) / f"case-{index}.pcnd"
                path.write_bytes(data)
                paths.append(str(path))
            result = subprocess.run([str(replay.resolve()), "pcnd", "0", *paths],
                                    capture_output=True, text=True, timeout=60)
            print(f"seed={row['name']} {result.stdout}", end="", flush=True)
            if result.stderr:
                print(result.stderr, end="", flush=True)
            result.check_returncode()
            match = re.fullmatch(
                r"target=Pcnd cases=(\d+) accepted=(\d+) rejected=(\d+) "
                r"seed=0x5041494e54435241 mode=deterministic-replay policy=invariants-fail-v1\n",
                result.stdout)
            if not match:
                raise ValueError("Unexpected checksum-aware parser receipt")
            count, good, bad = map(int, match.groups())
            if count != MUTATIONS + 1 or good + bad != count:
                raise ValueError("Incomplete checksum-aware seed batch")
            cases += count
            accepted += good
            rejected += bad
    for row in rows:
        verify(row)
    if cases != TOTAL or accepted < 10 or accepted + rejected != cases:
        raise ValueError("Incomplete checksum-aware PCND semantic stream")
    print(f"Checksum-aware PCND: seeds={SEEDS} cases={cases} accepted={accepted} rejected={rejected}; outer-SHA-only recomputation; container-boundary mutation smoke, not coverage-guided qualification")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--replay", type=Path, required=True)
    run(parser.parse_args().replay)
