"""Validate genuine Box transport outputs and keep the previous closed profiles closed.

Usage: python3 .../box_schema_controls.py RECEIPTS_DIRECTORY
Requires the already installed jsonschema qualification tool; no runtime dependency.
"""
import copy
import json
import pathlib
import sys
import jsonschema

SCHEMAS = pathlib.Path(__file__).resolve().parents[2] / "schemas"
NAMES = {
    "paintcraft.batch/v5": "batch-v5",
    "paintcraft.result/v4": "result-v4",
    "paintcraft.file-command/v4": "file-command-v4",
    "paintcraft.inspect/v4": "inspect-v4",
    "paintcraft.mcp-inspect/v4": "mcp-inspect-v4",
    "paintcraft.file-command/v2": "file-command-v2",
    "paintcraft.file-command/v3": "file-command-v3",
}

def validator(name):
    schema = json.loads((SCHEMAS / f"{name}.schema.json").read_text())
    cls = jsonschema.validators.validator_for(schema)
    cls.check_schema(schema)
    return cls(schema)


def main():
    records = sorted(pathlib.Path(sys.argv[1]).glob("*.json"))
    assert len(records) == 26, f"expected 26 genuine records, got {len(records)}"
    checks = 0
    for path in records:
        value = json.loads(path.read_text())
        name = NAMES[value["schema"]]
        check = validator(name)
        check.validate(value)
        checks += 1
        extra = copy.deepcopy(value)
        extra["unknown"] = True
        assert list(check.iter_errors(extra)), path
        checks += 1
        if "document_features" in value:
            extra = copy.deepcopy(value)
            extra["document_features"].append("future-feature.v1")
            assert list(check.iter_errors(extra)), path
            checks += 1
        if name == "batch-v5":
            for old in range(1, 5):
                extra = copy.deepcopy(value)
                extra["schema"] = f"paintcraft.batch/v{old}"
                old_name = "batch" if old == 1 else f"batch-v{old}"
                assert list(validator(old_name).iter_errors(extra)), (path, old)
                checks += 1
            for radius in [-1, 33, True, "1"]:
                extra = copy.deepcopy(value)
                extra["operations"][0]["descriptor"]["radius"] = radius
                assert list(check.iter_errors(extra)), (path, radius)
                checks += 1
        elif name in ["result-v4", "file-command-v4", "inspect-v4", "mcp-inspect-v4"]:
            old_name = {"result-v4":"result-v3", "file-command-v4":"file-command-v3", "inspect-v4":"inspect-v3", "mcp-inspect-v4":"mcp-inspect-v3"}[name]
            extra = copy.deepcopy(value)
            extra["schema"] = {"result-v4":"paintcraft.result/v3", "file-command-v4":"paintcraft.file-command/v3", "inspect-v4":"paintcraft.inspect/v3", "mcp-inspect-v4":"paintcraft.mcp-inspect/v3"}[name]
            assert list(validator(old_name).iter_errors(extra)), path
            checks += 1
    print(json.dumps({"genuine_records": len(records), "closed_schema_checks": checks}))

if __name__ == "__main__":
    main()
