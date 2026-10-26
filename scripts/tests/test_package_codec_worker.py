"""Inspect actual staged helper contracts without running cross-target binaries."""
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import codec_worker
import package_release as release
from test_package_release import executable


class CodecWorkerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="paintcraft-helper-package-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.binary = self.root / "paintcraft"
        self.version = release.workspace_version()
        self.target = "aarch64-apple-darwin"
        executable(self.binary, self.target)
        self.helper = self.root / codec_worker.filename(self.target)

    def test_all_architectures_preserve_exact_helper_and_embedded_contract_hash(self):
        for target in release.TARGETS:
            executable(self.binary, target)
            destination = self.root / target
            destination.mkdir()
            helper = codec_worker.copy(self.binary, destination, self.version, target)
            metadata = json.loads((destination / codec_worker.MANIFEST).read_bytes())
            self.assertEqual(helper.read_bytes(), (self.root / helper.name).read_bytes())
            self.assertEqual(metadata["sha256"], hashlib.sha256(helper.read_bytes()).hexdigest())
            self.assertEqual(metadata["application_version"], self.version)
            self.assertEqual(metadata["target"], target)
            self.assertEqual(metadata["protocol_version"], 2)
            self.assertEqual(metadata["requested_heap_ceiling_bytes"], 256 << 20)
            self.assertTrue(metadata["embedded_build_record_verified"])
            self.assertFalse(metadata["public_reader_enabled"])
            codec_worker.verify_manifest(destination, self.version, target)
            helper.write_bytes(helper.read_bytes() + b"changed executable")
            with self.assertRaises(ValueError):
                codec_worker.verify_manifest(destination, self.version, target)

    def test_missing_wrong_architecture_stale_version_protocol_and_heap_refuse(self):
        correct = self.helper.read_bytes()
        for bad in [b"", correct[:64],
                    correct.replace(self.version.encode(), b"999.0.0"),
                    correct.replace(b"PCDEC002", b"PCDEC003"),
                    correct.replace(b"HEAP268435456", b"HEAP268435457")]:
            self.helper.write_bytes(bad)
            with self.assertRaises(ValueError):
                codec_worker.prepare(self.binary, self.version, self.target)
        executable(self.helper, "x86_64-apple-darwin", with_helper=False)
        with self.assertRaises(ValueError):
            codec_worker.prepare(self.binary, self.version, self.target)
        self.helper.unlink()
        with self.assertRaises(ValueError):
            codec_worker.prepare(self.binary, self.version, self.target)

    def test_symlinked_helper_or_manifest_never_overwrites_a_foreign_file(self):
        original = self.helper.read_bytes()
        self.helper.unlink()
        foreign = self.root / "foreign"
        foreign.write_bytes(original)
        try:
            self.helper.symlink_to(foreign)
        except OSError:
            self.skipTest("symlink privilege unavailable")
        with self.assertRaises(ValueError):
            codec_worker.prepare(self.binary, self.version, self.target)
        self.helper.unlink()
        self.helper.write_bytes(original)
        output = self.root / "output"
        output.mkdir()
        (output / codec_worker.MANIFEST).symlink_to(foreign)
        with self.assertRaises(ValueError):
            codec_worker.copy(self.binary, output, self.version, self.target)
        self.assertEqual(foreign.read_bytes(), original)

    def test_cli_and_desktop_workflow_share_one_helper_cargo_build(self):
        workflow = (release.ROOT / ".github/workflows/release.yml").read_text()
        self.assertIn("-p paintcraft -p paintcraft-cli -p paintcraft-codec-worker --target", workflow)


if __name__ == "__main__":
    unittest.main()
