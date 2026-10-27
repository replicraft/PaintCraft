"""Finite installed-CLI qualification; C-oracle cases require explicit opt-in."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

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
    "jxr_export::installed_forged_export_contract_preserves_cli_destination_and_mcp_session",
)
ORACLES = (
    "jxr_export::installed_cli_fresh_copies_source_guards_and_own_output_read_are_real",
    "jxr_export::installed_mcp_fresh_copy_preserves_dirty_native_binding_and_real_alias_guards",
)


def selectors(profile, oracle=False):
    if profile not in ("v7", "v8") or (oracle and profile != "v8"):
        raise ValueError("Oracle export qualification requires exact public V8")
    return BASELINE + (JXR if profile == "v8" else ()) + (ORACLES if oracle else ())


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


def fixture_environment(inherited):
    # A selected helper must never be superseded by an ambient fixture override.
    owned = {f"PAINTCRAFT_{family}_{kind}"
             for family in ("QUALIFIED", "JXR", "V4", "V5", "V6", "V7", "V8")
             for kind in ("HELPER", "INSTALLATION", "REFERENCE")}
    return {key: value for key, value in inherited.items() if key not in owned}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=("v7", "v8"), required=True)
    parser.add_argument("--target", choices=release.TARGETS, required=True)
    parser.add_argument("--helper", type=Path)
    parser.add_argument("--installation", type=Path)
    parser.add_argument("--oracle", type=Path)
    args = parser.parse_args()
    environment = fixture_environment(os.environ)
    version = release.workspace_version()
    if args.profile == "v8":
        if args.installation is None or args.helper is not None:
            parser.error("V8 requires --installation, not a helper-only payload")
        directory = args.installation.resolve(strict=True)
        codec_worker.verify_installed_manifest(directory, version, args.target)
        metadata = codec_worker.read_manifest(directory)
        if metadata["profile"] != codec_worker.JXR_PROFILE:
            parser.error("V8 requires the exact nonprivate schema3 installation")
        helper = directory / codec_worker.filename(args.target)
        environment["PAINTCRAFT_QUALIFIED_INSTALLATION"] = str(directory)
        environment["PAINTCRAFT_JXR_INSTALLATION"] = str(directory)
    else:
        if args.helper is None or args.installation is not None or args.oracle is not None:
            parser.error("V7 requires --helper and does not run JXR export/oracle cases")
        helper = args.helper.resolve(strict=True)
    actual, _ = codec_worker.verify_build_record(helper, version)
    expected = codec_worker.JXR_PROFILE if args.profile == "v8" else codec_worker.FLOAT_PROFILE
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
    if args.profile == "v8":
        bind_cli(directory / ("paintcraft-cli.exe" if "windows" in args.target else "paintcraft-cli"), cli)
    print(f"qualification_cli_sha256={hashlib.sha256(cli.read_bytes()).hexdigest()}")
    print(f"qualification_test_sha256={hashlib.sha256(test_binary.read_bytes()).hexdigest()}")
    for command in commands(test_binary, args.profile, bool(args.oracle)):
        subprocess.run(command, cwd=release.ROOT, env=environment, check=True)


if __name__ == "__main__":
    main()
