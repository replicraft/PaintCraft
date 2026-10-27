"""Final notice propagation through native package layouts; no foreign execution."""
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

import test_package_native as fixtures
import codec_worker
import package_linux as linux
import package_macos as macos
import package_windows as windows
import package_release as release
from test_package_release import executable


class FloatNativeTests(unittest.TestCase):
    setUp = fixtures.NativePackageTests.setUp

    def final(self, target):
        executable(self.binary, target)
        helper = self.binary.parent / codec_worker.filename(target)
        helper.write_bytes(helper.read_bytes().replace(codec_worker.BUILD_PREFIX, codec_worker.FLOAT_BUILD_PREFIX)
                           .replace(codec_worker.BUILD_SUFFIX, codec_worker.FLOAT_BUILD_SUFFIX))

    def test_windows_wix_receives_exact_float_notice_only_for_final_profile(self):
        for target in windows.ARCHITECTURES:
            self.final(target)
            def build(command, **kwargs):
                notice = next(value.split("=", 1)[1] for value in command if value.startswith("FloatCodecNotice="))
                self.assertEqual(Path(notice).read_bytes(), (release.ROOT / "packaging" / codec_worker.FLOAT_NOTICE).read_bytes())
                Path(command[command.index("-o") + 1]).write_bytes(b"MSI control")
                return subprocess.CompletedProcess(command, 0)
            with patch.object(windows.subprocess, "run", side_effect=build):
                windows.package(self.version, target, self.binary, self.output)

    def test_linux_layouts_include_float_notice_at_the_standard_license_path(self):
        for target in linux.ARCHITECTURES:
            self.final(target)
            directory = self.root / target
            directory.mkdir()
            helper = codec_worker.copy(self.binary, directory, self.version, target)
            config = linux.configuration(self.version, target, self.binary, self.output, "deb", helper, directory / codec_worker.MANIFEST)
            notice = str(release.ROOT / "packaging" / codec_worker.FLOAT_NOTICE)
            self.assertEqual(config["deb"]["files"][notice], "usr/share/licenses/paintcraft/" + codec_worker.FLOAT_NOTICE)
            self.assertEqual((directory / codec_worker.FLOAT_NOTICE).read_bytes(), Path(notice).read_bytes())

    def test_macos_bundle_keeps_notice_and_reseals_changed_signed_helper_bytes(self):
        target = "aarch64-apple-darwin"
        self.final(target)
        launcher = self.root / "launcher"
        executable(launcher, target, with_helper=False)
        bundle = self.output / "PaintCraft.app"
        macos.populate_bundle(self.binary, bundle, self.version, launcher)
        helper_dir = bundle / "Contents/Helpers/PaintCraft Editor.app/Contents/MacOS"
        resources = helper_dir.parent / "Resources"
        before = codec_worker.read_manifest(resources)["sha256"]
        helper = helper_dir / codec_worker.filename(target)
        def sign(command, **kwargs):
            if command[:2] == ["/usr/bin/codesign", "--force"] and Path(command[-1]) == helper:
                helper.write_bytes(helper.read_bytes() + b"signature control")
            return subprocess.CompletedProcess(command, 0)
        with patch.object(macos.subprocess, "run", side_effect=sign):
            macos.sign_bundle(bundle)
        self.assertNotEqual(codec_worker.read_manifest(resources)["sha256"], before)
        codec_worker.verify_manifest(helper_dir, self.version, target, resource_directory=resources)
        self.assertEqual((resources / codec_worker.FLOAT_NOTICE).read_bytes(), (release.ROOT / "packaging" / codec_worker.FLOAT_NOTICE).read_bytes())
