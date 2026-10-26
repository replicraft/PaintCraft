"""Canonical MSI payload controls; synthetic PE files are never executed."""
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import codec_worker
import package_release as release
import verify_windows_payload as verifier
from test_package_release import executable


class WindowsPayloadTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="paintcraft-windows-payload-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "release"
        self.source.mkdir()
        self.binary = self.source / "paintcraft.exe"
        self.installed = self.root / "installed with spaces"
        self.installed.mkdir()
        self.version = release.workspace_version()

    def prepare(self, target, profile="v6"):
        executable(self.binary, target)
        helper = self.source / codec_worker.filename(target)
        if profile == "v6":
            helper.write_bytes(helper.read_bytes().replace(codec_worker.BUILD_PREFIX, codec_worker.AVIF_SDR_BUILD_PREFIX)
                               .replace(codec_worker.BUILD_SUFFIX, codec_worker.AVIF_SDR_BUILD_SUFFIX))
        shutil.copy2(self.binary, self.installed / "paintcraft.exe")
        codec_worker.copy(self.binary, self.installed, self.version, target)
        return json.loads((self.installed / codec_worker.MANIFEST).read_text())

    def test_actual_schema2_producer_output_passes_both_windows_architectures(self):
        for target in ("x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc"):
            for profile in ("v4", "v6"):
                with self.subTest(target=target, profile=profile):
                    metadata = self.prepare(target, profile)
                    self.assertEqual(metadata["schema_version"], 2)
                    self.assertNotIn("public_reader_enabled", metadata)
                    verifier.verify(self.binary, self.installed)

    def test_old_unknown_forged_and_malformed_manifests_fail_canonical_validation(self):
        metadata = self.prepare("x86_64-pc-windows-msvc")
        path = self.installed / codec_worker.MANIFEST
        for changes in ({"schema_version": 1, "public_reader_enabled": False},
                        {"schema_version": 3}, {"profile": "unknown-future-profile"},
                        {"profile": codec_worker.NATIVE_PROFILE},
                        {"qualified_import_variants": metadata["qualified_import_variants"][:-1]},
                        {"sha256": "0" * 64}, {"protocol_version": 3},
                        {"requested_heap_ceiling_bytes": 128 << 20},
                        {"embedded_build_record_verified": False}):
            with self.subTest(changes=changes):
                path.write_text(json.dumps({**metadata, **changes}))
                with self.assertRaises(ValueError):
                    verifier.verify(self.binary, self.installed)
        path.write_text("not JSON")
        with self.assertRaises(ValueError):
            verifier.verify(self.binary, self.installed)

    def test_changed_binary_helper_and_missing_changed_notice_fail(self):
        for name in ("paintcraft.exe", "paintcraft-codec-worker.exe", *codec_worker.NOTICES):
            for missing in (False, True):
                with self.subTest(name=name, missing=missing):
                    self.prepare("x86_64-pc-windows-msvc")
                    path = self.installed / name
                    if missing:
                        path.unlink()
                    else:
                        path.write_bytes(path.read_bytes() + b"changed")
                    with self.assertRaises((OSError, ValueError)):
                        verifier.verify(self.binary, self.installed)

    def test_real_validator_command_accepts_producer_and_returns_failure_for_old_schema(self):
        metadata = self.prepare("aarch64-pc-windows-msvc")
        command = [sys.executable, str(Path(verifier.__file__)), "--binary", str(self.binary),
                   "--directory", str(self.installed)]
        result = subprocess.run(command, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        (self.installed / codec_worker.MANIFEST).write_text(json.dumps({**metadata, "schema_version": 1}))
        result = subprocess.run(command, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("verification failed", result.stderr)
