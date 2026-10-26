"""Owned canonical small RGB/mask fixtures, independent of product encoder."""
import hashlib
import json
import sys
from pathlib import Path
sys.dont_write_bytecode = True
from generate import family, pixels, record
ROOT = Path(__file__).parent


def main():
    receipts = {}
    for side, tag, mask in [(16, "is32", "s8mk"), (32, "il32", "l8mk")]:
        rgba = pixels(side, 9)
        raw = bytes(v for i,v in enumerate(rgba) if i % 4 != 3)
        alpha = rgba[3::4]
        rle = bytearray()
        for channel in range(3):
            values = rgba[channel::4]
            for start in range(0, len(values), 128):
                chunk = values[start:start+128]
                rle += bytes([len(chunk)-1]) + chunk
        for encoding, payload in [("raw",raw),("rle",bytes(rle))]:
            name = f"legacy-{encoding}-{side}.icns"
            data = family([record(tag,payload),record(mask,alpha)])
            (ROOT/name).write_bytes(data)
            receipts[name] = {"sha256":hashlib.sha256(data).hexdigest(),"bytes":len(data),"side":side,"tag":tag,"mask":mask,"encoding":encoding,"seed":9,"rgba_sha256":hashlib.sha256(rgba).hexdigest()}
    # Separate terminal-repeat control, not a replacement for failing fixtures.
    rgba = bytearray(pixels(16, 9))
    for index in range(256): rgba[index*4+2] = 171
    payload = bytearray()
    for channel in range(3):
        if channel == 2:
            payload += bytes([255,171,251,171])
        else:
            values = rgba[channel::4]
            for start in range(0,256,128): payload += bytes([127]) + values[start:start+128]
    name = "legacy-terminal-repeat-16.icns"
    data = family([record("is32",payload),record("s8mk",rgba[3::4])])
    (ROOT/name).write_bytes(data)
    receipts[name] = {"sha256":hashlib.sha256(data).hexdigest(),"bytes":len(data),"side":16,"tag":"is32","mask":"s8mk","encoding":"rle","seed":9,"blue_override":171,"rgba_sha256":hashlib.sha256(rgba).hexdigest()}
    (ROOT/"legacy-sources.json").write_text(json.dumps({"license":"MIT, repository-authored pixel/wire content","files":receipts},indent=4)+"\n")


if __name__ == "__main__":
    main()
