#!/usr/bin/env python3
"""Build a local macOS bundle; installation and registration are explicit options."""
import argparse
import json
from pathlib import Path
import plistlib
import platform
import shutil
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
TEMPLATE = ROOT / "apps/paintcraft/resources/macos/Info.plist.in"
ICON = ROOT / "assets/paintcraft-icon.icns"
BUNDLE_ID = "io.github.replicraft.PaintCraft"
LSREGISTER = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"


def metadata(version):
    """Use the same versioned metadata that build.rs embeds into the executable."""
    return plistlib.loads(TEMPLATE.read_text().replace("@VERSION@", version).encode())


def populate_bundle(binary, destination, version, launcher):
    destination = Path(destination)
    contents = destination / "Contents"
    (contents / "MacOS").mkdir(parents=True)
    (contents / "Resources").mkdir()
    shutil.copy2(launcher, contents / "MacOS/paintcraft-launcher")
    (contents / "MacOS/paintcraft-launcher").chmod(0o755)
    shutil.copy2(ICON, contents / "Resources" / ICON.name)
    info = metadata(version)
    info["CFBundleExecutable"] = "paintcraft-launcher"
    with (contents / "Info.plist").open("wb") as stream:
        plistlib.dump(info, stream, sort_keys=False)
    (contents / "PkgInfo").write_bytes(b"APPL????")
    # Give the Rust NSApplication a separate identity so future Finder events
    # remain addressed to the outer launcher rather than a child editor window.
    editor = contents / "Helpers/PaintCraft Editor.app/Contents"
    executable = editor / "MacOS/paintcraft"
    executable.parent.mkdir(parents=True)
    (editor / "Resources").mkdir()
    shutil.copy2(binary, executable)
    executable.chmod(executable.stat().st_mode | 0o111)
    shutil.copy2(ICON, editor / "Resources" / ICON.name)
    editor_info = metadata(version)
    editor_info["CFBundleIdentifier"] = BUNDLE_ID + ".editor"
    editor_info.pop("CFBundleDocumentTypes")
    editor_info.pop("UTExportedTypeDeclarations")
    with (editor / "Info.plist").open("wb") as stream:
        plistlib.dump(editor_info, stream, sort_keys=False)
    (editor / "PkgInfo").write_bytes(b"APPL????")


def check_destination(destination, replace):
    if destination.is_symlink():
        raise ValueError(f"Refusing symlink destination: {destination}")
    if not destination.exists():
        return
    if not replace:
        raise ValueError(f"Already exists: {destination}; use --replace to update PaintCraft")
    try:
        with (destination / "Contents/Info.plist").open("rb") as stream:
            identifier = plistlib.load(stream)["CFBundleIdentifier"]
    except (OSError, KeyError, ValueError, plistlib.InvalidFileException) as error:
        raise ValueError(f"Refusing to replace an unrecognized bundle: {destination}") from error
    if identifier != BUNDLE_ID:
        raise ValueError(f"Refusing to replace another application's bundle: {destination}")


def publish_bundle(destination, prepare, replace=False):
    """Prepare privately, then replace only a recognized PaintCraft bundle."""
    destination = Path(destination).absolute()
    if destination.suffix != ".app":
        raise ValueError("Bundle destination must end in .app")
    check_destination(destination, replace)
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".paintcraft-package-", dir=destination.parent) as temporary:
        stage = Path(temporary) / "PaintCraft.app"
        prepare(stage)
        # Check again after a potentially long build/copy/sign step.
        check_destination(destination, replace)
        previous = Path(temporary) / "previous.app"
        had_previous = destination.exists()
        if had_previous:
            destination.rename(previous)
        try:
            stage.rename(destination)
        except OSError:
            if had_previous:
                previous.rename(destination)
            raise
    return destination


def built_binary(debug):
    command = ["cargo", "build", "-p", "paintcraft", "--message-format=json-render-diagnostics"]
    if not debug:
        command.append("--release")
    process = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, check=False)
    # Cargo diagnostics remain visible; structured stdout is used only for the path.
    sys.stderr.write(process.stderr)
    if process.returncode:
        raise subprocess.CalledProcessError(process.returncode, command)
    executable = None
    for line in process.stdout.splitlines():
        item = json.loads(line)
        if item.get("reason") == "compiler-artifact" and item.get("target", {}).get("name") == "paintcraft":
            executable = item.get("executable") or executable
    if executable is None:
        raise ValueError("Cargo did not report a PaintCraft executable")
    return Path(executable)


def build_launcher(directory):
    arch = {"arm64": "arm64", "x86_64": "x86_64"}.get(platform.machine())
    if arch is None:
        raise ValueError("Unsupported native macOS launcher architecture")
    launcher = Path(directory) / "paintcraft-launcher"
    subprocess.run(["/usr/bin/xcrun", "swiftc", "-O", "-target", f"{arch}-apple-macosx13.0",
                    "-module-cache-path", str(Path(directory) / "swift-module-cache"),
                    "-framework", "AppKit", str(TEMPLATE.with_name("Launcher.swift")),
                    "-o", str(launcher)], check=True)
    return launcher


def register(bundle):
    # Registration publishes declarations; no default-handler preference is written.
    subprocess.run([LSREGISTER, "-f", str(bundle)], check=True)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--debug", action="store_true", help="build the debug profile instead of release")
    parser.add_argument("--binary", type=Path, help="package an existing native macOS executable instead of building")
    parser.add_argument("--output", type=Path, default=ROOT / "target/macos/PaintCraft.app")
    parser.add_argument("--replace", action="store_true", help="update an existing recognized PaintCraft bundle")
    parser.add_argument("--install", action="store_true", help="also copy to ~/Applications/PaintCraft.app")
    parser.add_argument("--register", action="store_true", help="register the resulting bundle with Launch Services")
    args = parser.parse_args(argv)
    if sys.platform != "darwin":
        parser.error("macOS packaging must run on macOS")
    try:
        check_destination(args.output.absolute(), args.replace)
        install = Path.home() / "Applications/PaintCraft.app"
        if args.install:
            check_destination(install, args.replace)
        version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
        binary = args.binary.absolute() if args.binary else built_binary(args.debug)
        if not binary.is_file():
            raise ValueError(f"Executable does not exist: {binary}")
        with tempfile.TemporaryDirectory(prefix="paintcraft-launcher-") as directory:
            launcher = build_launcher(directory)
            def prepare(stage):
                populate_bundle(binary, stage, version, launcher)
                for app in [stage / "Contents/Helpers/PaintCraft Editor.app", stage]:
                    subprocess.run(["/usr/bin/codesign", "--force", "--sign", "-", "--timestamp=none", str(app)], check=True)
            bundle = publish_bundle(args.output, prepare, args.replace)
        if args.install and install.absolute() != bundle:
            bundle = publish_bundle(install, lambda stage: shutil.copytree(bundle, stage), args.replace)
        if args.register:
            register(bundle)
        print(bundle)
        return 0
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"Packaging failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
