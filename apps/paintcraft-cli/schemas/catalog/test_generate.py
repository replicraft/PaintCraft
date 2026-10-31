"""Portable schema controls; synthetic objects, not actual CLI/MCP qualification."""
import sys
sys.dont_write_bytecode = True

import copy
import hashlib
import itertools
import json
from pathlib import Path
import runpy
import unittest

from jsonschema import Draft202012Validator, FormatChecker

HERE = Path(__file__).resolve().parent
GEN = runpy.run_path(str(HERE / "generate.py"), run_name="catalog_schema_generator")
SHA, ENCODED = GEN["build"]()
SCHEMAS = {k: json.loads(ENCODED[k + ".schema.json"]) for k in GEN["SOURCES"]}
VALIDATORS = {k: Draft202012Validator(v, format_checker=FormatChecker()) for k, v in SCHEMAS.items()}
UUID = "00000000-0000-0000-0000-000000000001"
LEVELS = {"kind": "levels-rgb-v1", "input_black": 0, "input_white": 255,
          "gamma_hundredths": 100, "output_black": 0, "output_white": 255}


def batch(descriptor=None):
    descriptor = copy.deepcopy(LEVELS if descriptor is None else descriptor)
    return {"schema": "paintcraft.batch/catalog-v1", "atomic": True,
            "policy": "paintcraft.core/default-v1", "base": {"state_id": UUID, "publication_epoch": "0"},
            "required_filters": [descriptor["kind"]], "operations": [{"op": "filter.add", "layer_ref": {"id": UUID},
                "ref": "tone", "descriptor": descriptor}]}


def features(mask=False, spatial=False, source=False):
    return [GEN["POINT"]] + ([GEN["MASK"]] if mask else []) + ([GEN["SPATIAL"]] if spatial else []) + [GEN["CATALOG"]] + ([GEN["SOURCE"]] if source else [])


def result(mask=False, spatial=False, source=False):
    return {"schema": "paintcraft.result/catalog-v1", "committed": True,
            "state_id": UUID, "publication_epoch": "1", "history_id": UUID,
            "aliases": {}, "warnings": [], "document_features": features(mask, spatial, source),
            "required_filters": sorted(["levels-rgb-v1"] + (["box-blur-v1"] if spatial else []))}


def inspection(mcp=False, mask=False, spatial=False, source=False):
    row = {"id": UUID, "enabled": False, "descriptor": copy.deepcopy(LEVELS)}
    if mask:
        row["mask"] = {"schema": "a8-bands/v1", "enabled": False, "domain": [-1, 0, 2, 1],
                       "bands": 0, "runs": 0, "digest": "0" * 64}
    filters = [row]
    if spatial:
        filters.append({"id": "00000000-0000-0000-0000-000000000002", "enabled": False,
                        "descriptor": {"kind": "box-blur-v1", "radius": 0}})
    ids = sorted([f["descriptor"]["kind"] for f in filters])
    if mcp:
        return {"schema": "paintcraft.mcp-inspect/catalog-v1", "document_id": UUID, "state_id": UUID,
                "width": 2, "height": 1, "dirty": False, "undo_depth": 0, "redo_depth": 0,
                "required_filters": ids, "document_features": features(mask, spatial, source),
                "layers": [{"id": UUID, "name": "x", "visible": True, "opacity": 255, "filters": filters}]}
    return {"schema": "paintcraft.inspect/catalog-v1", "required_filters": ids,
            "document_features": features(mask, spatial, source), "input_sha256": "0" * 64,
            "ticket": {"state_id": UUID, "publication_epoch": "0"},
            "assessment": {"schema": "paintcraft.open-assessment/catalog-v1", "assessment_only": True,
                "source_sha256": "0" * 64, "profile": "paintcraft.native/binary6-document6-filter-catalog1",
                "container_version": 6, "document_schema": "paintcraft.document/v6",
                "required_features": GEN["HEADER"] + ([GEN["SOURCE"]] if source else []),
                "required_filters": [{"profile_id": name, "codec_id": "params-v1"} for name in ids],
                "capability_generation": "0", "mode": "editable", "migration_id": None, "reasons": []},
            "document": {"document_id": UUID, "state_id": UUID, "canvas": {"width": 2, "height": 1},
                "policy": "paintcraft.core/default-v1", "layers": [{"id": UUID, "name": "x", "visible": True,
                    "opacity_u8": 255, "origin": {"x": 0, "y": 0}, "width": 2, "height": 1,
                    "default_pixel": "#00000000", "filters": filters}]}}


class CatalogSchemas(unittest.TestCase):
    def valid(self, kind, value):
        VALIDATORS[kind].validate(value)

    def invalid(self, kind, value):
        self.assertTrue(list(VALIDATORS[kind].iter_errors(value)), (kind, value))

    def test_immutable_ids_hashes_and_local_resolution(self):
        manifest = json.loads(ENCODED["manifest.json"])
        self.assertEqual(SHA, hashlib.sha256(GEN["canonical"](manifest["profile_contract"])).hexdigest())
        for kind, schema in SCHEMAS.items():
            Draft202012Validator.check_schema(schema)
            self.assertEqual(schema["$id"], f"urn:paintcraft:schema:catalog-v1:{SHA}:{kind}")
        for row in manifest["schemas"]:
            self.assertEqual(row["sha256"], hashlib.sha256(ENCODED[row["file"]]).hexdigest())
        for name, expected in ENCODED.items():
            self.assertEqual((HERE / SHA / name).read_bytes(), expected)

    def test_all_eight_complete_descriptors_and_closed_legacy_input(self):
        values = [{"kind": "invert-rgb-v1"}, {"kind": "brightness-contrast-v1", "brightness": -100, "contrast": 99},
                  {"kind": "black-white-rgb-v1", "strength": 100}, {"kind": "threshold-luma-v1", "level": 0},
                  {"kind": "posterize-rgb-v1", "levels": 32}, {"kind": "solarize-rgb-v1"},
                  {"kind": "box-blur-v1", "radius": 32}, LEVELS]
        for descriptor in values:
            self.valid("batch", batch(descriptor))
        old = GEN["legacy"]("batch")
        value = batch()
        value["schema"] = "paintcraft.batch/v5"
        value.pop("required_filters")
        self.assertFalse(Draft202012Validator(old).is_valid(value))
        value["operations"][0]["descriptor"] = {"kind": "invert-rgb-v1"}
        Draft202012Validator(old).validate(value)

    def test_levels_required_keys_types_bounds_and_no_unknowns(self):
        for key in list(LEVELS)[1:]:
            for invalid in [None, True, "100", 100.5, -32769, 32768]:
                value = batch()
                value["operations"][0]["descriptor"][key] = invalid
                self.invalid("batch", value)
            value = batch()
            value["operations"][0]["descriptor"].pop(key)
            self.invalid("batch", value)
        for key, invalid in [("input_black", 255), ("input_white", 0), ("gamma_hundredths", 9),
                             ("gamma_hundredths", 1000), ("output_black", 256), ("output_white", -1)]:
            value = batch()
            value["operations"][0]["descriptor"][key] = invalid
            self.invalid("batch", value)
        value = batch()
        value["operations"][0]["descriptor"]["gamma"] = 1
        self.invalid("batch", value)
        value = batch()
        value["required_filters"] *= 2
        self.invalid("batch", value)
        value = batch()
        value["required_filters"] = ["future-filter-v1"]
        self.invalid("batch", value)
        value = batch()
        value.pop("required_filters")
        self.invalid("batch", value)

    def test_results_all_feature_shapes_and_catalog_envelope(self):
        for flags in itertools.product([False, True], repeat=3):
            engine = result(*flags)
            self.valid("result", engine)
            self.valid("file-command", {"schema": "paintcraft.file-command/catalog-v1", "engine": engine,
                                       "file": None, "outcome": "success", "exit_code": 0, "synthetic": True})
        for replacement in [[], [GEN["CATALOG"], GEN["POINT"]], [GEN["POINT"], GEN["CATALOG"], GEN["CATALOG"]]]:
            value = result()
            value["document_features"] = replacement
            self.invalid("result", value)
        value = result()
        value["required_filters"] = ["invert-rgb-v1"]
        self.invalid("result", value)
        value = result()
        value["extra"] = 1
        self.invalid("result", value)
        wrapper = {"schema": "paintcraft.file-command/v4", "engine": result(), "file": None,
                   "outcome": "success", "exit_code": 0, "synthetic": True}
        self.invalid("file-command", wrapper)

    def test_cli_and_mcp_projection_features_and_assessment_manifest(self):
        for flags in itertools.product([False, True], repeat=3):
            self.valid("inspect", inspection(False, *flags))
            self.valid("mcp-inspect", inspection(True, *flags))
        for kind in ["inspect", "mcp-inspect"]:
            value = inspection(kind == "mcp-inspect", True, True)
            value["document_features"] = features(False, False)
            self.invalid(kind, value)
            value = inspection(kind == "mcp-inspect")
            if kind == "inspect":
                value["document"]["layers"][0]["filters"][0]["descriptor"] = {"kind": "invert-rgb-v1"}
            else:
                value["layers"][0]["filters"][0]["descriptor"] = {"kind": "invert-rgb-v1"}
            self.invalid(kind, value)
        value = inspection()
        value["assessment"]["required_filters"][0]["codec_id"] = "params-v2"
        self.invalid("inspect", value)
        value = inspection()
        value["assessment"]["required_features"].append(GEN["SPATIAL"])
        self.invalid("inspect", value)
        value = inspection()
        value["assessment"]["schema"] = "paintcraft.open-assessment/v1"
        self.invalid("inspect", value)
        self.invalid("mcp-inspect", inspection())
        self.invalid("inspect", inspection(True))

    def test_runtime_checks_are_not_falsely_claimed_as_static_schema_guarantees(self):
        # JSON Schema cannot compare two parameter fields or payload inventories.
        value = batch()
        value["operations"][0]["descriptor"].update(input_black=64, input_white=64)
        self.valid("batch", value)
        value = batch()
        value["required_filters"] = []
        self.valid("batch", value)
        # JSON Schema integers are mathematical, not lexical token categories.
        value = batch()
        value["operations"][0]["descriptor"]["gamma_hundredths"] = 100.0
        self.valid("batch", value)


if __name__ == "__main__":
    unittest.main()
