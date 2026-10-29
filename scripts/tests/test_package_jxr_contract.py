"""Exact public V8 package metadata; synthetic headers are never executed."""
import ast
import hashlib
import json
import re
import unittest

import codec_worker
import package_release as release
import test_package_codec_worker as fixtures
import test_package_float_native as native_fixtures
import test_install_cli as install_fixtures
import install_cli as install
from test_package_release import executable


class JxrContractTests(unittest.TestCase):
    setUp = fixtures.CodecWorkerTests.setUp

    def final(self, target):
        executable(self.binary, target)
        helper = self.binary.parent / codec_worker.filename(target)
        helper.write_bytes(helper.read_bytes().replace(codec_worker.BUILD_PREFIX, codec_worker.JXR_BUILD_PREFIX)
                           .replace(codec_worker.BUILD_SUFFIX, codec_worker.JXR_BUILD_SUFFIX))
        return helper

    def test_public_record_matches_actual_rust_contract_source_exactly(self):
        source = (release.ROOT / "crates/services/paintcraft-formats/src/codec_worker/jxr_encode/qualified.rs").read_text()
        body = source.split("concat!(", 1)[1].split(").as_bytes()", 1)[0]
        body = body.replace('env!("CARGO_PKG_VERSION")', json.dumps(self.version))
        expected = "".join(ast.literal_eval(literal) for literal in re.findall(r'"(?:[^"\\]|\\.)*"', body)).encode()
        self.assertEqual(codec_worker.JXR_BUILD_PREFIX + self.version.encode() + codec_worker.JXR_BUILD_SUFFIX, expected)
        self.assertIn('NATIVE_PROFILE: &str = "' + codec_worker.JXR_PROFILE + '"', source)
        self.assertEqual(codec_worker.JXR_VARIANTS[:7], codec_worker.FLOAT_VARIANTS)
        self.assertEqual(len(codec_worker.JXR_VARIANTS), 8)

    def test_all_target_headers_copy_exact_schema3_and_notices_without_execution(self):
        for target in release.TARGETS:
            with self.subTest(target=target):
                helper = self.final(target)
                output = self.root / target
                output.mkdir()
                copied = codec_worker.copy(self.binary, output, self.version, target)
                value = codec_worker.read_manifest(output)
                self.assertEqual(len(value), 12)
                self.assertEqual(value["schema_version"], 3)
                self.assertEqual(value["profile"], codec_worker.JXR_PROFILE)
                self.assertEqual(value["protocol_version"], 2)
                self.assertEqual(value["encode_protocol_version"], 1)
                self.assertEqual(value["qualified_import_variants"], codec_worker.JXR_VARIANTS)
                self.assertEqual(value["qualified_export_variants"], codec_worker.JXR_EXPORT_VARIANTS)
                self.assertEqual(value["sha256"], hashlib.sha256(copied.read_bytes()).hexdigest())
                self.assertEqual(copied.read_bytes(), helper.read_bytes())
                self.assertEqual(codec_worker.installed_notices(output), codec_worker.NOTICES + (codec_worker.FLOAT_NOTICE,))
                codec_worker.verify_manifest(output, self.version, target)
                copied.write_bytes(copied.read_bytes() + b"post-sign control")
                with self.assertRaises(ValueError):
                    codec_worker.verify_manifest(output, self.version, target)
                codec_worker.write_manifest(output, self.version, target)
                codec_worker.verify_manifest(output, self.version, target)

    def test_exact_schema_export_import_and_private_negative_controls(self):
        self.final(self.target)
        value = codec_worker.installed_metadata(self.root, self.version, self.target)
        changes = [dict(schema_version=2), dict(schema_version=True),
                   dict(encode_protocol_version=2), dict(encode_protocol_version=True),
                   dict(qualified_export_variants=[]),
                   dict(qualified_export_variants=codec_worker.JXR_EXPORT_VARIANTS * 2),
                   dict(qualified_import_variants=codec_worker.FLOAT_VARIANTS),
                   dict(qualified_import_variants=list(reversed(codec_worker.JXR_VARIANTS))),
                   dict(profile="private-" + codec_worker.JXR_PROFILE), dict(unknown=True)]
        for change in changes:
            with self.subTest(change=change), self.assertRaises(ValueError):
                codec_worker.parse_manifest(json.dumps(value | change).encode())
        for field in ["encode_protocol_version", "qualified_export_variants"]:
            changed = dict(value)
            del changed[field]
            with self.assertRaises(ValueError):
                codec_worker.parse_manifest(json.dumps(changed).encode())
        duplicate = json.dumps(value).replace('"schema_version": 3', '"schema_version": 3, "schema_version": 3')
        with self.assertRaises(ValueError):
            codec_worker.parse_manifest(duplicate.encode())
        correct = self.helper.read_bytes()
        for changed in [correct.replace(codec_worker.JXR_BUILD_PREFIX, codec_worker.PRIVATE_JXR_BUILD_PREFIX),
                        correct + codec_worker.PRIVATE_JXR_BUILD_PREFIX + self.version.encode() + codec_worker.JXR_BUILD_SUFFIX,
                        correct + codec_worker.FLOAT_BUILD_PREFIX + self.version.encode() + codec_worker.FLOAT_BUILD_SUFFIX,
                        correct + codec_worker.JXR_BUILD_PREFIX + self.version.encode() + codec_worker.JXR_BUILD_SUFFIX,
                        correct.replace(b"SEEK_CAP2", b"SEEK_CAP1"),
                        correct.replace(b"SIDES65536", b"SIDES65535")]:
            self.helper.write_bytes(changed)
            with self.assertRaises(ValueError):
                codec_worker.prepare(self.binary, self.version, self.target)

    def test_legacy_schema2_cannot_claim_encoder_fields(self):
        executable(self.binary, self.target)
        value = codec_worker.installed_metadata(self.root, self.version, self.target)
        self.assertEqual(value["schema_version"], 2)
        self.assertEqual(len(value), 10)
        with self.assertRaises(ValueError):
            codec_worker.parse_manifest(json.dumps(value | {"encode_protocol_version": 1,
                                                           "qualified_export_variants": codec_worker.JXR_EXPORT_VARIANTS}).encode())


class JxrNativeLayoutTests(native_fixtures.FloatNativeTests):
    """Reuse real package-layout controls with exact new profile, not new layouts."""
    def final(self, target):
        executable(self.binary, target)
        helper = self.binary.parent / codec_worker.filename(target)
        helper.write_bytes(helper.read_bytes().replace(codec_worker.BUILD_PREFIX, codec_worker.JXR_BUILD_PREFIX)
                           .replace(codec_worker.BUILD_SUFFIX, codec_worker.JXR_BUILD_SUFFIX))


class JxrInstallerTests(unittest.TestCase):
    setUp = install_fixtures.InstallTests.setUp
    install = install_fixtures.InstallTests.install

    def test_public_schema3_archives_install_exact_payload_on_all_synthetic_targets(self):
        import package_cli
        for target in release.TARGETS:
            with self.subTest(target=target):
                directory = self.root / target
                directory.mkdir()
                binary = directory / install.executable(target)
                executable(binary, target)
                helper = directory / codec_worker.filename(target)
                helper.write_bytes(helper.read_bytes().replace(codec_worker.BUILD_PREFIX, codec_worker.JXR_BUILD_PREFIX)
                                   .replace(codec_worker.BUILD_SUFFIX, codec_worker.JXR_BUILD_SUFFIX))
                archive = package_cli.package(self.version, target, binary, directory / "archives")
                sums = archive.parent / "SHA256SUMS.txt"
                sums.write_bytes(release.checksum_line(archive))
                output = self.install(archive, sums, directory / "prefix", target)
                self.assertEqual(codec_worker.read_manifest(output)["profile"], codec_worker.JXR_PROFILE)
                self.assertEqual({p.name for p in output.iterdir()}, install.payload_names(target, codec_worker.JXR_PROFILE))
                codec_worker.verify_installed_manifest(output, self.version, target)
