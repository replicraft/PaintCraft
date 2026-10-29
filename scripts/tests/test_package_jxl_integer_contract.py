"""Exact V9/V10 inventories; synthetic target headers never prove execution."""
import ast
import hashlib
import json
import re
import unittest

import codec_worker as codec
import package_release as release
import test_package_codec_worker as fixtures
import test_package_float_native as native_fixtures
import test_install_cli as installer
import install_cli as install
from test_package_release import executable


PROFILES = (
    (codec.JXL_INTEGER_BUILD_PREFIX, codec.JXL_INTEGER_BUILD_SUFFIX,
     codec.JXL_INTEGER_PROFILE, codec.JXL_INTEGER_VARIANTS, 2, "base"),
    (codec.JXL_INTEGER_JXR_BUILD_PREFIX, codec.JXL_INTEGER_JXR_BUILD_SUFFIX,
     codec.JXL_INTEGER_JXR_PROFILE, codec.JXL_INTEGER_JXR_VARIANTS, 3, "writer"),
)


class IntegerJxlContractTests(unittest.TestCase):
    setUp = fixtures.CodecWorkerTests.setUp

    def helper_for(self, target, profile):
        prefix, suffix = profile[:2]
        executable(self.binary, target)
        helper = self.binary.parent / codec.filename(target)
        helper.write_bytes(helper.read_bytes().replace(codec.BUILD_PREFIX, prefix)
                           .replace(codec.BUILD_SUFFIX, suffix))
        return helper

    def test_records_match_both_actual_rust_contracts(self):
        path = release.ROOT / "crates/services/paintcraft-formats/src/codec_worker/jxl_integer/contracts.rs"
        source = path.read_text()
        for prefix, suffix, profile, variants, schema, module in PROFILES:
            body = source.split("pub mod " + module + " {", 1)[1]
            body = body.split("concat!(", 1)[1].split(").as_bytes()", 1)[0]
            body = body.replace('env!("CARGO_PKG_VERSION")', json.dumps(self.version))
            expected = "".join(ast.literal_eval(literal) for literal in
                               re.findall(r'"(?:[^"\\]|\\.)*"', body)).encode()
            self.assertEqual(prefix + self.version.encode() + suffix, expected)
            self.assertIn('"' + profile + '"', source)
            self.assertEqual(len(variants), 8 if schema == 2 else 9)
            self.assertEqual(variants[:-1], codec.FLOAT_VARIANTS if schema == 2 else codec.JXR_VARIANTS)
            self.assertEqual(variants[-1], codec.JXL_INTEGER_VARIANT)
        self.assertNotIn(codec.JXL_INTEGER_VARIANT, codec.FLOAT_VARIANTS)
        self.assertNotIn(codec.JXL_INTEGER_VARIANT, codec.JXR_VARIANTS)

    def test_all_targets_preserve_exact_bytes_manifests_and_notices(self):
        for target in release.TARGETS:
            for contract in PROFILES:
                with self.subTest(target=target, profile=contract[2]):
                    helper = self.helper_for(target, contract)
                    output = self.root / (target + contract[-1])
                    output.mkdir()
                    copied = codec.copy(self.binary, output, self.version, target)
                    value = codec.read_manifest(output)
                    self.assertEqual(value["profile"], contract[2])
                    self.assertEqual(value["schema_version"], contract[4])
                    self.assertEqual(value["qualified_import_variants"], contract[3])
                    self.assertEqual(value.get("qualified_export_variants"),
                                     codec.JXR_EXPORT_VARIANTS if contract[4] == 3 else None)
                    self.assertEqual(value.get("encode_protocol_version"), 1 if contract[4] == 3 else None)
                    self.assertEqual(len(value), 12 if contract[4] == 3 else 10)
                    self.assertEqual(copied.read_bytes(), helper.read_bytes())
                    self.assertEqual(value["sha256"], hashlib.sha256(helper.read_bytes()).hexdigest())
                    self.assertEqual(codec.installed_notices(output), codec.NOTICES + (codec.FLOAT_NOTICE,))
                    codec.verify_manifest(output, self.version, target)
                    copied.write_bytes(copied.read_bytes() + b"changed bytes")
                    with self.assertRaises(ValueError):
                        codec.verify_manifest(output, self.version, target)

    def test_unknown_mixed_duplicate_private_and_malformed_declarations_refuse(self):
        legacy = (
            (codec.FLOAT_BUILD_PREFIX, codec.FLOAT_BUILD_SUFFIX),
            (codec.JXR_BUILD_PREFIX, codec.JXR_BUILD_SUFFIX),
            (codec.PRIVATE_JXR_BUILD_PREFIX, codec.JXR_BUILD_SUFFIX),
        )
        for contract in PROFILES:
            helper = self.helper_for(self.target, contract)
            original = helper.read_bytes()
            for prefix, suffix in legacy + tuple(c[:2] for c in PROFILES):
                for version in (self.version.encode(), b"0.0.0", b"v-stale"):
                    with self.subTest(profile=contract[2], marker=prefix, version=version):
                        helper.write_bytes(original + prefix + version + suffix)
                        with self.assertRaises(ValueError):
                            codec.prepare(self.binary, self.version, self.target)
            for bad in (original.replace(b"ROUND257", b"ROUND256"),
                        original.replace(b"REPORT96", b"REPORT95"),
                        original.replace(b"PIXELS4194304", b"PIXELS4194305"),
                        original.replace(self.version.encode(), b"0.0.0")):
                helper.write_bytes(bad)
                with self.assertRaises(ValueError):
                    codec.prepare(self.binary, self.version, self.target)
            helper.write_bytes(original + b"".join(prefix + b"library_constant\0"
                                                  for prefix, _ in legacy + tuple(c[:2] for c in PROFILES)))
            self.assertEqual(codec.verify_build_record(helper, self.version)[0], contract[2])

    def test_manifest_requires_exact_selected_inventory_and_writer_role(self):
        for contract in PROFILES:
            self.helper_for(self.target, contract)
            value = codec.installed_metadata(self.root, self.version, self.target)
            changes = [dict(schema_version=3 if contract[4] == 2 else 2),
                       dict(schema_version=True), dict(unknown=True),
                       dict(qualified_import_variants=contract[3][:-1]),
                       dict(qualified_import_variants=list(reversed(contract[3]))),
                       dict(qualified_import_variants=contract[3] + [codec.JXL_INTEGER_VARIANT]),
                       dict(profile="private-" + contract[2]),
                       dict(encode_protocol_version=2), dict(qualified_export_variants=[])]
            for change in changes:
                with self.subTest(profile=contract[2], change=change), self.assertRaises(ValueError):
                    codec.parse_manifest(json.dumps(value | change).encode())
            for field in ("encode_protocol_version", "qualified_export_variants"):
                changed = dict(value)
                if contract[4] == 3:
                    del changed[field]
                else:
                    changed[field] = 1 if field == "encode_protocol_version" else codec.JXR_EXPORT_VARIANTS
                with self.assertRaises(ValueError):
                    codec.parse_manifest(json.dumps(changed).encode())
            duplicate = json.dumps(value).replace('"schema_version":', '"schema_version": 2, "schema_version":', 1)
            with self.assertRaises(ValueError):
                codec.parse_manifest(duplicate.encode())

    def test_maximum_version_record_and_oversized_versions(self):
        version = "1.0.0-" + "a" * 58
        for prefix, suffix, profile, *_ in PROFILES:
            self.helper.write_bytes(prefix + version.encode() + suffix)
            self.assertEqual(codec.verify_build_record(self.helper, version)[0], profile)
            self.helper.write_bytes(prefix + version.encode() + b"x" + suffix)
            with self.assertRaises(ValueError):
                codec.verify_build_record(self.helper, version)


class IntegerJxlInstallerTests(unittest.TestCase):
    setUp = installer.InstallTests.setUp
    install = installer.InstallTests.install

    def test_both_profiles_install_exact_payload_for_every_target(self):
        import package_cli
        for target in release.TARGETS:
            for prefix, suffix, profile, *_ in PROFILES:
                with self.subTest(target=target, profile=profile):
                    directory = self.root / (target + profile)
                    directory.mkdir()
                    binary = directory / install.executable(target)
                    executable(binary, target)
                    helper = directory / codec.filename(target)
                    helper.write_bytes(helper.read_bytes().replace(codec.BUILD_PREFIX, prefix)
                                       .replace(codec.BUILD_SUFFIX, suffix))
                    archive = package_cli.package(self.version, target, binary, directory / "archives")
                    sums = archive.parent / "SHA256SUMS.txt"
                    sums.write_bytes(release.checksum_line(archive))
                    output = self.install(archive, sums, directory / "prefix", target)
                    self.assertEqual(codec.read_manifest(output)["profile"], profile)
                    self.assertEqual({p.name for p in output.iterdir()}, install.payload_names(target, profile))
                    codec.verify_installed_manifest(output, self.version, target)


class IntegerJxlNativeLayoutTests(native_fixtures.FloatNativeTests):
    """Native layout, license and post-sign hash controls, without OS execution."""
    contract = PROFILES[0]

    def final(self, target):
        prefix, suffix = self.contract[:2]
        executable(self.binary, target)
        helper = self.binary.parent / codec.filename(target)
        helper.write_bytes(helper.read_bytes().replace(codec.BUILD_PREFIX, prefix)
                           .replace(codec.BUILD_SUFFIX, suffix))


class IntegerJxlWriterNativeLayoutTests(IntegerJxlNativeLayoutTests):
    contract = PROFILES[1]
