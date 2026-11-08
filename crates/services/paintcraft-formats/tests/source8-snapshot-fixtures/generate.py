#!/usr/bin/env python3
"""Independent raw PCND8/7/1 literals; stdlib only, no product imports."""
from pathlib import Path
import hashlib
import json
import sys

HERE = Path(__file__).resolve().parent
MAGIC = b"PCND\x08\r\n\x1a"
DOC, STATE, BOTTOM, TOP, FILTER = [bytes([n]) * 16 for n in [16, 17, 18, 19, 20]]


def u(n):
    assert n >= 0
    b = bytearray()
    while n >= 128:
        b.append((n & 127) | 128)
        n >>= 7
    return bytes(b) + bytes([n])


def signed(n):
    return u(2*n if n >= 0 else -2*n-1)


def text(s):
    b = s.encode()
    return u(len(b)) + b


def mask(value, enabled=True):
    # Presence, enabled, canonical encoding1, domain, bands/runs.
    out = bytes([1, int(enabled), 1])
    out += b"".join(signed(n) for n in [0, 0, 2, 1])
    if value == 0:
        return out + u(0)
    return out + u(1) + u(0) + u(1) + u(1) + u(0) + u(2) + bytes([value])


def tile(x, pixels):
    return bytes([1]) + u(x) + u(0) + u(len(pixels)) + pixels


def layer(identity, mode, color, own=None, filtered=False, filter_enabled=True,
          records=(), visible=True, opacity=255, version=8):
    out = identity + text("Layer") + bytes([int(visible), opacity])
    if version == 8:
        out += bytes([mode])
    out += signed(0) + signed(0) + u(2) + u(1) + bytes(color)
    if version >= 3:
        out += (u(1) + FILTER + bytes([int(filter_enabled)]) + u(0) + u(0) + bytes([0])) if filtered else u(0)
    if version >= 7:
        out += bytes([0]) if own is None else mask(*own)
    return out + b"".join(records) + bytes([0])


def wire(mode=1, own=None, proof_filter=False, filter_enabled=True,
         bottom_records=(), visible=True, opacity=255, version=8, duplicate=False):
    body = MAGIC[:4] + bytes([version]) + MAGIC[5:]
    if version >= 6:
        prelude = u(1)+text("invert-rgb-v1")+text("params-v1") if proof_filter else u(0)
        body += u(len(prelude)) + prelude
    body += DOC + STATE + u(2) + u(1) + bytes([0])
    if version >= 3:
        body += bytes([0])  # no source metadata
    body += u(2)
    body += layer(BOTTOM, 0, [255]*4, filtered=proof_filter, filter_enabled=filter_enabled,
                  records=bottom_records, version=version)
    body += layer(BOTTOM if duplicate else TOP, mode, [204,77,13,255], own=own,
                  visible=visible, opacity=opacity, version=version)
    return body + hashlib.sha256(body).digest()


def blend_sha(mode):
    body = b"paintcraft.layer-blends/v1\0" + DOC + (2).to_bytes(4,"big")
    body += BOTTOM + bytes([0]) + TOP + bytes([mode])
    return hashlib.sha256(body).hexdigest()


def build():
    cases = []
    def add(name, accepted=True, source8=True, expected_tiles=0, own_count=0,
            proof="opaque", **kw):
        raw = wire(**kw)
        mode = kw.get("mode", 1)
        cases.append(dict(name=name, accepted=accepted, source8=source8,
                          hex=raw.hex(), sha256=hashlib.sha256(raw).hexdigest(),
                          wire_tiles=expected_tiles, own_count=own_count,
                          blend_sha256=blend_sha(mode) if mode in [0,1,2] and not kw.get("duplicate") else None,
                          future_decoded_proof=proof,
                          settings={k:v for k,v in kw.items() if k != "bottom_records"}))
    add("zero-own-multiply")
    add("zero-own-screen", mode=2)
    add("hidden-multiply", visible=False)
    add("opacity0-screen", mode=2, opacity=0)
    add("own128", own=(128,True), own_count=1)
    add("disabled-own128", own=(128,False), own_count=1)
    add("enabled-own-zero", own=(0,True), own_count=1)
    add("proof-filter-disabled", proof_filter=True, filter_enabled=False, proof="general")
    # Independent raw explicit-default record; reader retains1, compact writer omits.
    default = tile(0,bytes([255]*8))
    add("explicit-default-record",bottom_records=(default,),expected_tiles=1,proof="general")
    add("explicit-nondefault-record",bottom_records=(tile(0,bytes([254,255,255,255]*2)),),expected_tiles=1,proof="general")
    add("all-normal-v8",accepted=False,mode=0,proof="not-source8")
    add("unknown-mode3",accepted=False,mode=3,proof="invalid")
    add("unknown-hidden255",accepted=False,mode=255,visible=False,proof="invalid")
    add("duplicate-layer",accepted=False,duplicate=True,proof="invalid")
    add("duplicate-tile",accepted=False,bottom_records=(default,default),expected_tiles=2,proof="invalid")
    add("tile-out-of-grid",accepted=False,bottom_records=(tile(1,bytes([255]*8)),),expected_tiles=1,proof="invalid")
    add("valid-old7-mask",source8=False,version=7,mode=0,own=(128,True),own_count=1,proof="old7")
    add("valid-old1",source8=False,version=1,mode=0,proof="old1")
    bad=bytearray(wire());bad[-1]^=1
    cases.append(dict(name="bad-sha",accepted=False,source8=True,hex=bad.hex(),sha256=hashlib.sha256(bad).hexdigest()))
    return dict(schema="paintcraft.source8-snapshot-independent/v1",
                scope="wire literals and semantic frame only; no execution/A8 ABI prices",
                cases=cases)


raw=(json.dumps(build(),indent="\t")+"\n").encode()
path=HERE/"oracle.json"
if "--check" in sys.argv:
    assert path.read_bytes()==raw,"oracle drift"
else:
    path.write_bytes(raw)
print(hashlib.sha256(raw).hexdigest())
