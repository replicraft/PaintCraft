"""A separate PCND stream cannot silently relabel historical/stock counts."""
import copy
import json
import hashlib
import unittest
from pcnd_replay_smoke import ROOT, SEEDS, TOTAL, inventory
from pcnd_semantic_smoke import STATE, mutations
from replay_smoke import MUTATIONS, verify, reviewed_inventory
from stock_replay_smoke import inventory as stock_inventory


class PcndInventory(unittest.TestCase):
    def test_checksum_aware_stream_is_deterministic_and_repairs_only_outer_hash(self):
        manifest = json.loads((ROOT / "pcnd-seeds.json").read_text())
        row = next(row for row in inventory(manifest) if row["name"] == "rgb-profile.pcnd")
        source = verify(row).read_bytes()
        first = list(mutations(source, STATE))
        self.assertEqual(first, list(mutations(source, STATE)))
        self.assertEqual(len(first), MUTATIONS + 1)
        self.assertEqual(first[0][0], source)
        for data, _ in first:
            self.assertEqual(hashlib.sha256(data[:-32]).digest(), data[-32:])
        # Iteration2 appends one byte to the body. The complete original inner
        # attachment, including its own checksum, is unchanged rather than repaired.
        self.assertEqual(first[2][0][:-33], source[:-32])
        corrupted = source[:-1] + bytes([source[-1] ^ 1])
        with self.assertRaises(ValueError):
            list(mutations(corrupted, STATE))

    def test_separate_inventory_and_unchanged_prior_streams(self):
        self.assertEqual(len(reviewed_inventory(json.loads((ROOT / "seeds.json").read_text()))), 24)
        self.assertEqual(len(stock_inventory(json.loads((ROOT / "stock-seeds.json").read_text()))), 29)
        manifest = json.loads((ROOT / "pcnd-seeds.json").read_text())
        self.assertEqual(len(inventory(manifest)), SEEDS)
        self.assertEqual(TOTAL, 3598)
        for change in ["schema", "duplicate", "count", "target", "sha256", "positive"]:
            altered = copy.deepcopy(manifest)
            if change == "schema":
                altered["schema"] = "unknown"
            elif change == "duplicate":
                altered["seeds"][1] = altered["seeds"][0]
            elif change == "count":
                altered["seeds"].pop()
            elif change == "target":
                altered["seeds"][0]["target"] = "unknown"
            elif change == "sha256":
                altered["seeds"][0]["sha256"] = "0" * 64
            else:
                altered["seeds"][0]["accepted"] = False
            with self.subTest(change=change), self.assertRaises(ValueError):
                inventory(altered)


if __name__ == "__main__":
    unittest.main()
