"""Packaging checks use temporary files and mocks, without launching/registering apps."""
import contextlib
import importlib.util
import io
import os
import sys
from pathlib import Path
import plistlib
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("package_macos", Path(__file__).parents[1] / "packaging/package_macos.py")
PACK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACK)


class BundleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="paintcraft-package-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.binary = self.root / "executable with spaces"
        self.binary.write_bytes(b"fixture executable; never launched")
        self.destination = self.root / "PaintCraft.app"

    def prepare(self, destination):
        PACK.populate_bundle(self.binary, destination, "0.1.0", self.binary)

    def test_owned_pcnd_type_and_editor_registration_share_existing_icon(self):
        PACK.publish_bundle(self.destination, self.prepare)
        contents = self.destination / "Contents"
        info = plistlib.loads((contents / "Info.plist").read_bytes())
        self.assertEqual(info["CFBundleIdentifier"], PACK.BUNDLE_ID)
        self.assertEqual(info["CFBundleVersion"], "0.1.0")
        self.assertEqual(info["CFBundleExecutable"], "paintcraft-launcher")
        editor = contents / "Helpers/PaintCraft Editor.app/Contents"
        child = plistlib.loads((editor / "Info.plist").read_bytes())
        self.assertEqual(child["CFBundleIdentifier"], PACK.BUNDLE_ID + ".editor")
        self.assertNotIn("CFBundleDocumentTypes", child)
        self.assertNotIn("UTExportedTypeDeclarations", child)
        document, = info["CFBundleDocumentTypes"]
        exported, = info["UTExportedTypeDeclarations"]
        self.assertEqual(document["CFBundleTypeRole"], "Editor")
        self.assertEqual(document["LSHandlerRank"], "Owner")
        self.assertEqual(document["LSItemContentTypes"], [exported["UTTypeIdentifier"]])
        self.assertEqual(exported["UTTypeIdentifier"], PACK.BUNDLE_ID + ".pcnd")
        self.assertIn("public.data", exported["UTTypeConformsTo"])
        self.assertEqual(exported["UTTypeTagSpecification"]["public.filename-extension"], ["pcnd"])
        self.assertEqual(exported["UTTypeTagSpecification"]["public.mime-type"], "application/x-paintcraft")
        icon = contents / "Resources" / document["CFBundleTypeIconFile"]
        self.assertEqual(icon.read_bytes(), PACK.ICON.read_bytes())
        self.assertEqual(exported["UTTypeIconFile"], icon.name)
        self.assertEqual((contents / "Helpers/PaintCraft Editor.app/Contents/MacOS/paintcraft").read_bytes(), self.binary.read_bytes())
        self.assertEqual((contents / "Resources/FONT_LICENSES.txt").read_bytes(),
                         (PACK.ROOT / "packaging/FONT_LICENSES.txt").read_bytes())
        # Windows chmod/stat do not implement Unix execute bits. Real DMG/ZIP
        # permissions are also checked on each native macOS release runner.
        if os.name != "nt":
            self.assertTrue((contents / "Helpers/PaintCraft Editor.app/Contents/MacOS/paintcraft").stat().st_mode & 0o111)

    def test_replacement_requires_explicit_flag_and_recognized_bundle(self):
        PACK.publish_bundle(self.destination, self.prepare)
        with self.assertRaises(ValueError):
            PACK.publish_bundle(self.destination, self.prepare)
        PACK.publish_bundle(self.destination, self.prepare, replace=True)
        info = self.destination / "Contents/Info.plist"
        metadata = plistlib.loads(info.read_bytes())
        metadata["CFBundleIdentifier"] = "another.application"
        info.write_bytes(plistlib.dumps(metadata))
        with self.assertRaisesRegex(ValueError, "another application's"):
            PACK.publish_bundle(self.destination, self.prepare, replace=True)
        self.assertEqual(plistlib.loads(info.read_bytes())["CFBundleIdentifier"], "another.application")
        link = self.root / "linked.app"
        link.symlink_to(self.destination, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "symlink"):
            PACK.publish_bundle(link, self.prepare, replace=True)

    def test_failed_preparation_leaves_existing_installation_unchanged(self):
        PACK.publish_bundle(self.destination, self.prepare)
        sentinel = self.destination / "untouched"
        sentinel.write_text("original")
        def fail(stage):
            self.prepare(stage)
            raise OSError("simulated signing failure")
        with self.assertRaises(OSError):
            PACK.publish_bundle(self.destination, fail, replace=True)
        self.assertEqual(sentinel.read_text(), "original")
        self.assertEqual(list(self.root.glob(".paintcraft-package-*")), [])

    def test_build_uses_reported_executable_path(self):
        artifact = '{"reason":"compiler-artifact","target":{"name":"paintcraft"},"executable":"/custom target/paintcraft"}\n'
        completed = subprocess.CompletedProcess([], 0, artifact, "")
        with patch.object(PACK.subprocess, "run", return_value=completed) as run:
            self.assertEqual(PACK.built_binary(False), Path("/custom target/paintcraft"))
        self.assertIn("--release", run.call_args.args[0])
        self.assertEqual(run.call_args.kwargs["cwd"], PACK.ROOT)

    def test_install_and_registration_are_opt_in_and_never_launch(self):
        installed = self.root / "home/Applications/PaintCraft.app"
        with patch.object(PACK.sys, "platform", "darwin"), \
             patch.object(PACK.Path, "home", return_value=self.root / "home"), \
             patch.object(PACK.subprocess, "run") as run, \
             patch.object(PACK, "build_launcher", return_value=self.binary), \
             contextlib.redirect_stdout(io.StringIO()):
            code = PACK.main(["--binary", str(self.binary), "--output", str(self.destination)])
            self.assertEqual(code, 0)
            self.assertFalse(installed.exists())
            self.assertEqual(len(run.call_args_list), 3)
            self.assertEqual(run.call_args.args[0][0], "/usr/bin/codesign")
            run.reset_mock()
            code = PACK.main(["--binary", str(self.binary), "--output", str(self.destination), "--replace", "--install", "--register"])
            self.assertEqual(code, 0)
            self.assertTrue(installed.exists())
            self.assertEqual(run.call_args.args[0], [PACK.LSREGISTER, "-f", str(installed)])
            self.assertEqual(len(run.call_args_list), 4)
        self.assertEqual((installed / "Contents/Helpers/PaintCraft Editor.app/Contents/MacOS/paintcraft").read_bytes(), self.binary.read_bytes())


# Explicit OS-event smoke test: the child executable only records argv and exits.
# No editor window, print job, default-handler preference or installation is used.
class NativeLauncherTests(unittest.TestCase):
    @unittest.skipUnless(
        sys.platform == "darwin" and os.environ.get("PAINTCRAFT_TEST_NATIVE_OPEN") == "1",
        "opt-in native Launch Services event test",
    )
    def test_cold_and_running_file_events_preserve_paths_without_blank_launch(self):
        import json
        import sys
        import time
        import uuid
        with tempfile.TemporaryDirectory(prefix="paintcraft-finder-event-") as directory:
            root = Path(directory).resolve()
            log = root / "received.jsonl"
            dummy = root / "dummy-editor"
            source = root / "Dummy.swift"
            log.write_text("")
            source.write_text("import AppKit\n"
                + "let output = FileHandle(forWritingAtPath: " + json.dumps(str(log)) + ")!\n"
                + "output.seekToEndOfFile()\n"
                + "let data = try! JSONSerialization.data(withJSONObject: Array(CommandLine.arguments.dropFirst()))\n"
                + "output.write(data + Data([10]))\n"
                + "let app = NSApplication.shared\napp.setActivationPolicy(.accessory)\n"
                + "DispatchQueue.main.asyncAfter(deadline: .now()+3) { exit(0) }\napp.run()\n")
            subprocess.run(["/usr/bin/xcrun", "swiftc", "-module-cache-path", str(root / "swift-module-cache"),
                            str(source), "-o", str(dummy)], check=True)
            launcher = PACK.build_launcher(root)
            bundle = root / "PaintCraft Association Test.app"
            PACK.populate_bundle(dummy, bundle, "0.1.0", launcher)
            metadata = bundle / "Contents/Info.plist"
            info = plistlib.loads(metadata.read_bytes())
            info["CFBundleIdentifier"] = "io.github.replicraft.PaintCraft.association-test." + uuid.uuid4().hex
            # Test event dispatch independently of global document registration.
            info.pop("CFBundleDocumentTypes")
            info.pop("UTExportedTypeDeclarations")
            info["LSUIElement"] = True
            metadata.write_bytes(plistlib.dumps(info))
            for app in [bundle / "Contents/Helpers/PaintCraft Editor.app", bundle]:
                subprocess.run(["/usr/bin/codesign", "--force", "--sign", "-", "--timestamp=none", str(app)], check=True)
            files = [root / "first 日本語 and spaces.pcnd", root / "second 'quotes' ; dollar$.pcnd"]
            for path in files:
                path.write_text("native event fixture, never parsed as document")
            def wait_for(count):
                deadline = time.monotonic() + 10
                while time.monotonic() < deadline:
                    rows = [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
                    if len(rows) >= count:
                        return rows
                    time.sleep(0.02)
                self.fail("Native launcher did not deliver expected file event")
            subprocess.run(["/usr/bin/open", "-n", "-a", str(bundle), str(files[0])], check=True)
            self.assertEqual(wait_for(1), [[str(files[0])]])
            subprocess.run(["/usr/bin/open", "-a", str(bundle), str(files[1])], check=True)
            self.assertEqual(wait_for(2), [[str(files[0])], [str(files[1])]])
            # Wait beyond blank-launch coalescing and for dummy children to exit.
            time.sleep(3.3)
            self.assertEqual(wait_for(2), [[str(files[0])], [str(files[1])]])


if __name__ == "__main__":
    unittest.main()
