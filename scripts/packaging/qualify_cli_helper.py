"""Finite installed-CLI qualification; C-oracle cases require explicit opt-in."""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import subprocess
import sys

import codec_worker
import package_release as release

BASELINE = (
    "avif::installed_cli_avif_consent_and_independent_libaom_pixels",
    "avif::installed_mcp_avif_consent_cancel_and_renamed_source_guard",
    "avif_sdr::installed_v6_cli_sdr_profiles_consent_and_exact_fraction_pcnd",
    "avif_sdr::installed_v6_mcp_sdr_consent_cancel_source_owner_and_exact_pcnd",
    "float::actual_cli_and_mcp_float_inspection_import_match_all_independent_cases",
    "float::actual_float_source_guards_cancel_and_rollback_preserve_prior_workspace",
)
JXR = (
    "jxr_export::portable::installed_cli_mcp_match_c_qualified_goldens",
    "jxr_export::installed_forged_export_contract_preserves_cli_destination_and_mcp_session",
)
ORACLES = (
    "jxr_export::installed_cli_fresh_copies_source_guards_and_own_output_read_are_real",
    "jxr_export::installed_mcp_fresh_copy_preserves_dirty_native_binding_and_real_alias_guards",
)
INTEGER_JXL = (
    "jxl_integer::actual_cli_and_mcp_integer_jxl_consent_pixels_receipts_and_guards",
)
PROFILES = {"v7": codec_worker.FLOAT_PROFILE, "v8": codec_worker.JXR_PROFILE,
            "v9": codec_worker.JXL_INTEGER_PROFILE, "v10": codec_worker.JXL_INTEGER_JXR_PROFILE}


def selectors(profile, oracle=False):
    if profile not in PROFILES or (oracle and profile != "v8"):
        raise ValueError("Oracle export qualification requires exact public V8")
    return (BASELINE + (JXR if profile in ("v8", "v10") else ())
            + (INTEGER_JXL if profile in ("v9", "v10") else ()) + (ORACLES if oracle else ()))


def build_command(target):
    return ["cargo", "test", "--locked", "--release", "-p", "paintcraft-cli",
            "--target", target, "--test", "installed_codec_worker", "--no-run",
            "--message-format=json"]


def built_artifacts(stdout):
    tests, clis = set(), set()
    for line in stdout.splitlines():
        item = json.loads(line)
        if item.get("reason") != "compiler-artifact" or not item.get("executable"):
            continue
        target = item.get("target", {})
        if target.get("name") == "installed_codec_worker" and item.get("profile", {}).get("test"):
            tests.add(Path(item["executable"]))
        if target.get("name") == "paintcraft-cli" and "bin" in target.get("kind", []):
            clis.add(Path(item["executable"]))
    if len(tests) != 1 or len(clis) != 1:
        raise ValueError("Cargo must identify exactly one installed test and CLI executable")
    return tests.pop(), clis.pop()


def bind_cli(installed, built):
    if installed.is_symlink() or not installed.is_file():
        raise ValueError("Installed CLI must be a regular file")
    if hashlib.sha256(installed.read_bytes()).digest() != hashlib.sha256(built.read_bytes()).digest():
        raise ValueError("Installed CLI differs from the exact qualification build artifact")


def commands(test_binary, profile, oracle=False):
    base = [str(test_binary)]
    return [base + ["--test-threads=1"]] + [
        base + [name, "--exact", "--ignored", "--test-threads=1"]
        for name in selectors(profile, oracle)
    ]


def require_selected_tests(names, profile, oracle=False):
    for name in selectors(profile, oracle):
        if names.count(name) != 1:
            raise ValueError(f"Missing or ambiguous required CLI test: {name}")


def execute_tests(command, environment):
    result = subprocess.run(command, cwd=release.ROOT, env=environment, check=False,
                            capture_output=True, text=True)
    print(result.stdout, end="")
    print(result.stderr, end="", file=sys.stderr)
    result.check_returncode()
    counts = re.findall(r"test result: ok\. (\d+) passed;", result.stdout)
    if not counts or int(counts[-1]) == 0 or ("--exact" in command and int(counts[-1]) != 1):
        raise ValueError("Required executed CLI test count missing")


def fixture_environment(inherited):
    # A selected helper must never be superseded by an ambient fixture override.
    owned = {f"PAINTCRAFT_{family}_{kind}"
             for family in ("QUALIFIED", "JXR", "JXL_INTEGER", "V4", "V5", "V6", "V7", "V8", "V9", "V10")
             for kind in ("HELPER", "INSTALLATION", "REFERENCE")}
    return {key: value for key, value in inherited.items() if key not in owned}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=tuple(PROFILES), required=True)
    parser.add_argument("--target", choices=release.TARGETS, required=True)
    parser.add_argument("--helper", type=Path)
    parser.add_argument("--installation", type=Path)
    parser.add_argument("--oracle", type=Path)
    args = parser.parse_args()
    environment = fixture_environment(os.environ)
    version = release.workspace_version()
    if args.oracle and args.profile != "v8":
        parser.error("The separate original-C fixture currently requires public V8")
    if args.profile != "v7":
        if args.installation is None or args.helper is not None:
            parser.error("This profile requires --installation, not a helper-only payload")
        directory = args.installation.resolve(strict=True)
        codec_worker.verify_installed_manifest(directory, version, args.target)
        metadata = codec_worker.read_manifest(directory)
        if metadata["profile"] != PROFILES[args.profile]:
            parser.error("Installation differs from the exact selected public profile")
        helper = directory / codec_worker.filename(args.target)
        environment["PAINTCRAFT_QUALIFIED_INSTALLATION"] = str(directory)
        if args.profile in ("v8", "v10"):
            environment["PAINTCRAFT_JXR_INSTALLATION"] = str(directory)
        if args.profile in ("v9", "v10"):
            environment["PAINTCRAFT_JXL_INTEGER_HELPER"] = str(helper)
    else:
        if args.helper is None or args.installation is not None or args.oracle is not None:
            parser.error("V7 requires --helper and does not run JXR export/oracle cases")
        helper = args.helper.resolve(strict=True)
    actual, _ = codec_worker.verify_build_record(helper, version)
    expected = PROFILES[args.profile]
    if actual != expected:
        parser.error("Helper record differs from selected exact qualification profile")
    environment["PAINTCRAFT_QUALIFIED_HELPER"] = str(helper)
    if args.profile == "v7":
        environment["PAINTCRAFT_V7_HELPER"] = str(helper)
    if args.oracle:
        # The Rust fixture independently enforces the pinned oracle SHA.
        environment["PAINTCRAFT_JXR_REFERENCE"] = str(args.oracle.resolve(strict=True))
    result = subprocess.run(build_command(args.target), cwd=release.ROOT, env=environment,
                            check=True, capture_output=True, text=True)
    # Build exactly once. Every selection below uses the frozen executable.
    print(result.stderr, end="")
    test_binary, cli = built_artifacts(result.stdout)
    if args.profile != "v7":
        bind_cli(directory / ("paintcraft-cli.exe" if "windows" in args.target else "paintcraft-cli"), cli)
    print(f"qualification_cli_sha256={hashlib.sha256(cli.read_bytes()).hexdigest()}")
    print(f"qualification_test_sha256={hashlib.sha256(test_binary.read_bytes()).hexdigest()}")
    listing = subprocess.run([test_binary, "--list"], cwd=release.ROOT, env=environment,
                             check=True, capture_output=True, text=True)
    names = [line.removesuffix(": test") for line in listing.stdout.splitlines() if line.endswith(": test")]
    require_selected_tests(names, args.profile, bool(args.oracle))
    for command in commands(test_binary, args.profile, bool(args.oracle)):
        execute_tests(command, environment)


if __name__ == "__main__":
    main()
