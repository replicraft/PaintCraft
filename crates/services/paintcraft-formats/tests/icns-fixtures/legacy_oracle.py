"""Independent Pillow and optional Apple readback, retaining native differences."""
import hashlib
import importlib.metadata
import io
import json
import platform
import subprocess
import sys
import tempfile
from pathlib import Path
sys.dont_write_bytecode = True
from PIL import Image
from generate import pixels
ROOT = Path(__file__).parent
assert importlib.metadata.version("Pillow") == "13.0.0"
receipts = json.loads((ROOT/"legacy-sources.json").read_text())["files"]
paths = [(ROOT/name,item) for name,item in receipts.items()]
if len(sys.argv)>1:
    output=Path(sys.argv[1])
    for side in [16,32]:
        for prefix in ["legacy","raw"]:
            paths.append((output/f"{prefix}-{side}.icns",{"side":side,"seed":9}))
report={"system":platform.platform(),"python":platform.python_version(),"pillow":importlib.metadata.version("Pillow"),"iconutil_sha256":hashlib.sha256(Path("/usr/bin/iconutil").read_bytes()).hexdigest(),"results":[]}
for path,receipt in paths:
    source=path.read_bytes()
    if "sha256" in receipt: assert hashlib.sha256(source).hexdigest()==receipt["sha256"]
    side=receipt["side"]
    expected=bytearray(pixels(side,receipt["seed"]))
    if "blue_override" in receipt:
        for index in range(side*side): expected[index*4+2] = receipt["blue_override"]
    with Image.open(io.BytesIO(source)) as family:
        rgba=family.icns.getimage((side,side,1)).convert("RGBA").tobytes()
        assert rgba==expected
    with tempfile.TemporaryDirectory(prefix="paintcraft-legacy-icns-") as temporary:
        target=Path(temporary)/"readback.iconset"
        result=subprocess.run(["/usr/bin/iconutil","-c","iconset","-o",str(target),str(path)],capture_output=True,text=True)
        row={"source":path.name,"source_sha256":hashlib.sha256(source).hexdigest(),"pillow_exact_rgba":True,"native_returncode":result.returncode,"native_stderr":result.stderr,"outputs":[]}
        for imagepath in sorted(target.glob("*.png")):
            with Image.open(imagepath) as image:
                actual=image.convert("RGBA").tobytes()
                item={"name":imagepath.name,"size":list(image.size),"rgba_sha256":hashlib.sha256(actual).hexdigest()}
                if image.size==(side,side):
                    pairs=list(zip([expected[i:i+4] for i in range(0,len(expected),4)],[actual[i:i+4] for i in range(0,len(actual),4)]))
                    item.update({"different_pixels":sum(a!=b for a,b in pairs),"different_alpha":sum(a[3]!=b[3] for a,b in pairs),"different_visible_rgb":sum(a[:3]!=b[:3] and a[3]!=0 for a,b in pairs),"different_hidden_rgb":sum(a[:3]!=b[:3] and a[3]==0 for a,b in pairs),"first_differences":[{"index":i,"expected":list(a),"actual":list(b)} for i,(a,b) in enumerate(pairs) if a!=b][:8]})
                row["outputs"].append(item)
        report["results"].append(row)
print(json.dumps(report,indent=4))
