"""Native package command and integration metadata checks without installers."""
import json
import hashlib
import zipfile
from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile
import tomllib
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import xml.etree.ElementTree as ET

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import package_linux as linux
import package_macos as macos
import package_release as release
import package_windows as windows
from test_package_release import executable


class NativePackageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="paintcraft-native-package-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.binary = self.root / "binary with spaces"
        self.output = self.root / "packages"
        self.output.mkdir()
        self.version = release.workspace_version()

    def test_bundled_font_notices_include_every_shared_font_license(self):
        notices = (release.ROOT / "packaging/FONT_LICENSES.txt").read_text()
        for source in (release.ROOT / "assets/fonts").glob("*/LICENSE*"):
            self.assertIn(source.parent.name, notices)
            self.assertIn(source.read_text().rstrip(), notices)

    def test_same_wix_template_uses_correct_arch_and_stable_version_for_both_targets(self):
        for target, architecture in windows.ARCHITECTURES.items():
            executable(self.binary, target)
            def build(command, **kwargs):
                Path(command[command.index("-o") + 1]).write_bytes(b"MSI command fixture")
                return subprocess.CompletedProcess(command, 0)
            with patch.object(windows.subprocess, "run", side_effect=build) as run:
                windows.package(self.version, target, self.binary, self.output)
            command = run.call_args.args[0]
            self.assertEqual(command[:4], ["wix", "build", "-arch", architecture])
            self.assertIn(f"Binary={self.binary.resolve()}", command)
            self.assertTrue(any(value.startswith("CodecWorker=") for value in command))
            self.assertTrue(any(value.startswith("CodecManifest=") for value in command))
            self.assertTrue(any(value.startswith("CodecNotice=") for value in command))
            self.assertEqual(command[-1], str(release.ROOT / "packaging/windows.wxs"))
            self.assertTrue((self.output / f"paintcraft-{self.version}-windows-{architecture}.msi.sha256").is_file())

    def test_msi_registers_quoted_pcnd_open_command_and_default_apps_capability(self):
        tree = ET.parse(release.ROOT / "packaging/windows.wxs")
        ns = {"w": "http://wixtoolset.org/schemas/v4/wxs"}
        package = tree.find("w:Package", ns)
        self.assertEqual(package.get("Scope"), "perMachine")
        self.assertEqual(package.get("UpgradeCode"), "67fca40e-0081-48a1-a395-7044c149044d")
        values = tree.findall(".//w:RegistryValue", ns)
        command, = [v for v in values if v.get("Key") == r"Software\Classes\PaintCraft.Document\shell\open\command"]
        self.assertEqual(command.get("Value"), '"[#PaintCraftExe]" "%1"')
        extension, = [v for v in values if v.get("Key") == r"Software\Classes\.pcnd" and v.get("Name") is None]
        self.assertEqual(extension.get("Value"), "PaintCraft.Document")
        self.assertTrue(any(v.get("Name") == ".pcnd" and v.get("Value") == "PaintCraft.Document" for v in values))
        self.assertTrue(any(v.get("Key") == r"Software\RegisteredApplications" for v in values))
        self.assertTrue(all(v.get("Root") == "HKLM" for v in values))
        self.assertFalse(any("UserChoice" in v.get("Key") for v in values))

    def test_deb_and_appimage_share_mime_and_license_payloads_on_both_architectures(self):
        for target in linux.ARCHITECTURES:
            executable(self.binary, target)
            for kind, extension in (("deb", "deb"), ("appimage", "AppImage")):
                def build(command, **kwargs):
                    config = json.loads(command[-1])
                    self.assertEqual(config["targetTriple"], target)
                    self.assertEqual(config["formats"], [kind])
                    self.assertIn("usr/share/licenses/paintcraft/JPEGXR_LICENSE.txt", config["appimage" if kind == "appimage" else "deb"]["files"].values())
                    self.assertIn("usr/share/mime/packages/paintcraft.xml",
                                  config["appimage" if kind == "appimage" else "deb"]["files"].values())
                    for source in config["deb"]["files"]:
                        self.assertTrue(Path(source).is_file())
                    if kind == "appimage":
                        self.assertEqual(kwargs["env"]["APPIMAGE_EXTRACT_AND_RUN"], "1")
                        self.assertIn("libxkbcommon*.so.*", config["appimage"]["libs"])
                        self.assertIn("libX11*.so.*", config["appimage"]["libs"])
                        self.assertEqual(len(config["binaries"]), 3)
                        launcher = Path(config["binaries"][0]["path"])
                        self.assertIn('"$@"', launcher.read_text())
                        desktop = Path(config["deb"]["desktopTemplate"]).read_text()
                        self.assertIn(f"Exec={linux.APP_ID}.sh %f", desktop)
                        self.assertIn(f"Icon={linux.APP_ID}", desktop)
                    Path(config["outDir"], "generated." + extension).write_bytes(b"package command fixture")
                with patch.object(linux.subprocess, "run", side_effect=build):
                    linux.package(self.version, target, self.binary, self.output, [kind])
                self.assertTrue((self.output / f"paintcraft-{self.version}-{release.TARGETS[target]}.{extension}.sha256").exists())

    def test_rpm_uses_supplied_executable_and_all_tracked_desktop_assets(self):
        executable(self.binary, linux.ARCHITECTURES[0])
        def build(command, **kwargs):
            data = tomllib.loads(command[command.index("--set-metadata") + 1])
            self.assertEqual(data["assets"][0]["source"], str(self.binary.resolve()))
            destinations = {asset["dest"] for asset in data["assets"]}
            self.assertIn("/usr/share/mime/packages/paintcraft.xml", destinations)
            self.assertIn("/usr/bin/paintcraft-codec-worker", destinations)
            self.assertIn("/usr/share/paintcraft/paintcraft-codec-worker.json", destinations)
            self.assertIn("/usr/share/applications/paintcraft.desktop", destinations)
            self.assertIn("/usr/share/licenses/paintcraft/LICENSE", destinations)
            for asset in data["assets"]:
                self.assertTrue((release.ROOT / asset["source"]).is_file())
            Path(command[command.index("--output") + 1]).write_bytes(b"RPM command fixture")
        with patch.object(linux.subprocess, "run", side_effect=build):
            linux.package(self.version, linux.ARCHITECTURES[0], self.binary, self.output, ["rpm"])

    def test_launcher_target_is_explicit_for_thin_intel_and_apple_silicon_apps(self):
        for target, arch in (("aarch64-apple-darwin", "arm64"), ("x86_64-apple-darwin", "x86_64")):
            with patch.object(macos.subprocess, "run") as run:
                macos.build_launcher(self.root, target)
            command = run.call_args.args[0]
            self.assertEqual(command[command.index("-target") + 1], f"{arch}-apple-macosx13.0")
        self.assertEqual(macos.metadata("0.1.0-beta.1")["CFBundleVersion"], "0.1.0")
        self.assertEqual(macos.metadata(self.version)["LSMinimumSystemVersion"], "13.0")

    def test_release_dmg_and_update_zip_reuse_the_same_signed_bundle(self):
        executable(self.binary, "aarch64-apple-darwin")
        calls = []
        def run(command, **kwargs):
            calls.append(command)
            if command[0].endswith("codesign") and Path(command[-1]).name == "paintcraft-codec-worker":
                helper = Path(command[-1])
                helper.write_bytes(helper.read_bytes() + b"signed helper fixture")
        def build_dmg(filename, volume_name, settings):
            self.assertEqual(volume_name, "PaintCraft")
            self.assertEqual(settings["window_rect"], ((200, 200), (640, 270)))
            self.assertEqual(settings["icon_locations"],
                             {"PaintCraft.app": (150, 70), "Applications": (490, 70)})
            self.assertEqual(settings["symlinks"], {"Applications": "/Applications"})
            self.assertEqual(settings["default_view"], "icon-view")
            self.assertEqual(settings["icon_size"], 128)
            self.assertTrue(Path(settings["background"]).is_file())
            self.assertTrue(Path(settings["files"][0]).is_dir())
            self.assertNotIn("hide_extensions", settings)
            Path(filename).write_bytes(b"DMG command fixture")
        with patch.object(macos.sys, "platform", "darwin"), \
             patch.dict(sys.modules, {"dmgbuild": SimpleNamespace(build_dmg=build_dmg)}), \
             patch.object(macos, "build_launcher", return_value=self.binary), \
             patch.object(macos.subprocess, "run", side_effect=run):
            macos.release_archives(self.binary, self.output, self.version, "aarch64-apple-darwin", "fixture")
        self.assertTrue((self.output / "fixture.dmg.sha256").exists())
        self.assertTrue((self.output / "fixture-update.zip.sha256").exists())
        self.assertEqual(sum(command[0].endswith("codesign") for command in calls), 4)
        self.assertTrue(calls[0][-1].endswith("/MacOS/paintcraft-codec-worker"))
        with zipfile.ZipFile(self.output / "fixture-update.zip") as archive:
            directory = "PaintCraft.app/Contents/Helpers/PaintCraft Editor.app/Contents/MacOS/"
            helper = archive.read(directory + "paintcraft-codec-worker")
            manifest = json.loads(archive.read(directory + "paintcraft-codec-worker.json"))
            self.assertTrue(helper.endswith(b"signed helper fixture"))
            self.assertEqual(manifest["sha256"], hashlib.sha256(helper).hexdigest())
        self.assertTrue(any("verify" in command and command[0].endswith("hdiutil") for command in calls))


if __name__ == "__main__":
    unittest.main()
