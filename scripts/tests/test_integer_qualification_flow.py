"""Synthetic orchestration controls, never native execution qualification.

Only the process runner and host probe are simulated. Archive creation, checksum
verification, installation, architecture headers and finite helper records use
the production implementations. No synthetic executable may be launched.
"""
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import codec_worker
import helper_policy
import package_release as release
import qualify_cli_helper as cli_tests
import qualify_native_codecs as qualification
from test_package_release import executable


WORKER_TESTS = (
    "actual_integer_family_uses_one_request_and_exact_report_for_both_source_depths",
    "old_exact8_request_never_adopts16_and_new_family_refuses_other_profiles",
)
CLI_INTEGER = "jxl_integer::actual_cli_and_mcp_integer_jxl_consent_pixels_receipts_and_guards"
APP_INTEGER = "importing::native_codec::jxl_integer_installed_tests::copied_jxl_integer_helper_runs_actual_app_review_and_failure_guards"
APP_PORTABLE_JXR = "jxr_export::installed_tests::portable::fixture_installed_app_writer_matches_c_qualified_goldens"
FLOAT_IDENTITY = "codec_worker::native::float::identity_tests::installed_float_settings_have_distinct_stable_converted_document_identities"
CORE_GROUPS = (
    "dds::", "export::dds::", "export::jxr::",
    "codec_worker::native::jxr_encode::tests::",
    "codec_worker::native::import::family_tests::",
    "codec_worker::jxl_integer::",
    "codec_worker::native::jxl_integer::tests::",
    "codec_worker::native::import::jxl_integer::tests::",
)
COMMIT = "1" * 40


class Scenario:
    def __init__(self, root, candidate, target, *, mutation=None):
        self.root, self.candidate, self.target = root, candidate, target
        self.mutation = mutation
        self.calls = []
        self.build = root / "build"
        self.build.mkdir()
        self.output = root / "qualification"
        self.listings = {}
        self.features = ["float-codecs", "jxl-integer-codec"]
        if candidate == "v10":
            self.features += ["jxr-codecs", "jxr-encode"]
        self.helper = self.build / codec_worker.filename(target)
        executable(self.helper, target, with_helper=False)
        prefix, suffix = (
            (codec_worker.JXL_INTEGER_BUILD_PREFIX, codec_worker.JXL_INTEGER_BUILD_SUFFIX)
            if candidate == "v9" else
            (codec_worker.JXL_INTEGER_JXR_BUILD_PREFIX, codec_worker.JXL_INTEGER_JXR_BUILD_SUFFIX)
        )
        self.helper.write_bytes(self.helper.read_bytes()
                                .replace(codec_worker.BUILD_PREFIX, prefix)
                                .replace(codec_worker.BUILD_SUFFIX, suffix))
        self.cli = self.build / ("paintcraft-cli.exe" if "windows" in target else "paintcraft-cli")
        executable(self.cli, target, with_helper=False)
        self.worker = [self.artifact("paintcraft-codec-worker", self.helper, test=False,
                                     features=[f for f in self.features
                                               if mutation != "missing-feature" or f != "jxl-integer-codec"])]
        for name in sorted(qualification.REQUIRED_WORKER_TARGETS):
            names = ["unrelated_control"]
            if name == "native_jxl_integer":
                names += [value for value in WORKER_TESTS
                          if mutation != "missing-worker-" + str(WORKER_TESTS.index(value))]
                if mutation == "duplicate-worker":
                    names.append(WORKER_TESTS[0])
            self.worker.append(self.test_artifact(name, names, prefix="worker-"))
        cli_names = [value for value in cli_tests.selectors(candidate)
                     if mutation != "missing-cli" or value != CLI_INTEGER]
        self.cli_test = self.test_artifact("installed_codec_worker", ["ordinary_control", *cli_names])
        self.cli_record = self.artifact("paintcraft-cli", self.cli, test=False, kind=["bin"])
        self.dds_cli = self.test_artifact("dds_texture_export", ["dds_cli_control"])
        formats_names = [p + "control" for p in CORE_GROUPS]
        if mutation != "missing-float-identity":
            formats_names.append(FLOAT_IDENTITY)
        self.formats = self.test_artifact("paintcraft_formats", formats_names)
        self.platform = self.test_artifact("paintcraft_platform", ["codec_helper::control"])
        self.dds = self.test_artifact("dds_texture_write", [
            "dds_texture_vectors_match_independently_measured_frozen_specimens"])
        app_names = [p + "control" for p in qualification.APP_GROUPS]
        if mutation == "missing-pressure":
            app_names = [name for name in app_names if not name.startswith("framework_force::tests::")]
        app_names += [qualification.APP_LEGACY]
        if mutation != "missing-app":
            app_names.append(APP_INTEGER)
        if mutation != "missing-app-writer":
            app_names.append(APP_PORTABLE_JXR)
        self.app = self.test_artifact("paintcraft", app_names)

    @staticmethod
    def artifact(name, path, *, test, features=(), kind=()):
        return {"reason": "compiler-artifact", "target": {"name": name, "kind": list(kind)},
                "profile": {"test": test}, "features": list(features), "executable": str(path)}

    def test_artifact(self, name, names, prefix=""):
        path = self.build / (prefix + name + (".exe" if "windows" in self.target else ""))
        executable(path, self.target, with_helper=False)
        self.listings[path.name] = names
        return self.artifact(name, path, test=True)

    def runner(self, output):
        scenario = self
        class FakeRunner:
            def __init__(self):
                self.output = output
                self.output.mkdir(parents=True)

            def run(self, name, argv, **options):
                argv = list(map(str, argv))
                scenario.calls.append((name, argv, dict(options.get("env", {}))))
                path = self.output / (name + ".stdout")
                builds = {
                    "build-worker": scenario.worker,
                    "build-cli": [scenario.cli_record, scenario.cli_test],
                    "build-dds-cli": [scenario.cli_record, scenario.dds_cli],
                    "build-core": [scenario.formats, scenario.platform],
                    "build-dds-golden": [scenario.dds], "build-app": [scenario.app],
                }
                if name in builds:
                    if argv[0] != "cargo":
                        raise AssertionError("Build must pass through the simulated Cargo runner")
                    if name == "build-app" and scenario.mutation == "oversized-app":
                        with Path(scenario.app["executable"]).open("ab") as stream:
                            stream.truncate(qualification.EXECUTABLE_MAX + 123)
                    value = "\n".join(map(json.dumps, builds[name]))
                elif name in ("source", "source-final"):
                    if argv != ["git", "rev-parse", "HEAD"]:
                        raise AssertionError("Source identity command changed")
                    value = ("2" * 40 if name == "source-final" and
                             scenario.mutation == "changed-commit" else COMMIT) + "\n"
                elif name in ("source-status", "source-final-status"):
                    if argv != ["git", "status", "--porcelain"]:
                        raise AssertionError("Source cleanliness command changed")
                    dirty = ((name == "source-status" and scenario.mutation == "dirty-start") or
                             (name == "source-final-status" and scenario.mutation == "dirty-finish"))
                    value = " M tracked-source.rs\n" if dirty else ""
                elif name == "cargo-version":
                    value = "cargo simulated\n"
                else:
                    filename = Path(argv[0]).name
                    if filename not in scenario.listings:
                        raise AssertionError("Unrecognized synthetic executable: " + repr(argv))
                    names = scenario.listings[filename]
                    if "--list" in argv:
                        value = "".join(name + ": test\n" for name in names)
                    else:
                        # libtest exits successfully for a nonexistent exact selector.
                        # Model that behavior so the production gates must catch it.
                        if "--exact" in argv:
                            count = names.count(argv[1])
                        elif len(argv) > 1 and not argv[1].startswith("--"):
                            count = sum(n.startswith(argv[1]) for n in names)
                        else:
                            count = 1
                        if ((scenario.mutation == "zero-cli" and CLI_INTEGER in argv) or
                                (scenario.mutation == "zero-app" and APP_INTEGER in argv) or
                                (scenario.mutation == "zero-app-writer" and APP_PORTABLE_JXR in argv) or
                                (scenario.mutation == "zero-float-identity" and FLOAT_IDENTITY in argv)):
                            count = 0
                        value = f"running {count} tests\ntest result: ok. {count} passed; 0 failed;\n"
                copied = scenario.output / "app-installation"
                if name == "float-document-identity" and scenario.mutation == "fixture-output":
                    (copied / "generated-fixture-output.pcnd").write_bytes(b"owned output")
                if name == "app-integer-jxl" and (scenario.mutation or "").startswith("residue-"):
                    selected = {
                        "residue-helper": scenario.helper.name,
                        "residue-manifest": codec_worker.MANIFEST,
                        "residue-notice": codec_worker.NOTICES[0],
                        "residue-oversize": scenario.helper.name,
                        "residue-symlink": scenario.helper.name,
                        "residue-directory": codec_worker.NOTICES[0],
                        "residue-app-size": Path(scenario.app["executable"]).name,
                        "residue-formats": Path(scenario.formats["executable"]).name,
                    }[scenario.mutation]
                    changed = copied / selected
                    if scenario.mutation in ("residue-oversize", "residue-app-size"):
                        with changed.open("r+b") as stream:
                            stream.truncate(qualification.EXECUTABLE_MAX + 123)
                    elif scenario.mutation == "residue-symlink":
                        changed.unlink()
                        changed.symlink_to(scenario.helper)
                    elif scenario.mutation == "residue-directory":
                        changed.unlink()
                        changed.mkdir()
                    else:
                        raw = bytearray(changed.read_bytes())
                        raw[-1] ^= 1
                        changed.write_bytes(raw)
                path.write_text(value, encoding="utf-8")
                return path
        return FakeRunner()

    def execute(self):
        copy_file = qualification.shutil.copy2
        copy_tree = qualification.shutil.copytree
        copied = self.output / "app-installation"

        def corrupt(path):
            raw = bytearray(path.read_bytes())
            raw[-1] ^= 1
            path.write_bytes(raw)

        def copying_file(source, destination, *args, **kwargs):
            result = copy_file(source, destination, *args, **kwargs)
            target = Path(destination)
            if target.parent == copied:
                expected = (self.app if self.mutation == "corrupt-app-copy" else self.formats)
                if self.mutation in ("corrupt-app-copy", "corrupt-formats-copy") and target.name == Path(expected["executable"]).name:
                    corrupt(target)
            return result

        def copying_tree(source, destination, *args, **kwargs):
            result = copy_tree(source, destination, *args, **kwargs)
            if Path(destination) == copied and self.mutation == "corrupt-payload-copy":
                corrupt(copied / self.helper.name)
            return result

        # A leaked call cannot launch even a header-only test image or real Cargo.
        with patch.object(qualification.shutil, "copy2", copying_file), \
             patch.object(qualification.shutil, "copytree", copying_tree), \
             patch.object(qualification, "Runner", self.runner), \
             patch.object(qualification.host, "identify", return_value={
                 "target": self.target, "scope": "synthetic test only"}), \
             patch("subprocess.run", side_effect=AssertionError("Synthetic flow launched a process")), \
             patch("subprocess.Popen", side_effect=AssertionError("Synthetic flow spawned a process")):
            qualification.qualify(self.target, self.output, target_directory=self.build,
                                  candidate=self.candidate)

    def receipt(self):
        return json.loads((self.output / "receipt.json").read_text())

    def phases(self):
        return [name for name, _, _ in self.calls]


class IntegerQualificationFlowTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="paintcraft-synthetic-integer-flow-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()

    def scenario(self, candidate, target="aarch64-apple-darwin", mutation=None):
        root = self.root / (candidate + "-" + target + "-" + str(mutation))
        root.mkdir()
        return Scenario(root, candidate, target, mutation=mutation)

    def test_exact_roles_complete_real_packaging_without_native_execution_or_promotion(self):
        policy = dict(helper_policy.ROLLOUT)
        self.assertEqual(tuple(qualification.INTEGER_WORKER_TESTS), WORKER_TESTS)
        self.assertEqual(qualification.APP_INTEGER, APP_INTEGER)
        for candidate in ("v9", "v10"):
            for target in release.TARGETS:
                with self.subTest(candidate=candidate, target=target):
                    flow = self.scenario(candidate, target)
                    flow.execute()
                    receipt = flow.receipt()
                    self.assertEqual(receipt["status"], "passed")
                    self.assertTrue(receipt["native_runtime_qualified"])
                    self.assertTrue(receipt["shipping_policy_unchanged"])
                    self.assertEqual(receipt["selected_candidate"], candidate)
                    self.assertEqual(receipt["source_commit"], COMMIT)
                    self.assertEqual(receipt["source_dirty"], "")
                    self.assertFalse(receipt["oracle_executed"])
                    app_build = receipt["app_build"]
                    self.assertEqual(app_build["role"], "headless-app-test")
                    self.assertEqual(app_build["strip"], "debuginfo" if "linux" in target else "workspace-default")
                    self.assertEqual(app_build["cargo_profile"], {"test": True})
                    self.assertEqual(app_build["binary"], qualification.binary_facts(Path(flow.app["executable"])))
                    self.assertEqual(app_build["executable_limit"], 256 << 20)
                    metadata = receipt["candidate"]["manifest"]
                    self.assertEqual(metadata["schema_version"], 2 if candidate == "v9" else 3)
                    self.assertEqual(metadata["profile"], qualification.CANDIDATES[candidate][1])
                    self.assertEqual(metadata["qualified_import_variants"],
                                     codec_worker.JXL_INTEGER_VARIANTS if candidate == "v9"
                                     else codec_worker.JXL_INTEGER_JXR_VARIANTS)
                    if candidate == "v9":
                        self.assertNotIn("qualified_export_variants", metadata)
                        self.assertNotIn("encode_protocol_version", metadata)
                        self.assertNotIn("jxr-codecs", receipt["worker_features"])
                    else:
                        self.assertEqual(metadata["qualified_export_variants"], codec_worker.JXR_EXPORT_VARIANTS)
                        self.assertEqual(metadata["encode_protocol_version"], 1)
                    installed = Path(receipt["candidate"]["installation"])
                    self.assertEqual((installed / flow.cli.name).read_bytes(), flow.cli.read_bytes())
                    self.assertEqual((installed / flow.helper.name).read_bytes(), flow.helper.read_bytes())
                    self.assertTrue(list((flow.output / "candidate/archives").glob("SHA256SUMS.txt")))
                    phases = flow.phases()
                    for name in ("integer-worker-0", "integer-worker-1", "cli-ordinary", "float-document-identity",
                                 "app-legacy-jxr", "app-integer-jxl", "source-final", "source-final-status"):
                        self.assertEqual(phases.count(name), 1)
                    integer_cli = [row for row in flow.calls if CLI_INTEGER in row[1]]
                    self.assertEqual(len(integer_cli), 1)
                    for selector in (*cli_tests.BASELINE, *(cli_tests.JXR if candidate == "v10" else ())):
                        commands = [argv for _, argv, _ in flow.calls if selector in argv]
                        self.assertEqual(len(commands), 1, selector)
                        self.assertIn("--exact", commands[0])
                        self.assertIn("--ignored", commands[0])
                    if candidate == "v9":
                        self.assertFalse(any("jxr_export::" in arg for name, argv, _ in flow.calls
                                             if name.startswith("cli-") for arg in argv))
                    worker = next(argv for name, argv, _ in flow.calls if name == "build-worker")
                    self.assertIn("--no-default-features", worker)
                    self.assertEqual(worker[worker.index("--features") + 1],
                                     "jxl-integer-codec" if candidate == "v9"
                                     else "jxr-codecs,jxl-integer-codec")
                    app = next(row for row in flow.calls if row[0] == "app-integer-jxl")
                    for prefix in ("framework_force::tests::", "controller::framework_force::tests::"):
                        self.assertEqual(sum(prefix in argv for _, argv, _ in flow.calls), 1)
                    self.assertIn("--exact", app[1])
                    self.assertIn("--ignored", app[1])
                    self.assertEqual(app[2]["PAINTCRAFT_JXL_INTEGER_HELPER"], str(installed / flow.helper.name))
                    self.assertLess(phases.index("cli-ordinary"), phases.index("app-integer-jxl"))
                    self.assertLess(phases.index("app-integer-jxl"), phases.index("source-final"))
                    writer = [row for row in flow.calls if row[0] == "app-jxr-portable"]
                    self.assertEqual(len(writer), int(candidate == "v10"))
                    if writer:
                        self.assertEqual(writer[0][1][1], APP_PORTABLE_JXR)
                        self.assertIn("--exact", writer[0][1])
                        self.assertIn("--ignored", writer[0][1])
                        self.assertEqual(writer[0][2]["PAINTCRAFT_JXR_PUBLIC_HELPER"],
                                         str(flow.output / "app-installation" / flow.helper.name))
                        self.assertLess(phases.index("app-jxr-portable"), phases.index("source-final"))
                    identity = next(row for row in flow.calls if row[0] == "float-document-identity")
                    self.assertEqual(identity[1][1], FLOAT_IDENTITY)
                    self.assertIn("--exact", identity[1])
                    self.assertIn("--ignored", identity[1])
                    self.assertEqual(Path(identity[1][0]).parent, flow.output / "app-installation")
                    self.assertEqual(receipt["phases"]["float_conversion_identity"], FLOAT_IDENTITY)
        self.assertEqual(helper_policy.ROLLOUT, policy)

    def test_copied_payload_and_cargo_artifacts_match_before_execution(self):
        for candidate in ("v9", "v10"):
            for target in release.TARGETS:
                for mutation in ("corrupt-payload-copy", "corrupt-app-copy", "corrupt-formats-copy"):
                    with self.subTest(candidate=candidate, target=target, mutation=mutation):
                        flow = self.scenario(candidate, target, mutation=mutation)
                        self.assert_failed(flow, "Copied qualification payload bytes differ",
                                           forbidden="float-document-identity")
                        self.assertNotIn("source-final", flow.phases())

    def test_copied_payload_residue_refuses_before_qualification(self):
        for candidate in ("v9", "v10"):
            for target in release.TARGETS:
                for mutation in ("residue-helper", "residue-manifest", "residue-notice",
                                 "residue-oversize", "residue-symlink", "residue-directory",
                                 "residue-app-size", "residue-formats"):
                    with self.subTest(candidate=candidate, target=target, mutation=mutation):
                        flow = self.scenario(candidate, target, mutation=mutation)
                        message = ("bounded regular file" if mutation in
                                   ("residue-oversize", "residue-symlink", "residue-directory", "residue-app-size")
                                   else "Copied qualification payload bytes differ")
                        self.assert_failed(flow, message, forbidden="source-final")
                        self.assertIn("app-integer-jxl", flow.phases())

    def test_generated_fixture_outputs_do_not_change_original_payload_inventory(self):
        for candidate in ("v9", "v10"):
            flow = self.scenario(candidate, mutation="fixture-output")
            flow.execute()
            self.assertEqual(flow.receipt()["status"], "passed")
            self.assertTrue(flow.receipt()["phases"]["copied_payload_preflight"])
            self.assertTrue(flow.receipt()["phases"]["copied_payload_postflight"])

    def test_size_and_file_kind_refusal_happens_before_digest_reads(self):
        for kind in ("oversize", "different-size", "symlink", "directory"):
            with self.subTest(kind=kind):
                path = self.root / kind
                if kind == "directory":
                    path.mkdir()
                elif kind == "symlink":
                    target = self.root / "symlink-target"
                    target.write_bytes(b"four")
                    path.symlink_to(target)
                else:
                    with path.open("wb") as stream:
                        stream.truncate(qualification.EXECUTABLE_MAX + 1 if kind == "oversize" else 5)
                with patch.object(qualification, "digest", side_effect=AssertionError("unbounded read")):
                    with self.assertRaisesRegex(ValueError, "bounded regular file|size differs"):
                        qualification.verify_copied_file(path, 4, "0" * 64)

    def test_float_identity_gate_cannot_be_missing_or_execute_zero_tests(self):
        self.assertEqual(qualification.FLOAT_IDENTITY, FLOAT_IDENTITY)
        for candidate in ("v9", "v10"):
            for target in release.TARGETS:
                for mutation, message in (("missing-float-identity", "Missing or ambiguous"),
                                          ("zero-float-identity", "count")):
                    with self.subTest(candidate=candidate, target=target, mutation=mutation):
                        flow = self.scenario(candidate, target, mutation=mutation)
                        self.assert_failed(flow, message, forbidden="app-legacy-jxr")
                        self.assertNotIn("source-final", flow.phases())

    def test_portable_writer_gate_is_exact_and_mandatory_on_every_v10_target(self):
        self.assertEqual(qualification.APP_PORTABLE_JXR, APP_PORTABLE_JXR)
        for target in release.TARGETS:
            for mutation, message in (("missing-app-writer", "Missing or ambiguous"),
                                      ("zero-app-writer", "count")):
                with self.subTest(target=target, mutation=mutation):
                    flow = self.scenario("v10", target, mutation=mutation)
                    self.assert_failed(flow, message, forbidden="source-final")
                    self.assertIn("app-integer-jxl", flow.phases())

    def test_reader_only_v9_does_not_require_or_run_the_writer_gate(self):
        flow = self.scenario("v9", mutation="missing-app-writer")
        flow.execute()
        self.assertEqual(flow.receipt()["status"], "passed")
        self.assertNotIn("app-jxr-portable", flow.phases())

    def test_app_bound_still_refuses_and_retains_actual_size_before_any_app_execution(self):
        flow = self.scenario("v9", "x86_64-unknown-linux-gnu", mutation="oversized-app")
        self.assert_failed(flow, "bounded regular file", forbidden="app-list")
        receipt = flow.receipt()
        self.assertEqual(receipt["app_build"]["binary"]["bytes"], qualification.EXECUTABLE_MAX + 123)
        self.assertEqual(receipt["app_build"]["strip"], "debuginfo")
        self.assertIn(f"bytes={qualification.EXECUTABLE_MAX + 123}", receipt["error"])
        self.assertNotIn("source-final", flow.phases())

    def test_missing_integer_worker_names_refuse_even_with_unrelated_passing_test(self):
        for candidate in ("v9", "v10"):
            for number in range(2):
                with self.subTest(candidate=candidate, number=number):
                    flow = self.scenario(candidate, mutation="missing-worker-" + str(number))
                    self.assert_failed(flow, "Missing or ambiguous", forbidden="cli-list")
                    self.assertNotIn("integer-worker-" + str(number), flow.phases())

    def test_missing_actual_app_fixture_cannot_be_replaced_by_other_app_tests(self):
        for candidate in ("v9", "v10"):
            with self.subTest(candidate=candidate):
                flow = self.scenario(candidate, mutation="missing-app")
                self.assert_failed(flow, "Missing or ambiguous", forbidden="app-integer-jxl")
                self.assertIn("app-legacy-jxr", flow.phases())
                self.assertNotIn("source-final", flow.phases())

    def test_missing_framework_pressure_controls_prevent_native_qualification(self):
        for candidate in ("v9", "v10"):
            flow = self.scenario(candidate, mutation="missing-pressure")
            self.assert_failed(flow, "Required native test group missing", forbidden="app-integer-jxl")
            self.assertNotIn("source-final", flow.phases())

    def test_missing_cli_integer_selector_cannot_be_replaced_by_baseline_tests(self):
        for candidate in ("v9", "v10"):
            with self.subTest(candidate=candidate):
                flow = self.scenario(candidate, mutation="missing-cli")
                self.assert_failed(flow, "Missing or ambiguous", forbidden="build-dds-cli")
                self.assertIn("cli-ordinary", flow.phases())
                self.assertFalse(any(CLI_INTEGER in argv for _, argv, _ in flow.calls))

    def test_ambiguous_worker_fixture_cannot_be_treated_as_one_exact_test(self):
        for candidate in ("v9", "v10"):
            with self.subTest(candidate=candidate):
                self.assert_failed(self.scenario(candidate, mutation="duplicate-worker"),
                                   "Missing or ambiguous", forbidden="integer-worker-0")

    def test_zero_executed_exact_cli_or_app_is_never_a_green_candidate(self):
        for candidate in ("v9", "v10"):
            for mutation, forbidden in (("zero-cli", "build-dds-cli"), ("zero-app", "source-final")):
                with self.subTest(candidate=candidate, mutation=mutation):
                    self.assert_failed(self.scenario(candidate, mutation=mutation), "count", forbidden=forbidden)

    def test_missing_integer_feature_refuses_before_cli_or_tests(self):
        for candidate in ("v9", "v10"):
            with self.subTest(candidate=candidate):
                self.assert_failed(self.scenario(candidate, mutation="missing-feature"), "features",
                                   forbidden="build-cli")

    def test_dirty_source_refuses_before_any_build(self):
        for candidate in ("v9", "v10"):
            with self.subTest(candidate=candidate):
                flow = self.scenario(candidate, mutation="dirty-start")
                self.assert_failed(flow, "clean committed", forbidden="build-worker")
                self.assertEqual(flow.phases(), ["source", "source-status"])

    def test_source_commit_or_cleanliness_change_after_all_tests_refuses_publication(self):
        for candidate in ("v9", "v10"):
            for mutation in ("changed-commit", "dirty-finish"):
                with self.subTest(candidate=candidate, mutation=mutation):
                    flow = self.scenario(candidate, mutation=mutation)
                    self.assert_failed(flow, "Source changed")
                    self.assertEqual(flow.phases()[-2:], ["source-final", "source-final-status"])
                    self.assertIn("app-integer-jxl", flow.phases())

    def assert_failed(self, flow, message, *, forbidden=None):
        policy = dict(helper_policy.ROLLOUT)
        with self.assertRaisesRegex(ValueError, message):
            flow.execute()
        receipt = flow.receipt()
        self.assertEqual(receipt["status"], "failed")
        self.assertFalse(receipt["native_runtime_qualified"])
        self.assertTrue(receipt["shipping_policy_unchanged"])
        self.assertIn(message.split()[0], receipt["error"])
        if forbidden:
            self.assertNotIn(forbidden, flow.phases())
        self.assertEqual(helper_policy.ROLLOUT, policy)


if __name__ == "__main__":
    unittest.main()
