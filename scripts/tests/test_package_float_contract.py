"""Final V7 profiles, exact notice sets and bidirectional installer transitions."""
import json
from pathlib import Path
import os
import unittest
from unittest.mock import patch

import test_install_cli as fixtures

import codec_worker
import install_cli as install
import package_release as release


class FloatContractTests(unittest.TestCase):
    setUp = fixtures.InstallTests.setUp
    archive = fixtures.InstallTests.archive
    install = fixtures.InstallTests.install
    repack = fixtures.InstallTests.repack

    def test_final_v7_archives_require_exact_seven_variants_and_versioned_notice(self):
        for target in release.TARGETS:
            with self.subTest(target=target):
                archive, sums = self.archive(target, target + "-v7", "v7")
                directory = self.install(archive, sums, self.root / (target + "-prefix"), target)
                value = codec_worker.read_manifest(directory)
                self.assertEqual(value["profile"], codec_worker.FLOAT_PROFILE)
                self.assertEqual(value["qualified_import_variants"], codec_worker.FLOAT_VARIANTS)
                self.assertEqual(len(value["qualified_import_variants"]), 7)
                self.assertEqual({p.name for p in directory.iterdir()}, install.payload_names(target, codec_worker.FLOAT_PROFILE))
                self.assertEqual((directory / codec_worker.FLOAT_NOTICE).read_bytes(), (release.ROOT / "packaging" / codec_worker.FLOAT_NOTICE).read_bytes())

    def test_missing_changed_private_duplicate_oversized_and_extra_payloads_keep_old_installation(self):
        old, old_sums = self.archive(stem="old", contract="v6")
        directory = self.install(old, old_sums)
        before = {p.name: p.read_bytes() for p in directory.iterdir()}
        archive, sums = self.archive(stem="final", contract="v7")
        original = archive.read_bytes()
        def manifest(entries, change):
            value = json.loads(entries[codec_worker.MANIFEST])
            change(value)
            entries[codec_worker.MANIFEST] = json.dumps(value).encode()
        changes = [lambda e: e.pop(codec_worker.FLOAT_NOTICE),
                   lambda e: e.update({codec_worker.FLOAT_NOTICE: b"wrong license"}),
                   lambda e: e.update({"foreign.txt": b"extra"}),
                   lambda e: manifest(e, lambda m: m.update(profile="private-native-v7-hdr-exr-sdr8-settings32-report96-16mib-1mp-parent-lease1")),
                   lambda e: manifest(e, lambda m: m.update(qualified_import_variants=m["qualified_import_variants"][:5])),
                   lambda e: e.update({codec_worker.MANIFEST: b" " * (codec_worker.MANIFEST_MAX + 1)}),
                   lambda e: e.update({codec_worker.MANIFEST: e[codec_worker.MANIFEST].replace(b'"schema_version": 2', b'"schema_version": 2, "schema_version": 2')})]
        for change in changes:
            archive.write_bytes(original)
            self.repack(archive, change)
            sums.write_bytes(release.checksum_line(archive))
            with self.assertRaises(ValueError):
                self.install(archive, sums)
            self.assertEqual({p.name: p.read_bytes() for p in directory.iterdir()}, before)
        # A legacy profile must not acquire the new notice as an arbitrary extra.
        self.repack(old, lambda e: e.update({codec_worker.FLOAT_NOTICE: (release.ROOT / "packaging" / codec_worker.FLOAT_NOTICE).read_bytes()}))
        old_sums.write_bytes(release.checksum_line(old))
        with self.assertRaises(ValueError):
            self.install(old, old_sums)
        self.assertEqual({p.name: p.read_bytes() for p in directory.iterdir()}, before)

    def test_failed_v6_v7_publication_restores_each_exact_notice_set(self):
        for number, (start, end) in enumerate([("v6", "v7"), ("v7", "v6")]):
            old, sums = self.archive(stem=f"old-{number}", contract=start)
            prefix = self.root / f"prefix-{number}"
            directory = self.install(old, sums, prefix)
            before = {p.name: p.read_bytes() for p in directory.iterdir()}
            new, new_sums = self.archive(stem=f"new-{number}", contract=end)
            original_replace = os.replace
            def fail(source, target):
                if Path(source).name == "payload":
                    raise OSError("publish control")
                return original_replace(source, target)
            with patch.object(install.os, "replace", side_effect=fail):
                with self.assertRaisesRegex(OSError, "publish control"):
                    self.install(new, new_sums, prefix)
            self.assertEqual({p.name: p.read_bytes() for p in directory.iterdir()}, before)
            self.assertFalse((prefix / ".install-lock").exists())

            # The same archive must still publish cleanly after rollback.
            directory = self.install(new, new_sums, prefix)
            profile = codec_worker.FLOAT_PROFILE if end == "v7" else codec_worker.AVIF_SDR_PROFILE
            self.assertEqual(codec_worker.read_manifest(directory)["profile"], profile)
            self.assertEqual({p.name for p in directory.iterdir()}, install.payload_names(self.target, profile))
            self.assertEqual((directory / codec_worker.FLOAT_NOTICE).exists(), end == "v7")
            codec_worker.verify_installed_manifest(directory, self.version, self.target)
            self.assertFalse((prefix / ".install-lock").exists())
