#!/usr/bin/env python3
"""Build DEB, RPM and AppImage packages on each native Linux architecture."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib

import codec_worker
import package_release as release

APP_ID = "io.github.replicraft.PaintCraft"
ARCHITECTURES = ("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu")


def desktop_files(notices=codec_worker.NOTICES):
    return {
        str(release.ROOT / "LICENSE"): "usr/share/licenses/paintcraft/LICENSE",
        **{str(release.ROOT / "packaging" / name): "usr/share/licenses/paintcraft/" + name for name in notices},
        str(release.ROOT / "packaging/FONT_LICENSES.txt"): "usr/share/licenses/paintcraft/FONT_LICENSES.txt",
        str(release.ROOT / "packaging/paintcraft.metainfo.xml"): f"usr/share/metainfo/{APP_ID}.metainfo.xml",
        str(release.ROOT / "packaging/paintcraft-mime.xml"): "usr/share/mime/packages/paintcraft.xml",
    }


def configuration(version, target, binary, output, format_name, helper, worker_manifest):
    config = json.loads((release.ROOT / "packaging/cargo-packager.json").read_text())
    config.update(version=version, targetTriple=target, outDir=str(output.resolve()),
                  binaries=[{"path": str(binary.resolve()), "main": True}, {"path": str(helper), "main": False}], formats=[format_name])
    config["icons"] = [str(release.ROOT / "assets/paintcraft-icon.iconset/icon_256x256.png")]
    files = desktop_files(codec_worker.installed_notices(helper.parent))
    files[str(worker_manifest)] = "usr/share/paintcraft/" + codec_worker.MANIFEST
    config["deb"].update(desktopTemplate=str(release.ROOT / "packaging/paintcraft.desktop"),
                         files=files)
    return config


def native_package(version, target, binary, archive, temporary, format_name):
    output = temporary / "output"
    output.mkdir()
    helpers = temporary / "helper"
    helpers.mkdir()
    helper = codec_worker.copy(binary, helpers, version, target)
    worker_manifest = helpers / codec_worker.MANIFEST
    config = configuration(version, target, binary, output, format_name, helper, worker_manifest)
    env = dict(os.environ)
    if format_name == "appimage":
        # Keep the dotted desktop ID intact through cargo-packager's file stem.
        launcher = temporary / f"{APP_ID}.sh"
        shutil.copy2(release.ROOT / "packaging/appimage-launcher.sh", launcher)
        launcher.chmod(0o755)
        config["binaries"] = [{"path": str(launcher), "main": True},
                              {"path": str(binary.resolve()), "main": False}, {"path": str(helper), "main": False}]
        desktop = temporary / f"{APP_ID}.desktop"
        desktop.write_text((release.ROOT / "packaging/paintcraft.desktop").read_text()
                           .replace("Exec=paintcraft", f"Exec={launcher.name}")
                           .replace("Icon=paintcraft", f"Icon={APP_ID}"), encoding="utf-8")
        metadata = temporary / f"{APP_ID}.metainfo.xml"
        metadata.write_text((release.ROOT / "packaging/paintcraft.metainfo.xml").read_text()
                            .replace("paintcraft.desktop", desktop.name), encoding="utf-8")
        files = desktop_files(codec_worker.installed_notices(helper.parent))
        files[str(worker_manifest)] = "usr/share/paintcraft/" + codec_worker.MANIFEST
        files.pop(str(release.ROOT / "packaging/paintcraft.metainfo.xml"))
        files[str(metadata)] = f"usr/share/metainfo/{metadata.name}"
        config["deb"]["desktopTemplate"] = str(desktop)
        config.setdefault("appimage", {})["files"] = files
        env["APPIMAGE_EXTRACT_AND_RUN"] = "1"
    subprocess.run(["cargo", "packager", "--config", json.dumps(config)],
                   cwd=release.ROOT, env=env, check=True)
    extension = "AppImage" if format_name == "appimage" else "deb"
    artifacts = list(output.glob(f"*.{extension}"))
    if len(artifacts) != 1:
        raise ValueError(f"Expected exactly one {extension}, found {len(artifacts)}")
    shutil.copy2(artifacts[0], archive)
    if format_name == "appimage":
        archive.chmod(0o755)


def rpm(version, target, binary, archive, temporary):
    manifest = tomllib.loads((release.ROOT / "apps/paintcraft/Cargo.toml").read_text())
    assets = manifest["package"]["metadata"]["generate-rpm"]["assets"]
    assets[0]["source"] = str(binary.resolve())
    helper = codec_worker.copy(binary, temporary, version, target)
    assets.extend([
        *[{"source": str(release.ROOT / "packaging" / name), "dest": "/usr/share/licenses/paintcraft/" + name, "mode": "644"} for name in codec_worker.installed_notices(helper.parent)],
        {"source": str(helper), "dest": "/usr/bin/paintcraft-codec-worker", "mode": "755"},
        {"source": str(temporary / codec_worker.MANIFEST), "dest": "/usr/share/paintcraft/" + codec_worker.MANIFEST, "mode": "644"},
    ])
    entries = ["{ " + ", ".join(f"{key} = {json.dumps(value)}" for key, value in entry.items()) + " }"
               for entry in assets]
    subprocess.run(["cargo", "generate-rpm", "-p", "apps/paintcraft", "--target", target,
                    "--output", str(archive), "--set-metadata", "assets = [" + ", ".join(entries) + "]"],
                   cwd=release.ROOT, check=True)


def package(version, target, binary, output, formats):
    codec_worker.prepare(binary, version, target)
    if target not in ARCHITECTURES:
        raise ValueError(f"Unsupported Linux target: {target}")
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    for format_name in formats:
        if format_name not in ("deb", "rpm", "appimage"):
            raise ValueError(f"Unsupported Linux format: {format_name}")
        extension = "AppImage" if format_name == "appimage" else format_name
        archive = output / f"paintcraft-{version}-{release.TARGETS[target]}.{extension}"
        with tempfile.TemporaryDirectory(prefix="paintcraft-linux-") as directory:
            if format_name == "rpm":
                rpm(version, target, binary, archive, Path(directory))
            else:
                native_package(version, target, binary, archive, Path(directory), format_name)
        release.write_checksum(archive)
        print(archive)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", choices=ARCHITECTURES, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--formats", nargs="+", choices=("deb", "rpm", "appimage"), default=["deb", "rpm", "appimage"])
    args = parser.parse_args()
    try:
        package(args.version, args.target, args.binary, args.output, args.formats)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Linux packaging failed: {error}\n")


if __name__ == "__main__":
    main()
