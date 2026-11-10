"""Desktop distributions carry exact updater notices; CLI archive schema stays unchanged."""
from pathlib import Path
import shutil
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import xml.etree.ElementTree as ET
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import codec_worker
import package_cli as cli
import package_linux as linux
import package_macos as macos
import package_release as release
import package_windows as windows
from test_package_release import executable


class DesktopNoticeTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="paintcraft-notice-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.version = release.workspace_version()
        self.aggregate = release.ROOT / "packaging/ThirdPartyLicenses.txt"

    def test_aggregate_preserves_all_notices_and_updater_apache_license_and_notice(self):
        data = self.aggregate.read_bytes()
        for name in ("FONT_LICENSES.txt", "JPEGXR_LICENSE.txt", "DAV1D_LICENSE.txt", "HDR_EXR_LICENSES_V1.txt"):
            self.assertIn((release.ROOT / "packaging/desktop/licenses" / name).read_bytes(), data)
        updater = release.ROOT / "crates/adapters/paintcraft-update"
        self.assertIn((updater / "LICENSE").read_bytes(), data)
        self.assertIn((updater / "NOTICE").read_bytes(), data)
        preferences = (release.ROOT / "apps/paintcraft/src/app/preferences.rs").read_text(encoding="utf-8")
        self.assertIn('../../../../packaging/ThirdPartyLicenses.txt', preferences)
        self.assertIn('"Third-party licenses"', preferences)
        for lang in ("en", "de", "es", "hi", "ja", "pt-BR", "zh-CN"):
            text = (release.ROOT / f"crates/presentation/paintcraft-ui/locales/{lang}/settings.ftl").read_text(encoding="utf-8")
            self.assertEqual(text.count("third-party-licenses ="), 1)

    def test_desktop_archives_include_exact_aggregate_and_cli_remains_separate(self):
        for target in ("x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc",
                       "x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"):
            with self.subTest(target=target):
                root = self.root / target
                root.mkdir()
                binary = executable(root / ("paintcraft.exe" if "windows" in target else "paintcraft"), target)
                output = root / "output"
                release.package(self.version, target, binary, output)
                stem = f"paintcraft-{self.version}-{release.TARGETS[target]}"
                if "windows" in target:
                    with zipfile.ZipFile(output / (stem + "-portable.zip")) as archive:
                        self.assertEqual(archive.read("ThirdPartyLicenses.txt"), self.aggregate.read_bytes())
                else:
                    with tarfile.open(output / (stem + ".tar.gz")) as archive:
                        self.assertEqual(archive.extractfile("ThirdPartyLicenses.txt").read(), self.aggregate.read_bytes())
                headless = root / ("paintcraft-cli.exe" if "windows" in target else "paintcraft-cli")
                shutil.copy2(binary, headless)
                archive_path = cli.package(self.version, target, headless, output)
                if "windows" in target:
                    with zipfile.ZipFile(archive_path) as archive:
                        self.assertNotIn("ThirdPartyLicenses.txt", archive.namelist())
                else:
                    with tarfile.open(archive_path) as archive:
                        self.assertNotIn("ThirdPartyLicenses.txt", archive.getnames())

    def test_linux_installer_file_maps_include_aggregate_on_both_architectures(self):
        self.assertEqual(linux.desktop_files()[str(self.aggregate)], "usr/share/licenses/paintcraft/ThirdPartyLicenses.txt")
        for target in linux.ARCHITECTURES:
            root = self.root / target
            root.mkdir()
            binary = executable(root / "paintcraft", target)
            seen = []
            def package(command, **kwargs):
                import json
                import tomllib
                if command[1] == "generate-rpm":
                    entries = tomllib.loads(command[command.index("--set-metadata") + 1])["assets"]
                    self.assertTrue(any(e["source"] == str(self.aggregate) and e["dest"] == "/usr/share/licenses/paintcraft/ThirdPartyLicenses.txt" for e in entries))
                    Path(command[command.index("--output") + 1]).write_bytes(b"fixture")
                else:
                    config = json.loads(command[-1]); kind = config["formats"][0]
                    self.assertEqual(config["appimage" if kind == "appimage" else "deb"]["files"][str(self.aggregate)],
                                     "usr/share/licenses/paintcraft/ThirdPartyLicenses.txt")
                    Path(config["outDir"], "fixture." + ("AppImage" if kind == "appimage" else "deb")).write_bytes(b"fixture")
                seen.append(command[1])
            with patch.object(linux.subprocess, "run", side_effect=package):
                linux.package(self.version, target, binary, root / "output", ["deb", "appimage", "rpm"])
            self.assertEqual(len(seen), 3)

    def test_macos_bundle_resources_and_msi_include_exact_aggregate(self):
        binary = executable(self.root / "paintcraft", "aarch64-apple-darwin")
        launcher = self.root / "launcher"
        shutil.copy2(binary, launcher)
        bundle = self.root / "PaintCraft.app"
        macos.populate_bundle(binary, bundle, self.version, launcher)
        self.assertEqual((bundle / "Contents/Resources/ThirdPartyLicenses.txt").read_bytes(), self.aggregate.read_bytes())
        for target in windows.ARCHITECTURES:
            binary = executable(self.root / "paintcraft.exe", target)
            def package(command, **kwargs):
                self.assertIn(f"ThirdPartyLicenses={self.aggregate}", command)
                Path(command[command.index("-o") + 1]).write_bytes(b"fixture")
            with patch.object(windows.subprocess, "run", side_effect=package):
                windows.package(self.version, target, binary, self.root / "output")
        tree = ET.parse(release.ROOT / "packaging/windows/windows.wxs")
        files = [e for e in tree.iter() if e.tag.endswith("}File") and e.get("Name") == "ThirdPartyLicenses.txt"]
        self.assertEqual(len(files), 1)
        self.assertEqual(files[0].get("Source"), "$(var.ThirdPartyLicenses)")


if __name__ == "__main__":
    unittest.main()
