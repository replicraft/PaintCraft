"""Owned iconset -> Apple ICNS writer -> Apple extraction controls, headless.
Run outside the filesystem sandbox: sandboxed iconset creation can fail.
"""
import hashlib
import importlib.metadata
import json
import platform
import struct
import subprocess
import sys
from pathlib import Path
sys.dont_write_bytecode = True
from PIL import Image
from generate import pixels, png
assert importlib.metadata.version("Pillow") == "13.0.0"
if len(sys.argv) != 2:
    raise SystemExit("Usage: apple_writer_control.py <new temporary directory>")
root = Path(sys.argv[1])
root.mkdir(parents=True,exist_ok=False)
report={"system":platform.platform(),"python":platform.python_version(),"pillow":"13.0.0","iconutil_sha256":hashlib.sha256(Path("/usr/bin/iconutil").read_bytes()).hexdigest(),"cases":[]}
def argb_wire(payload, count):
    if payload[:4] != b"ARGB":
        raise ValueError("missing ARGB marker")
    cursor = 4
    planes = []
    for channel in range(4):
        plane = bytearray()
        while len(plane) < count:
            if cursor == len(payload):
                raise ValueError(f"channel {channel}: missing packet")
            control = payload[cursor]
            cursor += 1
            run = control + 1 if control < 128 else control - 125
            if run > count - len(plane):
                raise ValueError(f"channel {channel}: packet crosses extent")
            length = run if control < 128 else 1
            if cursor + length > len(payload):
                raise ValueError(f"channel {channel}: truncated packet at pixel {len(plane)}: needs {length}, has {len(payload)-cursor}")
            data = payload[cursor:cursor+length]
            plane.extend(data if control < 128 else data * run)
            cursor += length
        planes.append(plane)
    if cursor != len(payload):
        raise ValueError("trailing ARGB packet data")
    return bytes(value for index in range(count) for value in (planes[1][index],planes[2][index],planes[3][index],planes[0][index]))


for name in ["partial-alpha","opaque","zero-alpha"]:
    source=root/f"{name}.iconset"
    source.mkdir()
    expected={}
    for logical in [16,32,128,256,512]:
        for scale in [1,2]:
            side=logical*scale
            rgba=bytearray(pixels(side,9))
            if name != "partial-alpha":
                for index in range(side*side):rgba[index*4+3] = 255 if name == "opaque" else 0
            filename=f"icon_{logical}x{logical}"+("@2x"if scale==2 else "")+".png"
            (source/filename).write_bytes(png(side,rgba))
            expected[filename]=bytes(rgba)
    encoded=root/f"{name}.icns"
    result=subprocess.run(["/usr/bin/iconutil","-c","icns",str(source),"-o",str(encoded)],capture_output=True,text=True)
    case={"case":name,"writer_exit":result.returncode,"writer_stderr":result.stderr,"source_png_sha256":{p.name:hashlib.sha256(p.read_bytes()).hexdigest()for p in sorted(source.glob("*.png"))}}
    report["cases"].append(case)
    if result.returncode:continue
    data=encoded.read_bytes()
    case["icns_sha256"]=hashlib.sha256(data).hexdigest()
    case["records"]=[]
    pos=8
    while pos<len(data):
        tag,length=struct.unpack_from(">4sI",data,pos)
        assert length>=8 and pos+length<=len(data)
        entry={"tag":tag.decode(),"length":length,"payload_sha256":hashlib.sha256(data[pos+8:pos+length]).hexdigest()}
        case["records"].append(entry)
        if tag in [b"ic04",b"ic05"]:
            side=16 if tag==b"ic04" else 32
            try:
                decoded=argb_wire(data[pos+8:pos+length],side*side)
                entry["independent_wire_exact_rgba"]=decoded==expected[f"icon_{side}x{side}.png"]
                entry["independent_wire_rgba_sha256"]=hashlib.sha256(decoded).hexdigest()
            except ValueError as error:
                entry["independent_wire_refusal"]=str(error)
        pos+=length
    assert pos==len(data)
    target=root/f"{name}-readback.iconset"
    result=subprocess.run(["/usr/bin/iconutil","-c","iconset",str(encoded),"-o",str(target)],capture_output=True,text=True)
    case.update({"extract_exit":result.returncode,"extract_stderr":result.stderr,"small_entries":[]})
    for side in [16,32]:
        filename=f"icon_{side}x{side}.png"
        with Image.open(target/filename)as image:actual=image.convert("RGBA").tobytes()
        pairs=list(zip([expected[filename][i:i+4]for i in range(0,len(actual),4)],[actual[i:i+4]for i in range(0,len(actual),4)]))
        row={"side":side,"different_pixels":sum(a!=b for a,b in pairs),"different_alpha":sum(a[3]!=b[3]for a,b in pairs),"different_visible_rgb":sum(a[:3]!=b[:3]and a[3]!=0 for a,b in pairs),"different_hidden_rgb":sum(a[:3]!=b[:3]and a[3]==0 for a,b in pairs),"first_differences":[{"index":i,"expected":list(a),"actual":list(b)}for i,(a,b)in enumerate(pairs)if a!=b][:8]}
        try:
            with Image.open(encoded)as image:decoded=image.icns.getimage((side,side,1)).convert("RGBA").tobytes()
            row["pillow_exact_rgba"]=decoded==expected[filename]
        except (KeyError,ValueError,OSError)as error:row["pillow_refusal"]=repr(error)
        case["small_entries"].append(row)
(root/"report.json").write_text(json.dumps(report,indent=4)+"\n")
print(json.dumps(report,indent=4))
