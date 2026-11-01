#!/usr/bin/env python3
"""Independent tiny exact MSP1/MSP2 framing; no product imports or execution."""
from pathlib import Path
import hashlib, json, struct, sys
HERE=Path(__file__).resolve().parent

def header(magic):
    words=[int.from_bytes(magic[:2],"little"),int.from_bytes(magic[2:],"little"),2,1,1,1,1,1,2,1,0,0,0,0,0,0]
    checksum=0
    for word in words: checksum ^= word
    words[12]=checksum
    return struct.pack("<16H",*words)

def outputs():
    # Exact opaque white then black: MSB-first row 10000000, zero tail bits.
    return {"white-black-v1.msp":header(b"DanM")+bytes([0x80]),
            "white-black-v2.msp":header(b"LinS")+bytes([2,0,1,0x80])}

def main():
    if sys.argv[1:] not in ([],["--check"]): raise SystemExit("use --check or no arguments")
    files=outputs()
    manifest={"scope":"independent exact framing, not a new Noise numerical oracle", "size":[2,1],
              "rgba":[255,255,255,255,0,0,0,255],
              "files":{name:{"length":len(data),"sha256":hashlib.sha256(data).hexdigest(),"hex":data.hex()} for name,data in files.items()}}
    files["manifest.json"]=(json.dumps(manifest,indent='\t')+"\n").encode()
    for name,data in files.items():
        target=HERE/name
        if sys.argv[1:]:
            if target.read_bytes()!=data: raise SystemExit("fixture drift: "+name)
        else: target.write_bytes(data)
    print("2 independent MSP fixtures checked" if sys.argv[1:] else "2 independent MSP fixtures written")
if __name__=="__main__":main()
