"""Finite native candidate qualification, independent of shipping holdbacks."""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import subprocess
import sys

import codec_worker
import helper_policy
import install_cli
import native_qualification_host as host
from native_qualification_process import Runner
import package_cli
import package_release as release
import qualify_cli_helper as cli_tests

TARGETS = tuple(release.TARGETS)
EXECUTABLE_MAX = 256 << 20
APP_GROUPS = ("importing::dds::tests::", "jxr_export::tests::", "dds_export::tests::")
REQUIRED_WORKER_TARGETS = {"paintcraft-codec-worker", "native_jxl", "native_jxr", "native_heif",
                           "native_heif_color", "native_avif", "native_avif_sdr", "native_float",
                           "native_jxr_encode", "native_jxr_writer", "native_jxl_integer"}
APP_LEGACY = "importing::native_codec::jxr_installed_tests::fixture_real_installed_jxr_review"
CANDIDATES = {
    "v8": ("jxr-codecs", codec_worker.JXR_PROFILE, 3),
    "v9": ("jxl-integer-codec", codec_worker.JXL_INTEGER_PROFILE, 2),
    "v10": ("jxr-codecs,jxl-integer-codec", codec_worker.JXL_INTEGER_JXR_PROFILE, 3),
}
APP_INTEGER = "importing::native_codec::jxl_integer_installed_tests::copied_jxl_integer_helper_runs_actual_app_review_and_failure_guards"
INTEGER_WORKER_TESTS = (
    "actual_integer_family_uses_one_request_and_exact_report_for_both_source_depths",
    "old_exact8_request_never_adopts16_and_new_family_refuses_other_profiles",
)


def candidate_contract(candidate):
    if candidate not in CANDIDATES:
        raise ValueError("Unknown finite native candidate")
    return CANDIDATES[candidate]


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def clean_environment(inherited):
    # No inherited golden-emission mode, helper path or oracle can change scope.
    return {key: value for key, value in inherited.items() if not key.startswith("PAINTCRAFT_")}


def bounded_binary(path, target):
    if path.is_symlink() or not path.is_file() or not 0 < path.stat().st_size <= EXECUTABLE_MAX:
        raise ValueError("Candidate executable must be a bounded regular file")
    release.validate_binary(target, path)


def verify_candidate(cli, target, candidate="v8"):
    """Deliberately does NOT use helper_policy.verify (five targets hold V7)."""
    bounded_binary(cli, target)
    helper = cli.parent / codec_worker.filename(target)
    bounded_binary(helper, target)
    version = release.workspace_version()
    profile, _ = codec_worker.verify_build_record(helper, version)
    if profile != candidate_contract(candidate)[1]:
        raise ValueError("Qualification requires the exact selected nonprivate candidate")
    return helper


def prepare_candidate(cli, target, output, candidate="v8"):
    verify_candidate(cli, target, candidate)
    version = release.workspace_version()
    archive = package_cli.package(version, target, cli, output / "archives")
    sums = archive.parent / "SHA256SUMS.txt"
    sums.write_bytes(release.checksum_line(archive))
    installed = install_cli.install(archive, sums, version, target, output / "prefix")
    codec_worker.verify_installed_manifest(installed, version, target)
    metadata = codec_worker.read_manifest(installed)
    _, profile, schema = candidate_contract(candidate)
    if metadata["profile"] != profile or metadata["schema_version"] != schema:
        raise ValueError("Candidate installation lost its exact public contract")
    cli_tests.bind_cli(installed / cli.name, cli)
    return installed


def artifacts(path, *, name=None):
    values = []
    for line in path.read_text(encoding="utf-8").splitlines():
        value = json.loads(line)
        if value.get("reason") == "compiler-artifact" and value.get("executable"):
            if name is None or value["target"]["name"] == name:
                values.append(value)
    return values


def one_test(path, name):
    values = {Path(value["executable"]) for value in artifacts(path, name=name)
              if value.get("profile", {}).get("test")}
    if len(values) != 1:
        raise ValueError(f"Expected exactly one compiled test executable: {name}")
    return values.pop()


def listing(runner, name, binary, cwd, env):
    path = runner.run(name, [binary, "--list"], cwd=cwd, env=env, timeout=60)
    return [line.removesuffix(": test") for line in path.read_text().splitlines() if line.endswith(": test")]


def run_tests(runner, label, argv, cwd, env, *, expected=None):
    output = runner.run(label, [*argv, "--color", "never"], cwd=cwd, env=env, timeout=900)
    counts = re.findall(r"test result: ok\. (\d+) passed;", output.read_text(encoding="utf-8"))
    if not counts or int(counts[-1]) == 0 or (expected is not None and int(counts[-1]) != expected):
        raise ValueError(f"Required executed-test count missing: {label}")
    return int(counts[-1])


def exact(runner, label, binary, name, names, cwd, env, *, ignored=False):
    if names.count(name) != 1:
        raise ValueError(f"Missing or ambiguous required test: {name}")
    args = [binary, name, "--exact", "--test-threads=1", "--nocapture"]
    if ignored:
        args.append("--ignored")
    run_tests(runner, label, args, cwd, env, expected=1)


def group(runner, label, binary, prefix, names, cwd, env):
    selected = [name for name in names if name.startswith(prefix)]
    if not selected:
        raise ValueError(f"Required native test group missing: {prefix}")
    run_tests(runner, label, [binary, prefix, "--test-threads=1", "--nocapture"], cwd, env)
    return selected


def qualify(target, output, oracle=None, target_directory=None, candidate="v8"):
    feature, profile, schema = candidate_contract(candidate)
    if oracle and candidate != "v8":
        raise ValueError("The separate original-C fixture currently requires public V8")
    output.mkdir(parents=True, exist_ok=False)
    output = output.resolve(strict=True)
    runner = Runner(output / "logs")
    env = clean_environment(os.environ)
    target_directory = (target_directory or release.ROOT / "target").resolve()
    env["CARGO_TARGET_DIR"] = str(target_directory)
    policy = dict(helper_policy.ROLLOUT)
    receipt = {"schema": 1, "target": target, "status": "started", "native_runtime_qualified": False,
               "scope": "Candidate only; shipping map and global Cargo default are unchanged",
               "target_directory": str(target_directory), "selected_candidate": candidate,
               "oracle_executed": False, "phases": {}, "flags": {
                   key: env.get(key) for key in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "MACOSX_DEPLOYMENT_TARGET")}}
    save = lambda: (output / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    save()
    def remember(path):
        bounded_binary(path, target)
        receipt.setdefault("executables", {})[str(path)] = {
            "sha256": digest(path), "bytes": path.stat().st_size, "target": target}
        save()
    try:
        commit = runner.run("source", ["git", "rev-parse", "HEAD"], cwd=release.ROOT, env=env, timeout=30).read_text().strip()
        status = runner.run("source-status", ["git", "status", "--porcelain"], cwd=release.ROOT, env=env, timeout=30).read_text()
        receipt.update(source_commit=commit, source_dirty=status, lock_sha256=digest(release.ROOT / "Cargo.lock"),
                       toolchain_file_sha256=digest(release.ROOT / "rust-toolchain.toml"))
        if status:
            raise ValueError("Native qualification requires a clean committed source checkout")
        receipt["native"] = host.identify(target, runner, release.ROOT, env)
        runner.run("cargo-version", ["cargo", "--version"], cwd=release.ROOT, env=env, timeout=30)
        if oracle:
            bounded_binary(oracle, target)
            if digest(oracle) != "8e3b71db2c53c5db5f988b8755946a30ac1070b7a6c0a8ffa6933d34d26b95ce":
                raise ValueError("Explicit original-C executable does not match pinned qualification")
        base = ["cargo", "test", "--locked", "--release", "--target", target]
        worker_json = runner.run("build-worker", base + ["-p", "paintcraft-codec-worker", "--no-default-features",
                                 "--features", feature, "--tests", "--no-run", "--message-format=json"],
                                 cwd=release.ROOT, env=env, timeout=3600)
        worker_values = artifacts(worker_json)
        test_targets = {value["target"]["name"] for value in worker_values if value["profile"]["test"]}
        if not REQUIRED_WORKER_TARGETS.issubset(test_targets):
            raise ValueError("Required worker unit/integration targets are missing")
        workers = {Path(value["executable"]) for value in worker_values
                   if value["target"]["name"] == "paintcraft-codec-worker" and not value["profile"]["test"]}
        if len(workers) != 1:
            raise ValueError("Cargo did not identify exactly one candidate helper")
        helper = workers.pop()
        selected_worker = next(value for value in worker_values if value["executable"] == str(helper) and not value["profile"]["test"])
        required = {"float-codecs"}
        if schema == 3:
            required |= {"jxr-codecs", "jxr-encode"}
        if candidate in ("v9", "v10"):
            required.add("jxl-integer-codec")
        if not required.issubset(selected_worker.get("features", [])):
            raise ValueError("Candidate record requires all declared worker features")
        receipt["worker_features"] = selected_worker["features"]
        remember(helper)
        actual_profile, _ = codec_worker.verify_build_record(helper, release.workspace_version())
        if actual_profile != profile:
            raise ValueError("Built worker is not the exact qualified public candidate")
        receipt["helper"] = {"path": str(helper), "sha256": digest(helper), "profile": profile}
        save()
        cli_json = runner.run("build-cli", cli_tests.build_command(target), cwd=release.ROOT, env=env, timeout=3600)
        cli_test, cli = cli_tests.built_artifacts(cli_json.read_text())
        remember(cli_test)
        remember(cli)
        if cli.parent != helper.parent:
            raise ValueError("CLI/helper artifacts are not in the same fixed target build")
        installed = prepare_candidate(cli, target, output / "candidate", candidate)
        receipt["candidate"] = {"installation": str(installed), "cli_sha256": digest(cli),
                                "cli_test_sha256": digest(cli_test),
                                "manifest": codec_worker.read_manifest(installed),
                                "files": {p.name: digest(p) for p in installed.iterdir()}}
        save()
        fixture_env = dict(env, PAINTCRAFT_QUALIFIED_HELPER=str(installed / helper.name),
                           PAINTCRAFT_QUALIFIED_INSTALLATION=str(installed), PAINTCRAFT_JXR_INSTALLATION=str(installed))
        if candidate in ("v9", "v10"):
            fixture_env["PAINTCRAFT_JXL_INTEGER_HELPER"] = str(installed / helper.name)
        if oracle:
            fixture_env["PAINTCRAFT_JXR_REFERENCE"] = str(oracle)
        for index, value in enumerate(worker_values):
            if value["profile"]["test"]:
                test = Path(value["executable"])
                remember(test)
                names = listing(runner, f"worker-list-{index}", test, release.ROOT, env)
                if not names:
                    raise ValueError("Empty worker qualification target")
                run_tests(runner, f"worker-{index}", [test, "--test-threads=1", "--nocapture"], release.ROOT, env)
                if value["target"]["name"] == "native_jxl_integer":
                    for number, name in enumerate(INTEGER_WORKER_TESTS):
                        exact(runner, f"integer-worker-{number}", test, name, names, release.ROOT, env)
        cli_names = listing(runner, "cli-list", cli_test, release.ROOT, fixture_env)
        run_tests(runner, "cli-ordinary", [cli_test, "--test-threads=1"], release.ROOT, fixture_env)
        for index, name in enumerate(cli_tests.selectors(candidate, bool(oracle))):
            exact(runner, f"cli-{index}", cli_test, name, cli_names, release.ROOT, fixture_env, ignored=True)
        receipt["oracle_executed"] = bool(oracle)
        dds_cli_json = runner.run("build-dds-cli", base + ["-p", "paintcraft-cli", "--test", "dds_texture_export",
                                  "--no-run", "--message-format=json"], cwd=release.ROOT, env=env, timeout=3600)
        dds_cli = one_test(dds_cli_json, "dds_texture_export")
        remember(dds_cli)
        # A different integration target must not silently bind another CLI build.
        extra_cli = {Path(value["executable"]) for value in artifacts(dds_cli_json, name="paintcraft-cli")
                     if not value["profile"]["test"]}
        if len(extra_cli) != 1:
            raise ValueError("DDS integration did not identify one exact CLI")
        cli_tests.bind_cli(installed / cli.name, extra_cli.pop())
        if not listing(runner, "dds-cli-list", dds_cli, release.ROOT, env):
            raise ValueError("DDS CLI/MCP integration target is empty")
        run_tests(runner, "dds-cli-mcp", [dds_cli, "--test-threads=1", "--nocapture"], release.ROOT, env)
        core_json = runner.run("build-core", base + ["-p", "paintcraft-formats", "-p", "paintcraft-platform",
                               "--lib", "--no-run", "--message-format=json"], cwd=release.ROOT, env=env, timeout=3600)
        formats = one_test(core_json, "paintcraft_formats")
        platform_test = one_test(core_json, "paintcraft_platform")
        for test in (formats, platform_test):
            remember(test)
        fnames = listing(runner, "formats-list", formats, release.ROOT, env)
        pnames = listing(runner, "platform-list", platform_test, release.ROOT, env)
        for index, prefix in enumerate(("dds::", "export::dds::", "export::jxr::",
                                        "codec_worker::native::jxr_encode::tests::",
                                        "codec_worker::native::import::family_tests::",
                                        "codec_worker::jxl_integer::",
                                        "codec_worker::native::jxl_integer::tests::",
                                        "codec_worker::native::import::jxl_integer::tests::")):
            receipt["phases"][prefix] = group(runner, f"core-{index}", formats, prefix, fnames, release.ROOT, env)
        receipt["phases"]["codec_helper"] = group(runner, "platform-helper", platform_test, "codec_helper::", pnames, release.ROOT, env)
        dds_json = runner.run("build-dds-golden", base + ["-p", "paintcraft-formats", "--test", "dds_texture_write",
                              "--no-run", "--message-format=json"], cwd=release.ROOT, env=env, timeout=3600)
        dds = one_test(dds_json, "dds_texture_write")
        remember(dds)
        dnames = listing(runner, "dds-list", dds, release.ROOT, env)
        exact(runner, "dds-96-goldens", dds, "dds_texture_vectors_match_independently_measured_frozen_specimens",
              dnames, release.ROOT, env)
        app_json = runner.run("build-app", base + ["-p", "paintcraft", "--lib", "--no-run", "--message-format=json"],
                              cwd=release.ROOT, env=env, timeout=3600)
        app = one_test(app_json, "paintcraft")
        remember(app)
        anames = listing(runner, "app-list", app, release.ROOT, env)
        for index, prefix in enumerate(APP_GROUPS):
            receipt["phases"][prefix] = group(runner, f"app-{index}", app, prefix, anames, release.ROOT, env)
        copied = output / "app-installation"
        shutil.copytree(installed, copied)
        app_copy = copied / app.name
        shutil.copy2(app, app_copy)
        remember(app_copy)
        exact(runner, "app-legacy-jxr", app_copy, APP_LEGACY, anames, release.ROOT, fixture_env, ignored=True)
        if candidate in ("v9", "v10"):
            exact(runner, "app-integer-jxl", app_copy, APP_INTEGER, anames,
                  release.ROOT, dict(fixture_env, PAINTCRAFT_JXL_INTEGER_GUI_EVIDENCE=str(output / "app-jxl-evidence")),
                  ignored=True)
        if oracle:
            exact(runner, "app-jxr-with-C", app_copy,
                  "jxr_export::installed_tests::copied_public_v8_runs_native_studio_export_and_own_reader",
                  anames, release.ROOT, dict(fixture_env, PAINTCRAFT_JXR_PUBLIC_HELPER=str(copied / helper.name)), ignored=True)
        codec_worker.verify_installed_manifest(installed, release.workspace_version(), target)
        if receipt["candidate"]["files"] != {p.name: digest(p) for p in installed.iterdir()}:
            raise ValueError("Qualified installation changed during tests")
        for path, identity in receipt["executables"].items():
            if digest(path) != identity["sha256"]:
                raise ValueError("Executed artifact changed during qualification")
        if policy != helper_policy.ROLLOUT:
            raise ValueError("Qualification must not alter shipping policy")
        final = runner.run("source-final", ["git", "rev-parse", "HEAD"], cwd=release.ROOT, env=env, timeout=30).read_text().strip()
        final_status = runner.run("source-final-status", ["git", "status", "--porcelain"], cwd=release.ROOT, env=env, timeout=30).read_text()
        if final != commit or final_status:
            raise ValueError("Source changed during qualification")
        receipt.update(status="passed", native_runtime_qualified=True)
    except Exception as error:
        receipt.update(status="failed", error=f"{type(error).__name__}: {error}")
        raise
    finally:
        receipt["process_records"] = "logs/processes.json"
        receipt["shipping_policy_unchanged"] = policy == helper_policy.ROLLOUT
        save()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--candidate", choices=tuple(CANDIDATES), default="v8")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, help="Explicit reusable dependency cache; defaults to checkout target")
    parser.add_argument("--oracle", type=Path, help="Separate pinned native original-C qualification; never implicit")
    args = parser.parse_args()
    qualify(args.target, args.output.absolute(), args.oracle.resolve(strict=True) if args.oracle else None,
            args.target_dir, args.candidate)


if __name__ == "__main__":
    main()
