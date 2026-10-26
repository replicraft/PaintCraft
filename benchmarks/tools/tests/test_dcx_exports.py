"""Delivery driver cleanup/measurement-boundary controls, no product benchmarks."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from run_dcx_exports import one


class Delivery(unittest.TestCase):
    def test_success_failure_and_timeout_destroy_only_the_owned_sample_output(self):
        for outcome in ["ok", "failure", "timeout"]:
            with self.subTest(outcome=outcome), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                (root / "work").mkdir()
                output = root / "work/sample.dcx"
                source = root / "source.pcnd"
                source.write_bytes(b"preserve source")
                manifest = root / "manifest.json"
                manifest.write_text(json.dumps({"schema": "paintcraft.dcx-delivery/1", "cli": "cli", "cases": [
                    {"id": "sample", "project": str(source), "project_sha256": "f" * 64,
                     "mode": "rgb24", "output": str(output)}]}))

                def invoke(argv, **kwargs):
                    self.assertIn("--expect-input-sha256", argv)
                    self.assertEqual(kwargs["timeout"], 30)
                    output.write_bytes(b"x" * 4101)
                    if outcome == "timeout":
                        raise subprocess.TimeoutExpired(argv, 30)
                    if outcome == "failure":
                        raise subprocess.CalledProcessError(1, argv)
                    return SimpleNamespace(stdout='{"outcome":"success"}')

                with patch("run_dcx_exports.subprocess.run", side_effect=invoke):
                    if outcome == "ok":
                        one(manifest, "sample")
                    else:
                        with self.assertRaises(subprocess.SubprocessError):
                            one(manifest, "sample")
                self.assertFalse(output.exists())
                self.assertEqual(source.read_bytes(), b"preserve source")

    def test_existing_output_is_refused_and_not_removed(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "work").mkdir()
            output = root / "work/sample.dcx"
            output.write_bytes(b"existing")
            manifest = root / "manifest.json"
            manifest.write_text(json.dumps({"schema": "paintcraft.dcx-delivery/1", "cli": "cli", "cases": [
                {"id": "sample", "output": str(output)}]}))
            with self.assertRaises(AssertionError):
                one(manifest, "sample")
            self.assertEqual(output.read_bytes(), b"existing")


if __name__ == "__main__":
    unittest.main()
