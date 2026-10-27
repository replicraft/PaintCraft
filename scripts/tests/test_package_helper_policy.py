"""Exact target rollout and Rust target selection, without cross-target execution."""
import os
from pathlib import Path
import subprocess
import sys
import unittest
import tomllib

import test_install_cli as install_fixtures
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import codec_worker
import helper_policy as policy
import package_release as release


class PolicyTests(unittest.TestCase):
    def test_only_measured_mac_arm_target_opts_into_v8(self):
        manifest = tomllib.loads((release.ROOT / "apps/paintcraft-codec-worker/Cargo.toml").read_text())
        self.assertEqual(manifest["features"]["default"], ["float-codecs"])
        self.assertEqual(set(policy.ROLLOUT), set(release.TARGETS))
        for target in release.TARGETS:
            expected = (("jxr-codecs", codec_worker.JXR_PROFILE) if target == "aarch64-apple-darwin"
                        else ("float-codecs", codec_worker.FLOAT_PROFILE))
            self.assertEqual(policy.selection(target), expected)
        with self.assertRaises(ValueError):
            policy.selection("unknown-or-private")

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

    def test_v7_v8_publication_failure_and_success_in_both_directions(self):
        import install_cli
        for target in release.TARGETS:
            for old, new in (("v7", "v8"), ("v8", "v7")):
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
                            raise OSError("simulated V7/V8 final publication failure")
                        return replace(source, final)
                    with patch.object(install_cli.os, "replace", side_effect=fail_publish):
                        with self.assertRaisesRegex(OSError, "publication failure"):
                            self.install(new_archive, new_sums, prefix, target)
                    self.assertEqual({p.name: p.read_bytes() for p in destination.iterdir()}, before)
                    destination = self.install(new_archive, new_sums, prefix, target)
                    expected = codec_worker.JXR_PROFILE if new == "v8" else codec_worker.FLOAT_PROFILE
                    self.assertEqual(codec_worker.read_manifest(destination)["profile"], expected)
                    self.assertEqual({p.name for p in destination.iterdir()}, install_cli.payload_names(target, expected))
                    codec_worker.verify_installed_manifest(destination, self.version, target)


if __name__ == "__main__":
    unittest.main()
