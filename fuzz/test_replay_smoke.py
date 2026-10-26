"""Reject false replay receipts and seed-inventory/provenance drift."""
import copy
import json
import unittest
from replay_smoke import ROOT, TARGETS, receipt_cases, reviewed_inventory, verify


class ReplayControls(unittest.TestCase):
    def test_inventory_preserves_five_existing_streams_and_adds_two_inspectors(self):
        manifest = json.loads((ROOT / "seeds.json").read_text())
        self.assertEqual(len(reviewed_inventory(manifest)), 24)
        self.assertEqual(TARGETS, ("avif", "heif", "paint", "pdn", "dcx", "radiance", "exr"))
        for change in ("schema", "count", "duplicate", "target", "distribution"):
            altered = copy.deepcopy(manifest)
            if change == "schema":
                altered["schema"] = "unreviewed"
            elif change == "count":
                altered["seeds"].pop()
            elif change == "duplicate":
                altered["seeds"][1] = altered["seeds"][0]
            elif change == "target":
                altered["seeds"][0]["target"] = "unreviewed"
            else:
                altered["seeds"][0]["target"] = "radiance"
            with self.subTest(change=change), self.assertRaises(ValueError):
                reviewed_inventory(altered)

    def test_source_identity_is_checked_before_replay(self):
        seed = json.loads((ROOT / "seeds.json").read_text())["seeds"][-1]
        for field, value in [("bytes", seed["bytes"] + 1), ("sha256", "0" * 64),
                             ("source", "../outside-the-repository")]:
            with self.subTest(field=field), self.assertRaises(ValueError):
                verify({**seed, field: value})

    def test_counts_positive_control_and_invariant_policy_are_required(self):
        # These synthetic counts exercise the validator, not parser measurements.
        valid = ("target=Radiance cases=1028 accepted=3 rejected=1025 "
                 "seed=0x5041494e54435241 mode=deterministic-replay policy=invariants-fail-v1\n")
        self.assertEqual(receipt_cases("radiance", 4, valid), 1028)
        for output in [valid.replace("Radiance", "Exr"), valid.replace("1028", "1027"),
                       valid.replace("accepted=3", "accepted=0"),
                       valid.replace("rejected=1025", "rejected=1"),
                       valid.replace(" policy=invariants-fail-v1", "")]:
            with self.subTest(output=output), self.assertRaises(ValueError):
                receipt_cases("radiance", 4, output)


if __name__ == "__main__":
    unittest.main()
