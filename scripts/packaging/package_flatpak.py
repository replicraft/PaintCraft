#!/usr/bin/env python3
"""Bundle the exact Linux release executables for Flatpak; never rebuild or strip them."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

import codec_worker
import package_release as release

APP_ID = "io.github.replicraft.PaintCraft"
RUNTIME = "25.08"
ARCHES = {"x86_64-unknown-linux-gnu": "x86_64", "aarch64-unknown-linux-gnu": "aarch64"}
RUNTIME_REPO = "https://flathub.org/repo/flathub.flatpakrepo"


def manifest():
    return {
        "app-id": APP_ID, "runtime": "org.freedesktop.Platform", "runtime-version": RUNTIME,
        "sdk": "org.freedesktop.Sdk", "command": "paintcraft", "branch": "stable",
        # File selection is provided by rfd's portal backend. Do not grant host/home access.
        # About checks GitHub releases over HTTPS; installation remains package-managed.
        "finish-args": ["--share=network", "--share=ipc", "--socket=wayland", "--socket=fallback-x11", "--device=dri"],
        # The codec helper's embedded contract and manifest cover its exact bytes.
        "build-options": {"strip": False, "no-debuginfo": True},
        "modules": [{"name": "paintcraft-release", "buildsystem": "simple",
                     "build-commands": ["cp -a payload/. /app/"],
                     "sources": [{"type": "dir", "path": "payload", "dest": "payload"}]}],
    }


def stage(version, target, binary, cli, payload):
    if target not in ARCHES:
        raise ValueError("Flatpak requires a supported Linux release target")
    codec_worker.prepare(binary, version, target)
    release.validate_package_input(version, target, cli)
    if binary.resolve() == cli.resolve():
        raise ValueError("Desktop and CLI inputs must be separate executables")
    bins = payload / "bin"
    resources = payload / "share/paintcraft"
    licenses = payload / "share/licenses/paintcraft"
    for directory in (bins, resources, licenses):
        directory.mkdir(parents=True)
    for source, name in ((binary, "paintcraft"), (cli, "paintcraft-cli")):
        shutil.copy2(source, bins / name)
        (bins / name).chmod(0o755)
    helper = codec_worker.copy(binary, bins, version, target, resource_directory=resources)
    for name in ("LICENSE",):
        shutil.copy2(release.ROOT / name, licenses / name)
    shutil.copy2(release.ROOT / "packaging/desktop/licenses/FONT_LICENSES.txt", licenses / "FONT_LICENSES.txt")
    shutil.copy2(release.ROOT / "packaging/ThirdPartyLicenses.txt", licenses / "ThirdPartyLicenses.txt")
    for name in codec_worker.installed_notices(resources):
        shutil.copy2(resources / name, licenses / name)
    desktop = payload / f"share/applications/{APP_ID}.desktop"
    desktop.parent.mkdir(parents=True)
    desktop.write_text((release.ROOT / "packaging/linux/paintcraft.desktop").read_text(encoding="utf-8")
                       .replace("Icon=paintcraft", f"Icon={APP_ID}"), encoding="utf-8")
    metadata = payload / f"share/metainfo/{APP_ID}.metainfo.xml"
    metadata.parent.mkdir(parents=True)
    metadata.write_text((release.ROOT / "packaging/linux/paintcraft.metainfo.xml").read_text(encoding="utf-8")
                        .replace("paintcraft.desktop", APP_ID + ".desktop"), encoding="utf-8")
    mime = payload / f"share/mime/packages/{APP_ID}.xml"
    mime.parent.mkdir(parents=True)
    mime.write_text((release.ROOT / "packaging/linux/paintcraft-mime.xml").read_text(encoding="utf-8")
                    .replace('icon name="paintcraft"', f'icon name="{APP_ID}"'), encoding="utf-8")
    icon = payload / f"share/icons/hicolor/256x256/apps/{APP_ID}.png"
    icon.parent.mkdir(parents=True)
    shutil.copy2(release.ROOT / "assets/paintcraft-icon.iconset/icon_256x256.png", icon)
    return {name: release.digest(bins / name) for name in
            ("paintcraft", "paintcraft-cli", helper.name)}


def verify_payload(build, hashes):
    for name, expected in hashes.items():
        path = build / "files/bin" / name
        if path.is_symlink() or not path.is_file() or release.digest(path) != expected:
            raise ValueError(f"Flatpak changed the exact release executable: {name}")
    resources = build / "files/share/paintcraft"
    contract = codec_worker.parse_manifest((resources / codec_worker.MANIFEST).read_bytes())
    if contract["sha256"] != hashes["paintcraft-codec-worker"]:
        raise ValueError("Flatpak codec helper manifest does not match its installed executable")


def builder_command(root, target, builder):
    if builder == "flathub":
        # Builder has a private /tmp; expose only this invocation's staged payload.
        command = ["flatpak", "run", "--user", "--filesystem=" + str(root), "org.flatpak.Builder"]
    elif builder == "host":
        command = ["flatpak-builder"]
    else:
        raise ValueError("Unknown Flatpak builder")
    return command + ["--user", "--force-clean", "--disable-rofiles-fuse",
                      "--arch=" + ARCHES[target], str(root / "build"),
                      str(root / f"{APP_ID}.json")]


def package(version, target, binary, cli, output, *, builder="host"):
    if target not in ARCHES:
        raise ValueError("Flatpak requires a supported Linux release target")
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    archive = output / f"paintcraft-{version}-{release.TARGETS[target]}.flatpak"
    if archive.exists():
        raise ValueError(f"Refusing to replace existing Flatpak: {archive}")
    with tempfile.TemporaryDirectory(prefix="paintcraft-flatpak-") as directory:
        root = Path(directory)
        hashes = stage(version, target, binary, cli, root / "payload")
        config = root / f"{APP_ID}.json"
        config.write_text(json.dumps(manifest(), indent=4) + "\n", encoding="utf-8")
        build = root / "build"
        subprocess.run(builder_command(root, target, builder), cwd=root, check=True)
        verify_payload(build, hashes)
        repo = root / "repo"
        subprocess.run(["flatpak", "build-export", "--arch=" + ARCHES[target],
                        str(repo), str(build), "stable"], check=True)
        subprocess.run(["flatpak", "build-bundle", "--arch=" + ARCHES[target],
                        "--runtime-repo=" + RUNTIME_REPO, str(repo), str(archive), APP_ID, "stable"], check=True)
    release.write_checksum(archive)
    print(archive)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", choices=ARCHES, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--cli-binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--builder", choices=("host", "flathub"), default="host",
                        help="Use a modern host builder or Flathub's org.flatpak.Builder")
    args = parser.parse_args()
    try:
        package(args.version, args.target, args.binary, args.cli_binary, args.output, builder=args.builder)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Flatpak packaging failed: {error}\n")


if __name__ == "__main__":
    main()
