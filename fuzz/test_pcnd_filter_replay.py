"""Independent PCND3 inventory cannot silently alter existing stream identity."""
import copy
import hashlib
import json
import unittest
import subprocess
import tempfile
from pathlib import Path
from unittest.mock import patch
import pcnd_filter_replay_smoke as runner
from pcnd_filter_replay_smoke import ROOT, SEEDS, TOTAL, inventory
from pcnd_replay_smoke import inventory as legacy_inventory
from pcnd_semantic_smoke import STATE, mutations
from replay_smoke import reviewed_inventory, verify
from stock_replay_smoke import inventory as stock_inventory


class FilterInventory(unittest.TestCase):
    def manifest(self):
        return json.loads((ROOT / "pcnd-filter-seeds.json").read_text())

    def test_separate_sixteen_seeds_keep_historical_inventories_distinct(self):
        rows = inventory(self.manifest())
        self.assertEqual((len(rows), SEEDS, TOTAL), (16, 16, 4112))
        self.assertEqual(sum(r["accepted"] for r in rows), 5)
        self.assertEqual(len(legacy_inventory(json.loads((ROOT / "pcnd-seeds.json").read_text()))), 14)
        self.assertEqual(len(reviewed_inventory(json.loads((ROOT / "seeds.json").read_text()))), 24)
        self.assertEqual(len(stock_inventory(json.loads((ROOT / "stock-seeds.json").read_text()))), 29)

    def test_inventory_tampering_cannot_relabel_independent_fixture_outcomes(self):
        for mode in ["schema", "count", "duplicate", "path", "target", "hash", "accepted-swap", "oracle", "generator"]:
            m = copy.deepcopy(self.manifest())
            if mode == "schema": m["schema"] = "unknown"
            elif mode == "count": m["seeds"].pop()
            elif mode == "duplicate": m["seeds"][1] = m["seeds"][0]
            elif mode == "path": m["seeds"][0]["source"] = "../outside.pcnd"
            elif mode == "target": m["seeds"][0]["target"] = "raster"
            elif mode == "hash": m["seeds"][0]["sha256"] = "0" * 64
            elif mode == "accepted-swap":
                positive = next(r for r in m["seeds"] if r["accepted"])
                negative = next(r for r in m["seeds"] if not r["accepted"])
                positive["accepted"], negative["accepted"] = False, True
            else: m[mode]["sha256"] = "0" * 64
            with self.subTest(mode=mode), self.assertRaises(ValueError): inventory(m)

    def test_timeout_keeps_exact_generated_batch_and_reraises_same_failure(self):
        manifest = self.manifest()
        accepted = {row["name"]: row["accepted"] for row in manifest["seeds"]}
        timeout = subprocess.TimeoutExpired(["replay", "pcnd"], 60,
                                            output=b"partial stdout", stderr=b"partial stderr")
        generated = {}
        def execute(args, **kwargs):
            if len(args) == 4:  # Exact original outcome controls.
                good = int(accepted[Path(args[-1]).name])
                stdout = f"target=Pcnd cases=1 accepted={good} rejected={1-good} "
            elif args[2] == "256":
                stdout = "target=Pcnd cases=4112 accepted=5 rejected=4107 "
            else:
                generated.update({Path(p).name: Path(p).read_bytes() for p in args[3:]})
                raise timeout
            stdout += "seed=0x5041494e54435241 mode=deterministic-replay policy=invariants-fail-v1\n"
            return subprocess.CompletedProcess(args, 0, stdout, "")
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder) / "fuzz"
            root.mkdir()
            (root / "pcnd-filter-seeds.json").write_text(json.dumps(manifest))
            with patch.object(runner, "ROOT", root), patch.object(runner.subprocess, "run", execute):
                with self.assertRaises(subprocess.TimeoutExpired) as caught:
                    runner.run(Path("not-executed-replay"))
            self.assertIs(caught.exception, timeout)
            parent = root.parent / "docs/validation/pcnd3-fuzz"
            failures = list(parent.glob("filter-failure-*"))
            self.assertEqual(len(failures), 1)
            self.assertFalse(list(parent.glob("filter-semantic-inputs-*")))
            self.assertEqual(len(generated), 257)
            self.assertEqual({p.name: p.read_bytes() for p in failures[0].glob("*.pcnd")}, generated)
            failure = json.loads((failures[0] / "failure.json").read_text())
            self.assertEqual(failure["timeout"], 60)
            self.assertIsNone(failure["returncode"])
            self.assertEqual(failure["stdout"], "partial stdout")
            self.assertEqual((failures[0] / "stderr.bin").read_bytes(), b"partial stderr")

    def test_outer_hash_mutations_preserve_inner_stack_bytes_on_append(self):
        row = next(r for r in inventory(self.manifest()) if r["name"] == "source-stack.pcnd")
        source = verify(row).read_bytes()
        cases = list(mutations(source, STATE))
        self.assertEqual(cases, list(mutations(source, STATE)))
        self.assertEqual(len(cases), 257)
        self.assertEqual(cases[0][0], source)
        self.assertEqual(cases[2][0][:-33], source[:-32])
        for data, _ in cases:
            self.assertEqual(hashlib.sha256(data[:-32]).digest(), data[-32:])


if __name__ == "__main__": unittest.main()
