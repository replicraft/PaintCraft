"""Exact target rollout and Rust target selection, without cross-target execution."""
import os
from pathlib import Path
import subprocess
import sys
import unittest
import tomllib
import tempfile

import test_install_cli as install_fixtures
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import codec_worker
import helper_policy as policy
import package_release as release


class PolicyTests(unittest.TestCase):
    def test_all_six_qualified_targets_and_default_enable_integer_jxl_and_jxr_writer(self):
        manifest = tomllib.loads((release.ROOT / "apps/paintcraft-codec-worker/Cargo.toml").read_text())
        self.assertEqual(manifest["features"]["default"], ["jxr-codecs", "jxl-integer-codec"])
        self.assertEqual(set(policy.ROLLOUT), set(release.TARGETS))
        for target in release.TARGETS:
            expected = ("jxr-codecs,jxl-integer-codec", codec_worker.JXL_INTEGER_JXR_PROFILE)
            self.assertEqual(policy.selection(target), expected)
            self.assertEqual(policy.selector(target), "v10")
        with self.assertRaises(ValueError):
            policy.selection("unknown-or-private")

    def test_github_outputs_bind_feature_profile_and_finite_selector(self):
        with tempfile.TemporaryDirectory(prefix="paintcraft-policy-test-") as temporary:
            output = Path(temporary) / "github-output"
            for target in release.TARGETS:
                output.write_text("")
                with patch.dict(os.environ, {"GITHUB_OUTPUT": str(output)}), \
                     patch.object(sys, "argv", ["helper_policy.py", "--target", target, "--github-output"]):
                    policy.main()
                values = dict(line.split("=", 1) for line in output.read_text().splitlines())
                feature, profile = policy.selection(target)
                self.assertEqual(values, {"feature": feature, "profile": profile,
                                          "selector": policy.selector(target)})

    def test_explicit_rust_target_wins_over_environment(self):
        with patch.dict(os.environ, {"CARGO_BUILD_TARGET": "aarch64-apple-darwin"}):
            self.assertEqual(policy.macos_target("x86_64-apple-darwin"), "x86_64-apple-darwin")
            self.assertEqual(policy.macos_target(), "aarch64-apple-darwin")
        with self.assertRaises(ValueError):
            policy.macos_target("x86_64-unknown-linux-gnu")

    def test_default_uses_rust_host_not_python_cpu(self):
        completed = subprocess.CompletedProcess([], 0, "rustc pinned\nhost: x86_64-apple-darwin\n", "")
        with patch.dict(os.environ, {}, clear=True), patch.object(policy.subprocess, "run", return_value=completed):
            self.assertEqual(policy.macos_target(), "x86_64-apple-darwin")
        completed.stdout += "host: aarch64-apple-darwin\n"
        with patch.dict(os.environ, {}, clear=True), patch.object(policy.subprocess, "run", return_value=completed):
            with self.assertRaises(ValueError):
                policy.macos_target()


class RollbackTests(unittest.TestCase):
    setUp = install_fixtures.InstallTests.setUp
    archive = install_fixtures.InstallTests.archive
    install = install_fixtures.InstallTests.install

    def test_historical_and_integer_promotions_roll_back_or_publish_complete_payloads(self):
        import install_cli
        for target in release.TARGETS:
            for old, new in (("v7", "v8"), ("v8", "v7"), ("v7", "v9"), ("v9", "v7"),
                             ("v8", "v10"), ("v10", "v8"), ("v9", "v10"), ("v10", "v9")):
                with self.subTest(target=target, old=old, new=new):
                    label = target + old + new
                    old_archive, old_sums = self.archive(target, label + "old", old)
                    new_archive, new_sums = self.archive(target, label + "new", new)
                    prefix = self.root / (label + "prefix")
                    destination = self.install(old_archive, old_sums, prefix, target)
                    before = {p.name: p.read_bytes() for p in destination.iterdir()}
                    replace = os.replace
                    def fail_publish(source, final):
                        if Path(source).name == "payload":
                            raise OSError("simulated final publication failure")
                        return replace(source, final)
                    with patch.object(install_cli.os, "replace", side_effect=fail_publish):
                        with self.assertRaisesRegex(OSError, "publication failure"):
                            self.install(new_archive, new_sums, prefix, target)
                    self.assertEqual({p.name: p.read_bytes() for p in destination.iterdir()}, before)
                    destination = self.install(new_archive, new_sums, prefix, target)
                    expected = {"v7": codec_worker.FLOAT_PROFILE, "v8": codec_worker.JXR_PROFILE,
                                "v9": codec_worker.JXL_INTEGER_PROFILE,
                                "v10": codec_worker.JXL_INTEGER_JXR_PROFILE}[new]
                    self.assertEqual(codec_worker.read_manifest(destination)["profile"], expected)
                    self.assertEqual({p.name for p in destination.iterdir()}, install_cli.payload_names(target, expected))
                    codec_worker.verify_installed_manifest(destination, self.version, target)


if __name__ == "__main__":
    unittest.main()
