"""Candidate orchestration controls; six synthetic headers are never executed."""
import contextlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import codec_worker
import helper_policy
import native_qualification_host as host
from native_qualification_process import Runner
import package_release as release
import qualify_native_codecs as qualification
import qualify_cli_helper
from test_package_release import executable


class CandidateTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="paintcraft-candidate-contract-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.version = release.workspace_version()

    def candidate(self, target, name):
        directory = self.root / name
        directory.mkdir()
        cli = directory / ("paintcraft-cli.exe" if "windows" in target else "paintcraft-cli")
        executable(cli, target)
        helper = directory / codec_worker.filename(target)
        helper.write_bytes(helper.read_bytes().replace(codec_worker.BUILD_PREFIX, codec_worker.JXR_BUILD_PREFIX)
                           .replace(codec_worker.BUILD_SUFFIX, codec_worker.JXR_BUILD_SUFFIX))
        return cli, helper

    def test_candidate_all_six_exact_installs_do_not_weaken_shipping_holdbacks(self):
        before = dict(helper_policy.ROLLOUT)
        for index, target in enumerate(release.TARGETS):
            with self.subTest(target=target):
                cli, helper = self.candidate(target, str(index))
                installed = qualification.prepare_candidate(cli, target, self.root / (str(index) + "-out"))
                metadata = codec_worker.read_manifest(installed)
                self.assertEqual(metadata["schema_version"], 3)
                self.assertEqual(metadata["qualified_import_variants"], codec_worker.JXR_VARIANTS)
                self.assertEqual(metadata["qualified_export_variants"], codec_worker.JXR_EXPORT_VARIANTS)
                self.assertEqual((installed / helper.name).read_bytes(), helper.read_bytes())
                self.assertEqual((installed / cli.name).read_bytes(), cli.read_bytes())
                if target != "aarch64-apple-darwin":
                    with self.assertRaisesRegex(ValueError, "rollout"):
                        helper_policy.verify(cli, self.version, target)
        self.assertEqual(before, helper_policy.ROLLOUT)

    def test_unknown_private_duplicate_wrong_target_and_forged_schema_refuse(self):
        target = "aarch64-apple-darwin"
        cli, helper = self.candidate(target, "negative")
        original = helper.read_bytes()
        for changed in [original.replace(codec_worker.JXR_BUILD_PREFIX, codec_worker.PRIVATE_JXR_BUILD_PREFIX),
                        original + codec_worker.JXR_BUILD_PREFIX,
                        original.replace(b"JXR_RGB8_LOSSLESS_SEEK_CAP2", b"JXR_RGB8_LOSSLESS_SEEK_CAP0"),
                        original.replace(codec_worker.JXR_BUILD_PREFIX, b"UNKNOWN_WORKER_RECORD\0")]:
            with self.subTest(changed=changed[-30:]):
                helper.write_bytes(changed)
                with self.assertRaises(ValueError):
                    qualification.verify_candidate(cli, target)
        helper.write_bytes(original)
        executable(cli, "x86_64-apple-darwin", with_helper=False)
        with self.assertRaises(ValueError):
            qualification.verify_candidate(cli, target)
        executable(cli, target, with_helper=False)
        installed = qualification.prepare_candidate(cli, target, self.root / "forged")
        metadata = codec_worker.read_manifest(installed)
        manifest = installed / codec_worker.MANIFEST
        for change in [{"schema_version": 2}, {"qualified_export_variants": []},
                       {"qualified_import_variants": list(reversed(codec_worker.JXR_VARIANTS))}]:
            manifest.write_text(json.dumps(metadata | change))
            with self.assertRaises(ValueError):
                codec_worker.verify_installed_manifest(installed, self.version, target)
        manifest.write_text(json.dumps(metadata))
        (installed / cli.name).write_bytes(b"different compiled CLI")
        with self.assertRaisesRegex(ValueError, "differs"):
            qualify_cli_helper.bind_cli(installed / cli.name, cli)

    def test_candidate_contract_failure_prevents_all_test_execution(self):
        target = "aarch64-apple-darwin"
        cli, helper = self.candidate(target, "before-execution")
        cli_test = cli.parent / "installed_codec_worker"
        executable(cli_test, target, with_helper=False)
        worker = {"reason": "compiler-artifact", "target": {"name": "paintcraft-codec-worker"},
                  "profile": {"test": False}, "features": ["jxr-codecs", "jxr-encode", "float-codecs"],
                  "executable": str(helper)}
        tests = [worker | {"target": {"name": name}, "profile": {"test": True}}
                 for name in qualification.REQUIRED_WORKER_TARGETS]
        cli_values = [
            worker | {"target": {"name": "paintcraft-cli", "kind": ["bin"]}, "executable": str(cli)},
            worker | {"target": {"name": "installed_codec_worker"}, "profile": {"test": True}, "executable": str(cli_test)},
        ]
        executions = []
        class FakeRunner:
            def __init__(self, output):
                self.output = output
                output.mkdir(parents=True)
            def run(self, name, argv, **kwargs):
                if str(argv[0]) not in ("cargo", "git"):
                    executions.append(argv)
                    raise AssertionError("Unverified candidate was executed")
                path = self.output / (name + ".stdout")
                value = "commit\n" if name == "source" else ""
                if name == "build-worker":
                    value = "\n".join(map(json.dumps, [worker, *tests]))
                if name == "build-cli":
                    value = "\n".join(map(json.dumps, cli_values))
                path.write_text(value)
                return path
        output = self.root / "gated"
        with patch.object(qualification, "Runner", FakeRunner), \
             patch.object(qualification.host, "identify", return_value={"target": target}), \
             patch.object(qualification, "prepare_candidate", side_effect=ValueError("schema/CLI identity refused")):
            with self.assertRaisesRegex(ValueError, "identity refused"):
                qualification.qualify(target, output)
        self.assertEqual(executions, [])
        receipt = json.loads((output / "receipt.json").read_text())
        self.assertEqual(receipt["status"], "failed")
        self.assertFalse(receipt["native_runtime_qualified"])
        self.assertTrue(receipt["shipping_policy_unchanged"])

    def test_sparse_oversized_or_symlink_binary_refuses_before_read(self):
        path = self.root / "oversized"
        with path.open("wb") as stream:
            stream.truncate(qualification.EXECUTABLE_MAX + 1)
        with self.assertRaisesRegex(ValueError, "bounded regular"):
            qualification.bounded_binary(path, "aarch64-apple-darwin")
        if os.name != "nt":
            alias = self.root / "alias"
            alias.symlink_to(path)
            with self.assertRaises(ValueError):
                qualification.bounded_binary(alias, "aarch64-apple-darwin")

    def test_fixture_emission_and_helpers_are_removed_but_toolchain_remains(self):
        values = {"PAINTCRAFT_DDS_WRITE_VECTORS": "/emit", "PAINTCRAFT_DDS_WRITE_QUALITY": "High",
                  "PAINTCRAFT_JXR_GOLDEN_OUTPUT": "/rewrite", "PAINTCRAFT_JXR_REFERENCE": "/wrong",
                  "PAINTCRAFT_QUALIFIED_INSTALLATION": "/stale", "PATH": "/tools", "RUSTFLAGS": "static"}
        self.assertEqual(qualification.clean_environment(values), {"PATH": "/tools", "RUSTFLAGS": "static"})

    def test_zero_or_missing_execution_summary_cannot_qualify(self):
        path = self.root / "test.stdout"
        runner = Mock()
        runner.run.return_value = path
        for output in ["running 0 tests\ntest result: ok. 0 passed;", "no summary"]:
            path.write_text(output)
            with self.assertRaisesRegex(ValueError, "count"):
                qualification.run_tests(runner, "selected", ["test"], self.root, {})
        path.write_text("test result: ok. 2 passed;")
        with self.assertRaises(ValueError):
            qualification.run_tests(runner, "exact", ["test"], self.root, {}, expected=1)
        self.assertEqual(qualification.run_tests(runner, "group", ["test"], self.root, {}), 2)

    def test_missing_required_test_cannot_be_a_zero_test_success(self):
        runner = Mock()
        with self.assertRaises(ValueError):
            qualification.exact(runner, "missing", Path("binary"), "required", [], self.root, {})
        with self.assertRaises(ValueError):
            qualification.group(runner, "missing", Path("binary"), "required::", [], self.root, {})
        runner.run.assert_not_called()
        self.assertIn("dds_export::tests::", qualification.APP_GROUPS)


class HostTests(unittest.TestCase):
    def test_all_native_host_pairs_accept_and_cross_or_translated_refuse(self):
        for (system, architecture), target in host.TARGETS.items():
            with self.subTest(target=target):
                value = host.validate(target, "host: " + target, system=system, machine=architecture,
                                      native_windows=architecture if system == "Windows" else None)
                self.assertEqual(value["target"], target)
                with self.assertRaises(ValueError):
                    host.validate(target, "host: different", system=system, machine=architecture,
                                  native_windows=architecture if system == "Windows" else None)
                with self.assertRaises(ValueError):
                    host.validate(target, "host: " + target, system=system, machine=architecture, translated=True,
                                  native_windows=architecture if system == "Windows" else None)
        with self.assertRaises(ValueError):
            host.validate("x86_64-pc-windows-msvc", "host: x86_64-pc-windows-msvc",
                          system="Windows", machine="AMD64", native_windows="aarch64")
        # An emulated Python controller does not disguise a natively executing ARM Rust target.
        self.assertEqual(host.validate("aarch64-pc-windows-msvc", "host: aarch64-pc-windows-msvc",
                                      system="Windows", machine="AMD64", native_windows="aarch64")["native_machine"], "aarch64")


class ProcessTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="paintcraft-bounded-qualifier-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()

    def run_code(self, runner, name, code, timeout=5):
        with contextlib.redirect_stdout(io.StringIO()):
            return runner.run(name, [sys.executable, "-c", code], cwd=self.root, env=os.environ.copy(), timeout=timeout)

    def test_success_failure_timeout_and_log_limits_keep_raw_receipts(self):
        runner = Runner(self.root / "logs", limit=128, total_limit=256)
        self.assertIn("success", self.run_code(runner, "ok", "print('success')").read_text())
        with self.assertRaises(RuntimeError):
            self.run_code(runner, "failed", "print('real failure');raise SystemExit(7)")
        self.assertEqual(runner.steps[-1]["exit"], 7)
        with self.assertRaises(RuntimeError):
            self.run_code(runner, "timeout", "import time;time.sleep(30)", timeout=0.1)
        self.assertTrue(runner.steps[-1]["timeout"])
        with self.assertRaises(RuntimeError):
            self.run_code(runner, "bounded", "print('x'*4096)")
        self.assertTrue(runner.steps[-1]["output_limit"])
        self.assertLessEqual(sum(path.stat().st_size for path in runner.output.glob("*.stdout")), 256)
        self.assertTrue((runner.output / "processes.json").is_file())
        self.assertIn("real failure", (runner.output / "01-failed.stdout").read_text())

    @unittest.skipIf(os.name == "nt", "POSIX process group; Windows taskkill tested separately")
    def test_timeout_retires_descendant_pipe_not_only_parent(self):
        runner = Runner(self.root / "tree")
        code = "import subprocess,sys,time;subprocess.Popen([sys.executable,'-c','import time;time.sleep(30)']);time.sleep(30)"
        with self.assertRaises(RuntimeError):
            self.run_code(runner, "tree", code, timeout=0.1)
        self.assertEqual(runner.steps[-1]["transport_faults"], [])

    def test_windows_cleanup_requests_entire_tree(self):
        process = Mock(pid=123)
        process.poll.return_value = None
        with patch("native_qualification_process.os.name", "nt"), patch("native_qualification_process.subprocess.run") as run:
            run.return_value.returncode = 0
            self.assertEqual(Runner.kill(process), [])
        self.assertEqual(run.call_args.args[0], ["taskkill", "/PID", "123", "/T", "/F"])
        process.kill.assert_called_once()


class WorkflowTests(unittest.TestCase):
    def test_native_matrix_and_read_only_candidate_route_are_exact(self):
        workflow = (release.ROOT / ".github/workflows/native-codec-qualification.yml").read_text()
        shipping = (release.ROOT / ".github/workflows/release.yml").read_text()
        pairs = re.findall(r'- os: (\S+)\n\s+target: (\S+)', workflow)
        self.assertEqual(pairs, re.findall(r'- os: (\S+)\n\s+target: (\S+)', shipping))
        self.assertIn("contents: read", workflow)
        self.assertIn("persist-credentials: false", workflow)
        self.assertIn("if: always()", workflow)
        self.assertIn("qualify_native_codecs.py --target", workflow)
        self.assertIn("meson==1.12.1 ninja==1.13.2", workflow)
        self.assertIn("target-feature=+crt-static", workflow)
        self.assertNotIn("contents: write", workflow)
        self.assertNotIn("secrets.", workflow)
        self.assertNotIn("pull_request_target", workflow)
        self.assertNotIn("helper_policy.py", workflow)


if __name__ == "__main__":
    unittest.main()
