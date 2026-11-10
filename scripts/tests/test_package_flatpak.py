"""Flatpak payload and release contracts without launching fixture executables."""
import hashlib
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import package_flatpak as flatpak
import package_release as release
import codec_worker


def elf(path, target):
    header = bytearray(64)
    header[:6] = b"\x7fELF\x02\x01"
    struct.pack_into("<H", header, 18, 183 if target.startswith("aarch64") else 62)
    if path.name == "paintcraft-codec-worker":
        header += codec_worker.BUILD_PREFIX + release.workspace_version().encode() + codec_worker.BUILD_SUFFIX
    path.write_bytes(header)
    return path


class FlatpakTests(unittest.TestCase):
    def test_manifest_has_exact_runtime_and_portal_permissions_without_host_filesystem_access(self):
        config = flatpak.manifest()
        self.assertEqual(config["app-id"], flatpak.APP_ID)
        self.assertEqual(config["runtime-version"], "25.08")
        self.assertEqual(config["command"], "paintcraft")
        self.assertEqual(config["finish-args"],
                         ["--share=network", "--share=ipc", "--socket=wayland", "--socket=fallback-x11", "--device=dri"])
        self.assertEqual(config["build-options"], {"strip": False, "no-debuginfo": True})
        self.assertEqual(config["modules"][0]["sources"],
                         [{"type": "dir", "path": "payload", "dest": "payload"}])

    def test_both_architectures_install_desktop_cli_exact_helper_and_consistent_ids(self):
        for target, arch in flatpak.ARCHES.items():
            with self.subTest(arch=arch), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                binary = elf(root / "paintcraft", target)
                cli = elf(root / "paintcraft-cli", target)
                helper = elf(root / "paintcraft-codec-worker", target)
                helper_bytes = helper.read_bytes()
                payload = root / "build/files"
                hashes = flatpak.stage(release.workspace_version(), target, binary, cli, payload)
                self.assertEqual((payload / "bin/paintcraft-codec-worker").read_bytes(), helper_bytes)
                self.assertEqual(hashes[helper.name], hashlib.sha256(helper_bytes).hexdigest())
                self.assertEqual((payload / "share/licenses/paintcraft/ThirdPartyLicenses.txt").read_bytes(),
                                 (release.ROOT / "packaging/ThirdPartyLicenses.txt").read_bytes())
                flatpak.verify_payload(root / "build", hashes)
                self.assertFalse((payload / "bin" / codec_worker.MANIFEST).exists())
                manifest = json.loads((payload / "share/paintcraft" / codec_worker.MANIFEST).read_text())
                self.assertEqual(manifest["sha256"], hashes[helper.name])
                desktop = (payload / f"share/applications/{flatpak.APP_ID}.desktop").read_text()
                self.assertIn("Exec=paintcraft %f", desktop)
                self.assertIn("Icon=" + flatpak.APP_ID, desktop)
                metadata = (payload / f"share/metainfo/{flatpak.APP_ID}.metainfo.xml").read_text()
                self.assertIn(flatpak.APP_ID + ".desktop", metadata)
                self.assertTrue((payload / f"share/icons/hicolor/256x256/apps/{flatpak.APP_ID}.png").is_file())
                (payload / "bin/paintcraft-codec-worker").write_bytes(helper_bytes + b"stripped")
                with self.assertRaisesRegex(ValueError, "changed the exact release executable"):
                    flatpak.verify_payload(root / "build", hashes)

    def test_unsupported_target_refuses_before_tools_or_payload(self):
        with patch.object(flatpak.subprocess, "run") as run:
            with self.assertRaises(ValueError):
                flatpak.package("0.1.1", "x86_64-pc-windows-msvc", Path("missing"), Path("missing"), Path("unused"))
            run.assert_not_called()

    def test_complete_release_retains_old_matrix_plus_exactly_two_bundles(self):
        names = release.expected_assets(release.workspace_version())
        flatpaks = {n for n in names if n.endswith(".flatpak")}
        self.assertEqual(flatpaks, {f"paintcraft-{release.workspace_version()}-linux-{arch}.flatpak"
                                  for arch in ("x64", "arm64")})
        self.assertEqual(len(names), 26)
        for target, suffix in release.TARGETS.items():
            self.assertIn(f"paintcraft-cli-{release.workspace_version()}-{suffix}" +
                          (".zip" if "windows" in target else ".tar.gz"), names)
        self.assertEqual(sum(n.endswith(".msi") for n in names), 2)
        self.assertEqual(sum(n.endswith(".dmg") for n in names), 2)
        self.assertEqual(sum(n.endswith(".AppImage") for n in names), 2)


if __name__ == "__main__":
    unittest.main()
