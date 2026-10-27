"""Finite CI selection protects baseline coverage without implicit oracle prerequisites."""
from pathlib import Path
import re
import json
import tempfile
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import qualify_cli_helper as qualify
import package_release as release


class SelectionTests(unittest.TestCase):
    def test_v7_runs_ordinary_and_exact_six_specialist_cases(self):
        commands = qualify.commands(Path("/cached/test"), "v7")
        self.assertEqual(len(commands), 7)
        self.assertEqual(commands[0][-1], "--test-threads=1")
        for command, name in zip(commands[1:], qualify.BASELINE):
            self.assertEqual(command[-4:], [name, "--exact", "--ignored", "--test-threads=1"])
        self.assertFalse(any("jxr_export::" in argument for command in commands for argument in command))

    def test_v8_adds_two_actual_consumer_controls_or_explicit_oracle(self):
        self.assertEqual(qualify.selectors("v8"), qualify.BASELINE + qualify.JXR)
        self.assertEqual(qualify.selectors("v8", True), qualify.BASELINE + qualify.JXR + qualify.ORACLES)
        with self.assertRaises(ValueError):
            qualify.selectors("v7", True)
        with self.assertRaises(ValueError):
            qualify.selectors("private-v8")

    def test_every_exact_selector_resolves_to_one_real_ignored_test(self):
        directory = release.ROOT / "apps/paintcraft-cli/tests/installed_codec_worker"
        for name in qualify.selectors("v8", True):
            module, function = name.split("::")
            source = (directory / (module + ".rs")).read_text()
            matches = re.findall(r'#\[ignore[^\n]*\]\s*fn\s+' + re.escape(function) + r'\(', source)
            self.assertEqual(len(matches), 1, name)

    def test_inherited_fixture_bindings_cannot_override_selected_mode(self):
        inherited = {f"PAINTCRAFT_{family}_{kind}": "stale"
                     for family in ("QUALIFIED", "JXR", "V4", "V5", "V6", "V7", "V8")
                     for kind in ("HELPER", "INSTALLATION", "REFERENCE")}
        inherited.update({"PATH": "/toolchain", "RUSTUP_TOOLCHAIN": "pinned"})
        clean = qualify.fixture_environment(inherited)
        self.assertEqual(clean, {"PATH": "/toolchain", "RUSTUP_TOOLCHAIN": "pinned"})
        clean["PAINTCRAFT_QUALIFIED_HELPER"] = "/fresh/helper"
        clean["PAINTCRAFT_V7_HELPER"] = "/fresh/helper"
        self.assertNotIn("PAINTCRAFT_V7_INSTALLATION", clean)
        self.assertNotIn("PAINTCRAFT_JXR_REFERENCE", clean)
        self.assertEqual(clean["PAINTCRAFT_QUALIFIED_HELPER"], "/fresh/helper")
        self.assertEqual(inherited["PAINTCRAFT_V7_INSTALLATION"], "stale")

    def test_cargo_artifacts_and_cli_identity_are_bound(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            installed, built = root / "installed", root / "built"
            installed.write_bytes(b"current-cli")
            built.write_bytes(b"current-cli")
            qualify.bind_cli(installed, built)
            built.write_bytes(b"stale-or-feature-union-cli")
            with self.assertRaisesRegex(ValueError, "differs"):
                qualify.bind_cli(installed, built)
        records = [
            {"reason": "compiler-artifact", "target": {"name": "installed_codec_worker"},
             "profile": {"test": True}, "executable": "/test"},
            {"reason": "compiler-artifact", "target": {"name": "paintcraft-cli", "kind": ["bin"]},
             "executable": "/cli"},
        ]
        self.assertEqual(qualify.built_artifacts("\n".join(map(json.dumps, records))),
                         (Path("/test"), Path("/cli")))
        for invalid in [records[:1], records + [records[0] | {"executable": "/other-test"}]]:
            with self.assertRaises(ValueError):
                qualify.built_artifacts("\n".join(map(json.dumps, invalid)))
        command = qualify.build_command("aarch64-apple-darwin")
        self.assertIn("--no-run", command)
        self.assertIn("--release", command)

    def test_release_uses_finite_selector_and_keeps_exact_v7_feature(self):
        workflow = (release.ROOT / ".github/workflows/release.yml").read_text()
        self.assertIn("qualify_cli_helper.py --profile v7", workflow)
        self.assertIn("--features paintcraft-codec-worker/float-codecs", workflow)
        self.assertNotIn("--include-ignored", workflow)


if __name__ == "__main__":
    unittest.main()
