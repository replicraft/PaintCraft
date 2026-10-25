"""Run inside GIMP 3's python-fu-eval batch interpreter, with no displays.

PAINTCRAFT_ORA_INPUT names a fresh PaintCraft export; PAINTCRAFT_ORA_REPORT
names a private JSON result. The image is loaded, checked, edited, saved to XCF,
and reopened. No Gimp.Display is created and no interactive procedures are used.
"""
import hashlib
import json
import os
from pathlib import Path

import gi
gi.require_version("Gimp", "3.0")
gi.require_version("Gegl", "0.4")
from gi.repository import Gimp, Gegl, Gio

Gegl.init(None)
source = Path(os.environ["PAINTCRAFT_ORA_INPUT"])
report = Path(os.environ["PAINTCRAFT_ORA_REPORT"])
oracle = json.loads((Path(os.environ["PAINTCRAFT_ORA_FIXTURES"]) / "oracle.json").read_text())
image = Gimp.file_load(Gimp.RunMode.NONINTERACTIVE, Gio.File.new_for_path(str(source)))
assert image is not None
assert (image.get_width(), image.get_height()) == (3, 2)
layers = list(reversed(image.get_layers()))
assert len(layers) == 3
observed = []
for index, (layer, expected) in enumerate(zip(layers, oracle["layers_bottom_to_top"])):
    assert layer.get_name() == expected["name"]
    assert layer.get_visible() == (index != 2)
    assert abs(layer.get_opacity() - 100.0) < 0.000001
    offsets = layer.get_offsets()
    assert tuple(offsets[-2:]) == [(0, 0), (1, 1), (-1, 0)][index]
    buffer = layer.get_buffer()
    pixels = buffer.get(buffer.get_extent(), 1.0, "R'G'B'A u8", Gegl.AbyssPolicy.NONE)
    digest = hashlib.sha256(pixels).hexdigest()
    assert digest == expected["rgba_sha256"], (index, digest, expected)
    observed.append({"name": layer.get_name(), "rgba_sha256": digest})

# An editable native save and reopen demonstrates more than a rendered preview.
layers[1].set_name("Edited in GIMP 日本語")
layers[1].set_opacity(50.0)
layers[2].set_visible(True)
native = source.with_suffix(".xcf")
assert Gimp.file_save(Gimp.RunMode.NONINTERACTIVE, image, Gio.File.new_for_path(str(native)), None)
image.delete()
reopened = Gimp.file_load(Gimp.RunMode.NONINTERACTIVE, Gio.File.new_for_path(str(native)))
changed = list(reversed(reopened.get_layers()))
assert changed[1].get_name() == "Edited in GIMP 日本語"
assert abs(changed[1].get_opacity() - 50.0) < 0.000001
assert changed[2].get_visible()
reopened.delete()
native.unlink()
report.write_text(json.dumps({"application": Gimp.version(), "status": "verified", "layers": observed,
                              "edit_native_save_reopen": True}, ensure_ascii=False, indent=4) + "\n")
print("PAINTCRAFT_ORA_GIMP_VERIFIED")
