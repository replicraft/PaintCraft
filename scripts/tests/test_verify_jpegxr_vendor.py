"""Exact source verification is a prerequisite to twelve inherited exceptions."""

import contextlib
import io
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import check_file_lengths as lengths
import verify_jpegxr_vendor as vendor


class VendorVerification(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="paintcraft-jxr-verification-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / vendor.VENDOR
        shutil.copytree(vendor.ROOT / vendor.VENDOR, self.source)
        for suffix in [".patch", ".provenance.json"]:
            source = Path(str(vendor.ROOT / vendor.VENDOR) + suffix)
            shutil.copyfile(source, Path(str(self.source) + suffix))
        shutil.copyfile(vendor.ROOT / "vendor/.gitattributes",
                        self.root / "vendor/.gitattributes")
        (self.root / "packaging").mkdir()
        shutil.copyfile(vendor.ROOT / "packaging/JPEGXR_LICENSE.txt",
                        self.root / "packaging/JPEGXR_LICENSE.txt")

    def test_only_twelve_known_files_are_exempt(self):
        result = vendor.verify(self.root)
        self.assertEqual(result, {
            f"{vendor.VENDOR}/{name}" for name in vendor.INHERITED_LONG_FILES
        })
        self.assertEqual(len(result), 12)
        self.assertNotIn(f"{vendor.VENDOR}/src/api.rs", result)

    def test_changed_exempt_source_fails_before_exception(self):
        path = self.source / "src/image/sys/strcodec.rs"
        path.write_bytes(path.read_bytes() + b"\n")
        with self.assertRaisesRegex(ValueError, "source size mismatch"):
            vendor.verify(self.root)

    def test_unknown_source_cannot_hide_in_vendor(self):
        (self.source / "unexpected.rs").write_text("// owned\n" * 801)
        with self.assertRaisesRegex(ValueError, "missing/unexpected"):
            vendor.verify(self.root)

    def test_unknown_exception_is_refused(self):
        path = Path(str(self.source) + ".provenance.json")
        metadata = json.loads(path.read_text())
        metadata["line_exemptions"].append("unexpected.rs")
        path.write_text(json.dumps(metadata))
        with self.assertRaisesRegex(ValueError, "source-line exception"):
            vendor.verify(self.root)

    def test_patch_bytes_must_match_reviewed_checksum(self):
        path = Path(str(self.source) + ".patch")
        path.write_bytes(path.read_bytes() + b"\n")
        with self.assertRaisesRegex(ValueError, "patch checksum"):
            vendor.verify(self.root)

    def test_packaged_license_must_retain_upstream_notice(self):
        (self.root / "packaging/JPEGXR_LICENSE.txt").write_text("incorrect notice")
        with self.assertRaisesRegex(ValueError, "notice differs"):
            vendor.verify(self.root)

    def test_symlink_is_not_a_verified_source(self):
        target = self.source / "README.md"
        expected = target.read_bytes()
        target.unlink()
        alternate = self.root / "README-copy.md"
        alternate.write_bytes(expected)
        try:
            target.symlink_to(alternate)
        except OSError as error:
            self.skipTest(f"Host cannot create symlinks: {error}")
        with self.assertRaisesRegex(ValueError, "symlink"):
            vendor.verify(self.root)

    def test_first_party_limit_is_not_loosened(self):
        path = self.root / "application.rs"
        for count, expected in [(800, 0), (801, 1)]:
            path.write_text("// first-party source\n" * count)
            with (mock.patch.object(lengths, "ROOT", self.root),
                  mock.patch.object(lengths.subprocess, "check_output",
                                    return_value=b"application.rs\0"),
                  contextlib.redirect_stdout(io.StringIO())):
                self.assertEqual(lengths.main(), expected)

    def test_line_checker_fails_closed_for_bad_vendor(self):
        (self.source / "src/lib.rs").write_text("// replaced upstream\n")
        with (mock.patch.object(lengths, "ROOT", self.root),
              contextlib.redirect_stdout(io.StringIO())):
            self.assertEqual(lengths.main(), 1)

    def test_git_blobs_and_checkouts_keep_exact_bytes(self):
        # A disposable Git index only: never stage or write the real repository.
        def git(*args):
            return subprocess.check_output(
                ["git", *args], cwd=self.root, stderr=subprocess.STDOUT,
            )

        git("init", "--quiet")
        metadata = json.loads(Path(str(self.source) + ".provenance.json").read_text())
        expected = {
            f"{vendor.VENDOR}/{name}": row["sha256"]
            for name, row in metadata["files"].items()
        }
        for suffix in [".patch", ".provenance.json"]:
            relative = vendor.VENDOR + suffix
            expected[relative] = vendor.digest((self.root / relative).read_bytes())
        for mode in ["true", "input", "false"]:
            git("-c", f"core.autocrlf={mode}", "add", "--", "vendor")
            git("diff", "--cached", "--check", "--", "vendor")
            output = self.root / "checkouts" / mode
            output.mkdir(parents=True)
            git("-c", f"core.autocrlf={mode}", "checkout-index",
                "--prefix=" + output.as_posix() + "/", "--all")
            for relative, checksum in expected.items():
                stored = git("show", ":" + relative)
                self.assertEqual(vendor.digest(stored), checksum, (mode, relative))
                self.assertEqual(vendor.digest((output / relative).read_bytes()),
                                 checksum, (mode, relative))


if __name__ == "__main__":
    unittest.main()
