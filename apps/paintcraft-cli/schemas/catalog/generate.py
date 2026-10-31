#!/usr/bin/env python3
"""Resolve immutable finite catalog schema bundles; never modify legacy files."""
import sys
sys.dont_write_bytecode = True

import argparse
import copy
import hashlib
import itertools
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
BASE = HERE.parent
HASH_TOKEN = "__PROFILE_CONTRACT_SHA256__"
SOURCES = {
    "batch": ("batch-v5", "6a049a3b05d2a0db7d611f5bcc88fd3543ed71f7a1b1e2a0ef9c5fbd19cb34b0"),
    "result": ("result-v4", "b0c412a2fab1135812ded70023618a2812d5baa45bb44de4048e465045bee154"),
    "file-command": ("file-command-v4", "78ab9a1ac3bec9e3274cc480d4ad8b4ad516b6f10c8d5fdbd3b1265237cc8b8a"),
    "inspect": ("inspect-v4", "b5197142216edea86d388536961ea65a6c6c5e59f4c4dedff98ec57ca2949bc0"),
    "mcp-inspect": ("mcp-inspect-v4", "d8efe86d229d278c2f87868f89daef7c1397217a5fc06ef18a79e1c8f85e0fe8"),
}
POINT = "layer-point-filters.v1"
MASK = "layer-filter-masks.v1"
SPATIAL = "layer-spatial-filters.v1"
CATALOG = "layer-filter-catalog.v1"
SOURCE = "source-metadata.v1"
CATALOG_ONLY = ["curves-rgb-v1", "exposure-srgb-v1", "levels-rgb-v1", "vibrance-rgb-v1"]
HEADER = ["raster.rgba8", "canvas.finite", "layers.flat", POINT, MASK, CATALOG]


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def legacy(kind):
    name, expected = SOURCES[kind]
    raw = (BASE / (name + ".schema.json")).read_bytes()
    if sha(raw) != expected:
        raise ValueError(f"Frozen legacy input changed: {name}")
    return json.loads(raw)


def strip_ids(value):
    if isinstance(value, dict):
        return {k: strip_ids(v) for k, v in value.items() if k not in ["$id", "$schema"]}
    if isinstance(value, list):
        return [strip_ids(v) for v in value]
    return value


def profile_definitions():
    first = legacy("batch")["properties"]["operations"]["items"]["oneOf"][0]
    profiles = copy.deepcopy(first["properties"]["descriptor"]["oneOf"])
    properties = {"kind": {"const": "levels-rgb-v1"}}
    for key, minimum, maximum in [("input_black", 0, 254), ("input_white", 1, 255),
                                   ("gamma_hundredths", 10, 999), ("output_black", 0, 255),
                                   ("output_white", 0, 255)]:
        properties[key] = {"type": "integer", "minimum": minimum, "maximum": maximum}
    profiles.append({"type": "object", "properties": properties, "required": list(properties),
                     "additionalProperties": False,
                     "$comment": "Runtime additionally requires input_black < input_white and canonical integer tokens."})
    profiles.append({"type": "object", "properties": {
        "kind": {"const": "exposure-srgb-v1"},
        "stops_hundredths": {"type": "integer", "minimum": -800, "maximum": 800}},
        "required": ["kind", "stops_hundredths"], "additionalProperties": False,
        "$comment": "Runtime requires canonical integer tokens; zero EV is retained catalog intent."})
    profiles.append({"type": "object", "properties": {
        "kind": {"const": "curves-rgb-v1"},
        "outputs": {"type": "array", "items": {"type": "integer", "minimum": 0, "maximum": 255},
                    "minItems": 5, "maxItems": 5}},
        "required": ["kind", "outputs"], "additionalProperties": False,
        "$comment": "Five fixed input knots 0,64,128,192,255; free output ordering and endpoints. Runtime requires canonical integer tokens."})
    profiles.append({"type":"object","properties":{
        "kind":{"const":"vibrance-rgb-v1"},
        "amount_hundredths":{"type":"integer","minimum":-100,"maximum":100}},
        "required":["kind","amount_hundredths"],"additionalProperties":False,
        "$comment":"Whole incoming RGB, exact rational adaptive saturation; runtime canonical integer tokens. Default and disabled remain stored intent."})
    return sorted(profiles, key=lambda p: p["properties"]["kind"]["const"])


def requirements(ids, stored):
    result = {"type": "array", "items": {"enum": ids}, "uniqueItems": True,
              "minItems": 1 if stored else 0, "maxItems": len(ids),
              "$comment": "Runtime enforces ASCII sorting and exact Add/Update union (batch) or stored inventory (output)."}
    if stored:
        result["contains"] = {"enum": CATALOG_ONLY}
    return result


def features():
    choices = []
    for mask, spatial, source in itertools.product([False, True], repeat=3):
        items = [POINT] + ([MASK] if mask else []) + ([SPATIAL] if spatial else []) + [CATALOG] + ([SOURCE] if source else [])
        choices.append({"const": items})
    return {"oneOf": choices}


def descriptor_union(profiles):
    return {"oneOf": copy.deepcopy(profiles)}


def has_filter(test, nested):
    shape = {"required": ["layers"], "properties": {"layers": {"contains": {
        "required": ["filters"], "properties": {"filters": {"contains": test}}}}}}
    return {"required": ["document"], "properties": {"document": shape}} if nested else shape


def kind_test(kind):
    return {"required": ["descriptor"], "properties": {"descriptor": {
        "required": ["kind"], "properties": {"kind": {"const": kind}}}}}


def flag_rule(condition, feature):
    return {"if": condition,
            "then": {"properties": {"document_features": {"contains": {"const": feature}}}},
            "else": {"properties": {"document_features": {"not": {"contains": {"const": feature}}}}}}


def templates():
    profiles = profile_definitions()
    ids = [p["properties"]["kind"]["const"] for p in profiles]
    docs = {kind: strip_ids(legacy(kind)) for kind in SOURCES}
    for kind, d in docs.items():
        d["$schema"] = "https://json-schema.org/draft/2020-12/schema"
        d["$id"] = f"urn:paintcraft:schema:catalog-v1:{HASH_TOKEN}:{kind}"
        d["title"] = f"PaintCraft finite eleven-profile catalog {kind}"
        d["properties"]["schema"] = {"const": f"paintcraft.{kind}/catalog-v1"}
        d["$comment"] = "Immutable resolved catalog bundle; legacy profiles unchanged. Runtime verifies inventories and relational constraints."
    batch = docs["batch"]
    batch["properties"]["required_filters"] = requirements(ids, False)
    batch["required"].append("required_filters")
    for op in batch["properties"]["operations"]["items"]["oneOf"]:
        if op["properties"]["op"]["const"] in ["filter.add", "filter.update"]:
            op["properties"]["descriptor"] = descriptor_union(profiles)
    result = docs["result"]
    result["properties"]["document_features"] = features()
    result["properties"]["required_filters"] = requirements(ids, True)
    result["required"].append("required_filters")
    docs["file-command"]["$defs"]["engine_result"] = strip_ids(result)
    for kind in ["inspect", "mcp-inspect"]:
        d = docs[kind]
        nested = kind == "inspect"
        layers = (d["properties"]["document"] if nested else d)["properties"]["layers"]
        # Legacy inspect/v4 directly requires Box. Catalog documents instead
        # require a catalog-only profile; spatial features remain conditional.
        layers.pop("contains", None)
        layer = layers["items"]
        layer["properties"]["filters"]["items"]["properties"]["descriptor"] = descriptor_union(profiles)
        d["properties"]["document_features"] = features()
        d["properties"]["required_filters"] = requirements(ids, True)
        d["required"].append("required_filters")
        d["allOf"] = [has_filter({"anyOf": [kind_test(name) for name in CATALOG_ONLY]}, nested),
                      flag_rule(has_filter({"required": ["mask"]}, nested), MASK),
                      flag_rule(has_filter(kind_test("box-blur-v1"), nested), SPATIAL)]
        if nested:
            assessment = d["properties"]["assessment"]
            assessment["properties"]["schema"] = {"const": "paintcraft.open-assessment/catalog-v1"}
            assessment["properties"]["profile"] = {"const": "paintcraft.native/binary6-document6-filter-catalog1"}
            assessment["properties"]["container_version"] = {"const": 6}
            assessment["properties"]["document_schema"] = {"const": "paintcraft.document/v6"}
            assessment["properties"]["required_features"] = {"oneOf": [{"const": HEADER}, {"const": HEADER + [SOURCE]}]}
            pair = {"type": "object", "properties": {"profile_id": {"enum": ids}, "codec_id": {"const": "params-v1"}},
                    "required": ["profile_id", "codec_id"], "additionalProperties": False}
            assessment["properties"]["required_filters"] = {"type": "array", "items": pair, "minItems": 1,
                "maxItems": len(ids), "uniqueItems": True, "contains": {
                    "required": ["profile_id"], "properties": {"profile_id": {"enum": CATALOG_ONLY}}}}
            assessment["required"].append("required_filters")
            condition = {"properties": {"assessment": {"properties": {
                "required_features": {"contains": {"const": SOURCE}}}}}}
            d["allOf"].append(flag_rule(condition, SOURCE))
    contract = {
        "schema": "paintcraft.filter-profile-contract/v1", "codec_id": "params-v1",
        "profiles": profiles,
        "numerical_identity": "Versioned numerical IDs retain their independently specified arithmetic; no LUT or transport identity substitution.",
        "envelope_names": {k: f"paintcraft.{k}/catalog-v1" for k in SOURCES},
        "assessment_schema": "paintcraft.open-assessment/catalog-v1",
        "header6_features": HEADER,
        "runtime_constraints": ["ordered input endpoints", "canonical integer tokens", "sorted unique inventories",
                                "exact Add/Update union", "exact final stored inventory", "actual source/mask/spatial features"],
        "resolved_template_sha256": {k: sha(canonical(v)) for k, v in docs.items()},
    }
    return docs, contract


def build():
    docs, contract = templates()
    digest = sha(canonical(contract))
    encoded = {kind + ".schema.json": (json.dumps(doc, indent=2).replace(HASH_TOKEN, digest) + "\n").encode()
               for kind, doc in docs.items()}
    manifest = {
        "schema": "paintcraft.schema-catalog-manifest/v1", "profile_contract_sha256": digest,
        "contract_hash_encoding": "SHA256 of UTF8 compact JSON, sorted object keys, ensure_ascii=true, no final newline",
        "profile_contract": contract,
        "schemas": [{"kind": k, "id": f"urn:paintcraft:schema:catalog-v1:{digest}:{k}",
                     "file": k + ".schema.json", "sha256": sha(encoded[k + ".schema.json"])} for k in SOURCES],
        "frozen_legacy_inputs": [{"file": name + ".schema.json", "sha256": expected} for name, expected in SOURCES.values()],
        "availability": "Schema definitions only; runtime integration and actual consumer validation require separate qualification.",
    }
    encoded["manifest.json"] = (json.dumps(manifest, indent=2) + "\n").encode()
    return digest, encoded


def discovery(digest):
    rows = "\n".join(f"| `{kind}` | `urn:paintcraft:schema:catalog-v1:{digest}:{kind}` |" for kind in SOURCES)
    return f"""# Catalog schema discovery

Generated by `generate.py`; schema definitions are not a runtime support claim.
The current eleven-profile contract SHA-256 is `{digest}`. Its resolved bundle is
`{digest}/manifest.json`. Schema IDs are immutable and fully qualified:

| Kind | Immutable schema ID |
|---|---|
{rows}

The manifest records the canonical profile contract and every schema file hash.
Consumers should resolve the selected manifest and validate against that exact
bundle, rather than treating the fixed envelope name as a changing descriptor
union. Future registered algorithms retain envelope names `catalog-v1` but require
a new contract hash and a new immutable bundle. Old files are never overwritten.
The frozen eight-profile bundle remains at
`d842bed34f9b27250082dffc95656eecb28c266cedadbfe745cf1d07ae3148f6/`.
The frozen nine-profile bundle remains at
`cac612d12ffbfd5bd8826fb6fd9e78d727e94f5186626867a58305ac56f20453/`.
The frozen ten-profile bundle remains at
`2729c23e843ca43c502a01e8f1e41d5d2ad9c0bfba610bb9b2659274d8e780e5/`.
Vibrance uses kind `vibrance-rgb-v1` and sole integer `amount_hundredths` in
-100..100, default0. This is whole-RGB direct execution, never a per-channel LUT.
These schema definitions alone do not establish evaluator/transport/UI qualification.
Curves uses kind `curves-rgb-v1` and the sole required field `outputs`, exactly five
integer bytes in 0..255 at fixed input knots 0,64,128,192,255. Identity is
`[0,64,128,192,255]`; equal, inverted and nonmonotone outputs and unlocked endpoints
are valid. Identity and disabled instances remain catalog intentions. This is a
schema/source checkpoint; it does not advertise a qualified Curves evaluator,
transport, UI or public feature.
Exposure uses kind `exposure-srgb-v1` and the sole required integer parameter
`stops_hundredths` in -800..800. Zero EV and disabled Exposure remain catalog
intentions. Catalog outputs require stored Levels, Exposure, Curves or Vibrance; removing
the last catalog-only intention restores the applicable older minimal profile.
Existing batch v1–v5 and inspect/result/envelope profiles remain frozen.

Batch `required_filters` is the sorted unique union of numerical IDs in Add/Update
payloads, including legacy kinds. Result and inspection inventories are the sorted
unique IDs of all stored intentions, including disabled/default entries, and use
catalog output only when a catalog-only profile remains in the final document.
Assessment reports actual file manifest pairs as `{{profile_id,codec_id}}`.
`params-v1` is scoped by numerical ID. Header6 grammar and actual document feature
inventory remain distinct; no unconditional spatial flag is inferred from framing.

JSON Schema enforces closed objects, known eleven-profile parameter domains, unique
inventory items and canonical feature-list shapes. Runtime additionally checks
exact inventory correspondence, ASCII sorting, input-black < input-white, masks'
coordinate/canonical relationships and canonical integer tokens. JSON Schema's
integer keyword describes integral numeric values and cannot distinguish a lexical
`100.0` token; PaintCraft's strict parser must reject that token before schema use.
MCP's source-metadata feature cannot be inferred from its deliberately smaller
projection; runtime checks the actual document. Generated schema tests use synthetic
objects; genuine CLI/MCP result qualification is a separate integration gate.
""".encode()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    digest, files = build()
    folder = HERE / digest
    for name, value in files.items():
        path = folder / name
        if path.exists():
            if path.read_bytes() != value:
                raise SystemExit(f"Refusing to overwrite immutable catalog bundle: {path}")
        elif args.write:
            folder.mkdir(exist_ok=True)
            path.write_bytes(value)
        else:
            raise SystemExit(f"Missing generated schema: {path}; review before --write")
    doc = HERE / "DISCOVERY.md"
    content = discovery(digest)
    if args.write:
        doc.write_bytes(content)
    elif not doc.exists() or doc.read_bytes() != content:
        raise SystemExit("Generated DISCOVERY.md differs; review before --write")
    print(digest)


if __name__ == "__main__":
    main()
