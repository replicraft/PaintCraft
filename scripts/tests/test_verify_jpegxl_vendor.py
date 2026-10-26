"""Selected JPEG XL bytes remain a prerequisite for one inherited line exception."""
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

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check_file_lengths as lengths
import verify_jpegxl_vendor as vendor


class JpegXlVerification(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="paintcraft-jxl-vendor-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        for name in [vendor.VENDOR, "vendor/jpegxr-pure-rs-0.1.0-paintcraft1"]:
            shutil.copytree(vendor.ROOT / name, self.root / name)
            for suffix in [".patch", ".provenance.json"]:
                shutil.copyfile(vendor.ROOT / (name + suffix), self.root / (name + suffix))
        shutil.copyfile(vendor.ROOT / "vendor/.gitattributes", self.root / "vendor/.gitattributes")
        (self.root / "packaging").mkdir()
        shutil.copyfile(vendor.ROOT / "packaging/JPEGXR_LICENSE.txt",
                        self.root / "packaging/JPEGXR_LICENSE.txt")
        self.source = self.root / vendor.VENDOR

    def test_only_exact_encoder_is_exempt(self):
        self.assertEqual(vendor.verify(self.root), {vendor.VENDOR + "/src/encoder.rs"})

    def test_changed_exempt_source_is_refused(self):
        path = self.source / "src/encoder.rs"
        path.write_bytes(path.read_bytes() + b"\n")
        with self.assertRaisesRegex(ValueError, "source size mismatch"):
            vendor.verify(self.root)

    def test_source_cannot_be_resigned_with_edited_inventory(self):
        path = self.source / "src/encoder.rs"
        path.write_bytes(path.read_bytes().replace(b"size >= 1", b"size >= 2"))
        record = Path(str(self.source) + ".provenance.json")
        metadata = json.loads(record.read_text())
        metadata["files"]["src/encoder.rs"]["sha256"] = vendor.digest(path.read_bytes())
        record.write_text(json.dumps(metadata))
        with self.assertRaisesRegex(ValueError, "reviewed inventory mismatch"):
            vendor.verify(self.root)

    def test_preservation_config_is_exact_and_local(self):
        (self.source / "rustfmt.toml").write_text("disable_all_formatting = false\n")
        with self.assertRaisesRegex(ValueError, "formatting policy mismatch"):
            vendor.verify(self.root)

    def test_unknown_source_is_not_exempt(self):
        (self.source / "unexpected.rs").write_text("// authored\n" * 801)
        with self.assertRaisesRegex(ValueError, "missing/unexpected"):
            vendor.verify(self.root)

    def test_missing_original_license_is_refused(self):
        (self.source / "LICENSE-ZLIB").unlink()
        with self.assertRaisesRegex(ValueError, "missing/unexpected"):
            vendor.verify(self.root)

    def test_extra_exception_and_corrupt_patch_are_refused(self):
        record = Path(str(self.source) + ".provenance.json")
        metadata = json.loads(record.read_text())
        metadata["line_exemptions"].append("src/lib.rs")
        record.write_text(json.dumps(metadata))
        with self.assertRaisesRegex(ValueError, "scope mismatch"):
            vendor.verify(self.root)
        metadata["line_exemptions"].pop()
        record.write_text(json.dumps(metadata))
        patch = Path(str(self.source) + ".patch")
        patch.write_bytes(patch.read_bytes() + b"\n")
        with self.assertRaisesRegex(ValueError, "patch bytes mismatch"):
            vendor.verify(self.root)

    def test_symlinks_cannot_pass_as_source(self):
        path = self.source / "README.md"
        alternate = self.root / "copy.md"
        path.rename(alternate)
        try:
            path.symlink_to(alternate)
        except OSError as error:
            self.skipTest(f"Host cannot create symlinks: {error}")
        with self.assertRaisesRegex(ValueError, "symlink"):
            vendor.verify(self.root)

    def test_missing_attributes_are_refused(self):
        (self.root / "vendor/.gitattributes").write_text("")
        with self.assertRaisesRegex(ValueError, "checkout attributes missing"):
            vendor.verify(self.root)

    def test_generic_checker_keeps_first_party_limit_and_both_vendors_required(self):
        path = self.root / "application.rs"
        for count, expected in [(800, 0), (801, 1)]:
            path.write_text("// authored\n" * count)
            with (mock.patch.object(lengths, "ROOT", self.root),
                  mock.patch.object(lengths.subprocess, "check_output",
                                    return_value=b"application.rs\0"),
                  contextlib.redirect_stdout(io.StringIO())):
                self.assertEqual(lengths.main(), expected)
        path.write_text("// authored\n")
        for name in [vendor.VENDOR, "vendor/jpegxr-pure-rs-0.1.0-paintcraft1"]:
            target = self.root / name
            parked = target.with_name(target.name + ".parked")
            target.rename(parked)
            with (mock.patch.object(lengths, "ROOT", self.root),
                  contextlib.redirect_stdout(io.StringIO())):
                self.assertEqual(lengths.main(), 1)
            parked.rename(target)

    def test_git_index_and_checkout_preserve_every_reviewed_byte(self):
        def git(*args):
            return subprocess.check_output(["git", *args], cwd=self.root,
                                           stderr=subprocess.STDOUT)
        git("init", "--quiet")
        metadata = json.loads(Path(str(self.source) + ".provenance.json").read_text())
        expected = {vendor.VENDOR + "/" + name: row["sha256"]
                    for name, row in (metadata["files"] | metadata["authored_files"]).items()}
        for suffix in [".patch", ".provenance.json"]:
            name = vendor.VENDOR + suffix
            expected[name] = vendor.digest((self.root / name).read_bytes())
        for mode in ["true", "input", "false"]:
            git("-c", f"core.autocrlf={mode}", "add", "--", "vendor")
            git("diff", "--cached", "--check", "--", "vendor")
            output = self.root / "checkout" / mode
            output.mkdir(parents=True)
            git("-c", f"core.autocrlf={mode}", "checkout-index",
                "--prefix=" + output.as_posix() + "/", "--all")
            for name, expected_sha in expected.items():
                self.assertEqual(vendor.digest(git("show", ":" + name)), expected_sha)
                self.assertEqual(vendor.digest((output / name).read_bytes()), expected_sha)


if __name__ == "__main__":
    unittest.main()
