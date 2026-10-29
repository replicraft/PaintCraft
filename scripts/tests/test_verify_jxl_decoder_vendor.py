"""The decoder's exact upstream bytes never grant blanket authored exemptions."""
import contextlib
import io
import json
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check_file_lengths as lengths
import verify_jxl_decoder_vendor as vendor


class JxlDecoderVerification(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="paintcraft-jxl-decoder-vendor-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.vendors = [vendor.VENDOR, "vendor/zune-jpegxl-0.5.2-paintcraft1",
                        "vendor/jpegxr-pure-rs-0.1.0-paintcraft1"]
        for name in self.vendors:
            shutil.copytree(vendor.ROOT / name, self.root / name)
            for suffix in [".patch", ".provenance.json"]:
                shutil.copyfile(vendor.ROOT / (name + suffix), self.root / (name + suffix))
        shutil.copyfile(vendor.ROOT / (vendor.VENDOR + ".README.md"),
                        self.root / (vendor.VENDOR + ".README.md"))
        shutil.copyfile(vendor.ROOT / "vendor/.gitattributes", self.root / "vendor/.gitattributes")
        (self.root / "packaging").mkdir()
        shutil.copyfile(vendor.ROOT / "packaging/JPEGXR_LICENSE.txt",
                        self.root / "packaging/JPEGXR_LICENSE.txt")
        self.source = self.root / vendor.VENDOR
        self.provenance = Path(str(self.source) + ".provenance.json")

    def test_only_eighteen_unchanged_upstream_long_files_are_exempt(self):
        actual = vendor.verify(self.root)
        self.assertEqual(actual, {vendor.VENDOR + "/" + name for name in vendor.LONG_FILES})
        self.assertEqual(len(actual), 18)
        for name in vendor.CHANGED_FILES + ["rustfmt.toml"]:
            self.assertNotIn(vendor.VENDOR + "/" + name, actual)

    def test_modified_or_missing_inherited_source_is_refused(self):
        for name in [sorted(vendor.LONG_FILES)[0], "src/lib.rs", *vendor.CHANGED_FILES]:
            path = self.source / name
            original = path.read_bytes()
            for replacement in [None, original + b"\n", b"replacement"]:
                with self.subTest(name=name, replacement=replacement is None):
                    if replacement is None:
                        path.unlink()
                    else:
                        path.write_bytes(replacement)
                    with self.assertRaises(ValueError):
                        vendor.verify(self.root)
                    path.write_bytes(original)

    def test_unknown_source_and_empty_directory_are_refused(self):
        path = self.source / "authored.rs"
        path.write_text("// authored\n" * 801)
        with self.assertRaisesRegex(ValueError, "missing/unexpected"):
            vendor.verify(self.root)
        path.unlink()
        (self.source / "extra-directory").mkdir()
        with self.assertRaisesRegex(ValueError, "missing/unexpected"):
            vendor.verify(self.root)

    def test_modified_or_missing_patch_readme_license_and_provenance_are_refused(self):
        paths = [Path(str(self.source) + ".patch"), Path(str(self.source) + ".README.md"),
                 self.source / "LICENSE", self.provenance]
        for path in paths:
            original = path.read_bytes()
            for replacement in [None, original + b"\n"]:
                with self.subTest(path=path.name, missing=replacement is None):
                    if replacement is None:
                        path.unlink()
                    else:
                        path.write_bytes(replacement)
                    with self.assertRaises(ValueError):
                        vendor.verify(self.root)
                    path.write_bytes(original)

    def test_edited_inventory_cannot_resign_changed_source_or_authored_scope(self):
        original = self.provenance.read_bytes()
        for mutation in ["source", "changed", "exemption", "authored", "license", "identity"]:
            metadata = json.loads(original)
            if mutation == "source":
                metadata["files"]["src/lib.rs"]["sha256"] = "0" * 64
            elif mutation == "changed":
                metadata["changed_files"].append("src/lib.rs")
            elif mutation == "exemption":
                metadata["line_exemptions"].append("src/lib.rs")
            elif mutation == "authored":
                metadata["authored_files"]["extra.rs"] = metadata["authored_files"]["rustfmt.toml"]
            elif mutation == "license":
                metadata["licenses"] = {}
            else:
                metadata["upstream_commit"] = "0" * 40
            self.provenance.write_text(json.dumps(metadata))
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                vendor.verify(self.root)
        self.provenance.write_bytes(original)

    def test_unsafe_inventory_paths_are_rejected_before_source_access(self):
        original = self.provenance.read_bytes()
        for name in ["../outside.rs", "/absolute.rs", "src/../outside.rs", "src\\outside.rs",
                     "C:outside.rs", "src//outside.rs", ".", "src/./outside.rs"]:
            metadata = json.loads(original)
            metadata["files"][name] = metadata["files"].pop("src/lib.rs")
            self.provenance.write_text(json.dumps(metadata))
            with self.subTest(path=name), self.assertRaisesRegex(ValueError, "unsafe inventory path"):
                vendor.verify(self.root)
        self.provenance.write_bytes(original)

    def test_duplicate_provenance_fields_and_formatting_changes_are_refused(self):
        original = self.provenance.read_bytes()
        self.provenance.write_bytes(original.replace(b'"schema": 1', b'"schema": 1, "schema": 1'))
        with self.assertRaisesRegex(ValueError, "duplicate provenance field"):
            vendor.verify(self.root)
        self.provenance.write_bytes(original)
        (self.source / "rustfmt.toml").write_bytes(b"disable_all_formatting = false\n")
        with self.assertRaises(ValueError):
            vendor.verify(self.root)

    def test_symlinked_source_metadata_license_or_directory_is_refused(self):
        for path in [self.source / "src/lib.rs", self.source / "LICENSE", self.provenance,
                     Path(str(self.source) + ".patch"), Path(str(self.source) + ".README.md"), self.source]:
            parked = path.with_name(path.name + ".parked")
            path.rename(parked)
            try:
                path.symlink_to(parked, target_is_directory=parked.is_dir())
            except OSError as error:
                parked.rename(path)
                self.skipTest(f"Host cannot create symlinks: {error}")
            with self.subTest(path=path.name), self.assertRaisesRegex(ValueError, "symlink"):
                vendor.verify(self.root)
            path.unlink()
            parked.rename(path)

    def test_exact_checkout_attributes_remain_required(self):
        (self.root / "vendor/.gitattributes").write_text("")
        with self.assertRaisesRegex(ValueError, "checkout attributes missing"):
            vendor.verify(self.root)

    def test_generic_checker_keeps_first_party_limit_and_every_vendor_required(self):
        path = self.root / "application.rs"
        for count, expected in [(800, 0), (801, 1)]:
            path.write_text("// authored\n" * count)
            with (mock.patch.object(lengths, "ROOT", self.root),
                  mock.patch.object(lengths.subprocess, "check_output", return_value=b"application.rs\0"),
                  contextlib.redirect_stdout(io.StringIO())):
                self.assertEqual(lengths.main(), expected)
        for name in self.vendors:
            target = self.root / name
            parked = target.with_name(target.name + ".parked")
            target.rename(parked)
            with mock.patch.object(lengths, "ROOT", self.root), contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(lengths.main(), 1)
            parked.rename(target)

    def upstream_archive(self):
        paths = list(Path.home().glob(".cargo/registry/cache/*/jxl-0.8.0.crate"))
        if not paths:
            self.skipTest("Original release archive is not cached; use verifier --archive explicitly")
        return paths[0]

    def test_original_release_archive_and_complete_patch_delta_verify(self):
        archive = self.upstream_archive()
        self.assertEqual(vendor.verify(self.root, archive),
                         {vendor.VENDOR + "/" + name for name in vendor.LONG_FILES})
        changed = self.root / "changed.crate"
        data = bytearray(archive.read_bytes())
        data[len(data) // 2] ^= 1
        changed.write_bytes(data)
        with self.assertRaisesRegex(ValueError, "archive checksum"):
            vendor.verify(self.root, changed)
        metadata = json.loads(self.provenance.read_text())
        patch = Path(str(self.source) + ".patch").read_bytes()
        with self.assertRaisesRegex(ValueError, "actual archive-to-vendor delta"):
            vendor.verify_archive(archive, self.source, metadata, patch + b"\n")
        changed_source = self.source / vendor.CHANGED_FILES[0]
        original = changed_source.read_bytes()
        changed_source.write_bytes(original.replace(b"\n", b"\r\n"))
        with self.assertRaisesRegex(ValueError, "actual archive-to-vendor delta"):
            vendor.verify_archive(archive, self.source, metadata, patch)
        changed_source.write_bytes(original)

    def test_archive_paths_symlinks_duplicates_and_unknown_files_are_refused(self):
        archive = self.root / "hostile.crate"
        metadata = json.loads(self.provenance.read_text())
        patch = Path(str(self.source) + ".patch").read_bytes()
        cases = [[("../escape", b"x", tarfile.REGTYPE)],
                 [("jxl-0.8.0/../escape", b"x", tarfile.REGTYPE)],
                 [("jxl-0.8.0/source", b"", tarfile.SYMTYPE)],
                 [("jxl-0.8.0/unknown.rs", b"x", tarfile.REGTYPE)],
                 [("jxl-0.8.0/unknown", b"", tarfile.DIRTYPE)]]
        license_bytes = (self.source / "LICENSE").read_bytes()
        cases.append([("jxl-0.8.0/LICENSE", license_bytes, tarfile.REGTYPE)] * 2)
        for entries in cases:
            with tarfile.open(archive, "w") as output:
                for name, data, kind in entries:
                    member = tarfile.TarInfo(name)
                    member.type = kind
                    member.size = len(data)
                    output.addfile(member, io.BytesIO(data))
            with (mock.patch.object(vendor, "ARCHIVE_BYTES", archive.stat().st_size),
                  mock.patch.object(vendor, "ARCHIVE_SHA256", vendor.digest(archive.read_bytes())),
                  self.subTest(name=entries[0][0]), self.assertRaises(ValueError)):
                vendor.verify_archive(archive, self.source, metadata, patch)

    def test_git_index_and_checkout_preserve_every_decoder_byte(self):
        def git(*args):
            result = subprocess.run(["git", *args], cwd=self.root, stdout=subprocess.PIPE,
                                    stderr=subprocess.STDOUT, check=False)
            self.assertEqual(result.returncode, 0, result.stdout.decode(errors="replace"))
            return result.stdout
        git("init", "--quiet")
        metadata = json.loads(self.provenance.read_text())
        expected = {vendor.VENDOR + "/" + name: row["sha256"]
                    for name, row in (metadata["files"] | metadata["authored_files"]).items()}
        for suffix in [".patch", ".provenance.json", ".README.md"]:
            name = vendor.VENDOR + suffix
            expected[name] = vendor.digest((self.root / name).read_bytes())
        for mode in ["true", "input", "false"]:
            git("-c", f"core.autocrlf={mode}", "add", "--", "vendor")
            git("diff", "--cached", "--check", "--", "vendor")
            output = self.root / "checkout" / mode
            output.mkdir(parents=True)
            git("-c", f"core.autocrlf={mode}", "checkout-index",
                "--prefix=" + output.as_posix() + "/", "--all")
            for name, sha in expected.items():
                self.assertEqual(vendor.digest(git("show", ":" + name)), sha)
                self.assertEqual(vendor.digest((output / name).read_bytes()), sha)


if __name__ == "__main__":
    unittest.main()
