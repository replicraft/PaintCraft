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
            self.assertEqual(metadata["profile"], "internal-heif-gray8-heif-srgb8-jxl-rgba8-jxr-rgb8-private-source1-parent-lease1")
            self.assertEqual(metadata["requested_heap_ceiling_bytes"], 256 << 20)
            self.assertTrue(metadata["embedded_build_record_verified"])
            self.assertEqual(metadata["schema_version"], 2)
            self.assertEqual(metadata["qualified_import_variants"], ["native-jxl-srgb8-worker-v1", "native-jxr-srgb8-worker-v1", "native-heif-srgb8-worker-v1"])
            self.assertEqual((destination / codec_worker.NOTICE).read_bytes(),
                             (release.ROOT / "crates/services/paintcraft-formats/tests/jxr-fixtures/JXRLib-LICENSE.md").read_bytes())
            codec_worker.verify_manifest(destination, self.version, target)
            helper.write_bytes(helper.read_bytes() + b"changed executable")
            with self.assertRaises(ValueError):
                codec_worker.verify_manifest(destination, self.version, target)


    def test_missing_wrong_architecture_stale_version_protocol_and_heap_refuse(self):
        correct = self.helper.read_bytes()
        for bad in [b"", correct[:64],
                    correct.replace(self.version.encode(), b"999.0.0"),
                    correct.replace(b"PCDEC002", b"PCDEC003"),
                    correct.replace(b"HEAP268435456", b"HEAP268435457"),
                    correct.replace(b"BUILD_V4", b"BUILD_V3"),
                    correct.replace(b"STDIN_PARENT_LEASE1", b"STDIN_PARENT_LEASE0"),
                    correct.replace(b"JXR_PRIVATE_SOURCE1", b"JXR_PRIVATE_SOURCE0")]:
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

    def test_missing_changed_and_symlinked_notice_are_refused_without_foreign_writes(self):
        destination = self.root / "notice-output"
        destination.mkdir()
        codec_worker.copy(self.binary, destination, self.version, self.target)
        notice = destination / codec_worker.NOTICE
        expected = notice.read_bytes()
        notice.write_bytes(b"changed notice")
        with self.assertRaises(ValueError):
            codec_worker.verify_manifest(destination, self.version, self.target)
        notice.unlink()
        with self.assertRaises(ValueError):
            codec_worker.verify_manifest(destination, self.version, self.target)
        foreign = self.root / "foreign-notice"
        foreign.write_bytes(expected)
        try:
            notice.symlink_to(foreign)
        except OSError:
            self.skipTest("symlink privilege unavailable")
        with self.assertRaises(ValueError):
            codec_worker.copy(self.binary, destination, self.version, self.target)
        self.assertEqual(foreign.read_bytes(), expected)

    def test_avif_contract_and_license_are_exact_and_ambiguous_markers_refuse(self):
        original = self.helper.read_bytes()
        v5 = original.replace(codec_worker.BUILD_PREFIX, codec_worker.AVIF_BUILD_PREFIX).replace(codec_worker.BUILD_SUFFIX, codec_worker.AVIF_BUILD_SUFFIX)
        self.helper.write_bytes(v5)
        destination = self.root / "avif-output"
        destination.mkdir()
        codec_worker.copy(self.binary, destination, self.version, self.target)
        metadata = json.loads((destination / codec_worker.MANIFEST).read_bytes())
        self.assertEqual(metadata["profile"], codec_worker.AVIF_PROFILE)
        self.assertEqual(metadata["qualified_import_variants"], codec_worker.NATIVE_VARIANTS + ["native-avif-srgb8-worker-v1"])
        self.assertEqual((destination / codec_worker.AVIF_NOTICE).read_bytes(), (release.ROOT / "apps/paintcraft-codec-worker/vendor/dav1d/COPYING").read_bytes())
        codec_worker.verify_manifest(destination, self.version, self.target)
        for invalid in [v5 + codec_worker.BUILD_PREFIX, v5.replace(b"ALLOC1", b"ALLOC0"),
                        v5.replace(b"DAV1D154", b"DAV1D130")]:
            self.helper.write_bytes(invalid)
            with self.assertRaises(ValueError):
                codec_worker.prepare(self.binary, self.version, self.target)
        for invalid_notice in [b"", b"wrong license"]:
            (destination / codec_worker.AVIF_NOTICE).write_bytes(invalid_notice)
            with self.assertRaises(ValueError):
                codec_worker.verify_manifest(destination, self.version, self.target)

    def test_sdr_contract_is_exact_for_all_target_archives_without_executing_them(self):
        for target in release.TARGETS:
            with self.subTest(target=target):
                executable(self.binary, target)
                helper = self.root / codec_worker.filename(target)
                v6 = helper.read_bytes().replace(codec_worker.BUILD_PREFIX, codec_worker.AVIF_SDR_BUILD_PREFIX).replace(codec_worker.BUILD_SUFFIX, codec_worker.AVIF_SDR_BUILD_SUFFIX)
                helper.write_bytes(v6)
                destination = self.root / (target + "-sdr")
                destination.mkdir()
                codec_worker.copy(self.binary, destination, self.version, target)
                metadata = json.loads((destination / codec_worker.MANIFEST).read_bytes())
                self.assertEqual(metadata["profile"], codec_worker.AVIF_SDR_PROFILE)
                self.assertEqual(metadata["qualified_import_variants"], codec_worker.NATIVE_VARIANTS + ["native-avif-srgb8-worker-v1", "native-avif-sdr-ycbcr8-worker-v1"])
                codec_worker.verify_manifest(destination, self.version, target)
                for bad in [v6 + codec_worker.AVIF_BUILD_PREFIX, v6 + codec_worker.BUILD_PREFIX,
                            v6.replace(b"CSP12_WORK1", b"CSP12_WORK0"),
                            v6.replace(b"YUV444_420", b"YUV444_422"),
                            v6.replace(b"ALLOC1", b"ALLOC0")]:
                    helper.write_bytes(bad)
                    with self.assertRaises(ValueError):
                        codec_worker.prepare(self.binary, self.version, target)
                manifest = destination / codec_worker.MANIFEST
                for bad in [metadata["qualified_import_variants"][:-1], list(reversed(metadata["qualified_import_variants"]))]:
                    manifest.write_text(json.dumps({**metadata, "qualified_import_variants": bad}))
                    with self.assertRaises(ValueError):
                        codec_worker.verify_manifest(destination, self.version, target)

    def test_cli_and_desktop_workflow_share_one_helper_cargo_build(self):
        workflow = (release.ROOT / ".github/workflows/release.yml").read_text()
        self.assertIn("-p paintcraft -p paintcraft-cli -p paintcraft-codec-worker --target", workflow)
        self.assertIn("--features paintcraft-codec-worker/float-codecs", workflow)
        self.assertIn("meson==1.12.1 ninja==1.13.2", workflow)
        self.assertIn("qualify_cli_helper.py --profile v7", workflow)
        self.assertNotIn("--include-ignored", workflow)


if __name__ == "__main__":
    unittest.main()
