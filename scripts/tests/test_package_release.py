"""Archive, architecture and checksum contracts, without running fixture binaries."""
import contextlib
import hashlib
import io
from pathlib import Path
import plistlib
import struct
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import package_release as release
import package_macos as macos


def executable(path, target):
    if "apple" in target:
        cpu = 0x0100000C if target.startswith("aarch64") else 0x01000007
        header = struct.pack("<II", 0xFEEDFACF, cpu) + bytes(56)
    elif "windows" in target:
        header = bytearray(64)
        header[:2] = b"MZ"
        struct.pack_into("<I", header, 60, 64)
        cpu = 0xAA64 if target.startswith("aarch64") else 0x8664
        header += b"PE\0\0" + struct.pack("<H", cpu)
    else:
        header = bytearray(64)
        header[:6] = b"\x7fELF\x02\x01"
        header[7] = 9 if "freebsd" in target else 0
        struct.pack_into("<H", header, 18, 183 if target.startswith("aarch64") else 62)
    path.write_bytes(header)
    return path


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="paintcraft-release-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.version = release.workspace_version()
        self.output = self.root / "output with spaces"
        self.output.mkdir()
        self.binary = self.root / "input with spaces"

    def full_set(self):
        for name in release.expected_assets(self.version):
            path = self.output / name
            path.write_bytes(b"checksum fixture " + name.encode())
            release.write_checksum(path)

    def test_tag_and_workspace_version_must_match_and_github_output_is_lf(self):
        output = self.root / "github-output"
        with patch.dict("os.environ", {"GITHUB_OUTPUT": str(output)}), contextlib.redirect_stdout(io.StringIO()):
            release.validate("v" + self.version)
            with self.assertRaises(ValueError):
                release.validate("v99.0.0")
        self.assertEqual(output.read_bytes(), f"version={self.version}\n".encode())

    def test_every_executable_header_rejects_every_other_target(self):
        for actual in release.TARGETS:
            executable(self.binary, actual)
            release.validate_package_input(self.version, actual, self.binary)
            for claimed in release.TARGETS:
                if claimed != actual:
                    with self.assertRaises(ValueError, msg=f"{actual} mislabeled {claimed}"):
                        release.validate_package_input(self.version, claimed, self.binary)

    def test_truncated_binary_and_wrong_version_fail_before_any_output(self):
        for data in (b"", b"MZ", b"\x7fELF", b"not a binary"):
            self.binary.write_bytes(data)
            for target in release.TARGETS:
                with self.assertRaises(ValueError):
                    release.package(self.version, target, self.binary, self.output)
        with self.assertRaises(ValueError):
            release.package("999.0.0", next(iter(release.TARGETS)), self.binary, self.output)
        self.assertEqual(list(self.output.iterdir()), [])

    def test_portable_archives_preserve_only_binary_license_and_executable_bits(self):
        for target, suffix in release.TARGETS.items():
            if "apple" in target:
                continue
            executable(self.binary, target)
            release.package(self.version, target, self.binary, self.output)
            name = f"paintcraft-{self.version}-{suffix}"
            if "windows" in target:
                archive = self.output / (name + "-portable.zip")
                with zipfile.ZipFile(archive) as stream:
                    self.assertEqual(set(stream.namelist()), {"paintcraft.exe", "LICENSE", "FONT_LICENSES.txt"})
                    self.assertEqual(stream.read("paintcraft.exe"), self.binary.read_bytes())
                self.assertEqual((self.output / (name + ".exe")).read_bytes(), self.binary.read_bytes())
            else:
                archive = self.output / (name + ".tar.gz")
                with tarfile.open(archive) as stream:
                    self.assertEqual(set(stream.getnames()), {"paintcraft", "LICENSE", "FONT_LICENSES.txt"})
                    self.assertEqual(stream.extractfile("paintcraft").read(), self.binary.read_bytes())
                    self.assertEqual(stream.getmember("paintcraft").mode & 0o777, 0o755)
            self.assertEqual(archive.with_name(archive.name + ".sha256").read_bytes(),
                             f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n".encode())

    def test_update_zip_keeps_whole_signed_layout_posix_paths_and_permissions(self):
        binary = executable(self.binary, "aarch64-apple-darwin")
        app = self.root / "PaintCraft.app"
        macos.populate_bundle(binary, app, self.version, binary)
        signature = app / "Contents/_CodeSignature/CodeResources"
        signature.parent.mkdir()
        signature.write_bytes(b"signature fixture")
        archive = self.output / "update.zip"
        release.write_update_archive(app, archive)
        with zipfile.ZipFile(archive) as stream:
            names = stream.namelist()
            self.assertTrue(all(name.startswith("PaintCraft.app/") and "\\" not in name for name in names))
            for entry in app.rglob("*"):
                if entry.is_file():
                    name = entry.relative_to(app.parent).as_posix()
                    self.assertEqual(stream.read(name), entry.read_bytes())
                    self.assertEqual(stream.getinfo(name).external_attr >> 16 & 0o777, entry.stat().st_mode & 0o777)
            info = plistlib.loads(stream.read("PaintCraft.app/Contents/Info.plist"))
            self.assertEqual(info["CFBundleExecutable"], "paintcraft-launcher")
            self.assertIn("PaintCraft.app/Contents/Helpers/PaintCraft Editor.app/Contents/MacOS/paintcraft", names)

    def test_update_zip_refuses_external_symlinks(self):
        app = self.root / "PaintCraft.app"
        app.mkdir()
        try:
            (app / "external").symlink_to(self.binary)
        except OSError:
            self.skipTest("symlink privilege is unavailable")
        with self.assertRaises(ValueError):
            release.write_update_archive(app, self.output / "update.zip")

    def test_final_manifest_requires_all_19_assets_and_independently_matches_hashes(self):
        self.assertEqual(len(release.expected_assets(self.version)), 19)
        self.full_set()
        release.verify_assets(self.version, self.output, finalize=True)
        lines = (self.output / "SHA256SUMS.txt").read_bytes().splitlines()
        self.assertEqual(len(lines), 19)
        self.assertFalse(list(self.output.glob("*.sha256")))
        for line in lines:
            digest, name = line.decode().split("  ")
            self.assertEqual(digest, hashlib.sha256((self.output / name).read_bytes()).hexdigest())
        self.assertEqual([line.split(b"  ")[1] for line in lines], sorted(line.split(b"  ")[1] for line in lines))
        release.verify_assets(self.version, self.output)

    def test_missing_intel_or_arm_asset_cannot_publish_a_partial_release(self):
        for arch in ("macos-x64.dmg", "windows-arm64.msi", "linux-arm64.rpm", "freebsd-x64.tar.gz"):
            with self.subTest(arch=arch):
                self.full_set()
                missing = next(p for p in self.output.iterdir() if p.name.endswith(arch))
                missing.unlink()
                with self.assertRaisesRegex(ValueError, "missing="):
                    release.verify_assets(self.version, self.output, finalize=True)
                self.assertFalse((self.output / "SHA256SUMS.txt").exists())

    def test_corruption_and_stale_extra_assets_fail_before_manifest_publication(self):
        self.full_set()
        asset = next(p for p in self.output.iterdir() if p.suffix == ".msi")
        original = asset.read_bytes()
        asset.write_bytes(b"tampered")
        with self.assertRaisesRegex(ValueError, "Checksum"):
            release.verify_assets(self.version, self.output, finalize=True)
        asset.write_bytes(original)
        (self.output / "stale.exe").write_bytes(b"stale")
        with self.assertRaisesRegex(ValueError, "unexpected="):
            release.verify_assets(self.version, self.output, finalize=True)
        self.assertFalse((self.output / "SHA256SUMS.txt").exists())

    def test_modified_final_manifest_and_payload_fail_verification(self):
        self.full_set()
        release.verify_assets(self.version, self.output, finalize=True)
        manifest = self.output / "SHA256SUMS.txt"
        original = manifest.read_bytes()
        manifest.write_bytes(original.replace(b"\n", b"\r\n"))
        with self.assertRaises(ValueError):
            release.verify_assets(self.version, self.output)
        manifest.write_bytes(original)
        next(self.output.glob("*.dmg")).write_bytes(b"changed")
        with self.assertRaises(ValueError):
            release.verify_assets(self.version, self.output)

    def test_finalization_never_writes_through_a_manifest_symlink(self):
        self.full_set()
        sentinel = self.root / "untouched"
        sentinel.write_bytes(b"original")
        try:
            (self.output / "SHA256SUMS.txt").symlink_to(sentinel)
        except OSError:
            self.skipTest("symlink privilege is unavailable")
        with self.assertRaisesRegex(ValueError, "symlink"):
            release.verify_assets(self.version, self.output, finalize=True)
        self.assertEqual(sentinel.read_bytes(), b"original")


if __name__ == "__main__":
    unittest.main()
