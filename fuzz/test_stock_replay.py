"""Stock fixture identity and target boundaries remain separate from history."""
import copy
import json
import unittest
from replay_smoke import ROOT, reviewed_inventory
from stock_replay_smoke import COUNTS, TARGETS, TOTAL, inventory


class StockInventory(unittest.TestCase):
    def test_independent_inventory_and_unchanged_historical_stream(self):
        historical = reviewed_inventory(json.loads((ROOT / "seeds.json").read_text()))
        self.assertEqual(len(historical), 24)
        manifest = json.loads((ROOT / "stock-seeds.json").read_text())
        self.assertEqual(len(inventory(manifest)), 29)
        self.assertEqual(TARGETS, ("raster", "tiff_pages", "icons"))
        self.assertEqual(TOTAL, 7453)
        self.assertEqual(COUNTS, {"raster": 17, "tiff_pages": 6, "icons": 6})
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
                for row in altered["seeds"]:
                    if row["target"] == "icons":
                        row["accepted"] = False
            with self.subTest(change=change), self.assertRaises(ValueError):
                inventory(altered)


if __name__ == "__main__":
    unittest.main()
