"""Complete CLI installs into owned temporary prefixes, with no network or profiles."""
from pathlib import Path
import contextlib
import io
import json
import os
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import codec_worker
import install_cli as install
import package_cli
import package_release as release
from test_package_release import executable


class InstallTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="paintcraft-install-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.version = release.workspace_version()
        self.target = install.native_target()

    def archive(self, target=None, stem="fixture"):
        target = target or self.target
        directory = self.root / stem
        directory.mkdir()
        binary = directory / install.executable(target)
        executable(binary, target)
        helper = directory / codec_worker.filename(target)
        helper.write_bytes(helper.read_bytes().replace(codec_worker.BUILD_PREFIX, codec_worker.AVIF_BUILD_PREFIX).replace(codec_worker.BUILD_SUFFIX, codec_worker.AVIF_BUILD_SUFFIX))
        output = directory / "archives"
        archive = package_cli.package(self.version, target, binary, output)
        sums = output / "SHA256SUMS.txt"
        sums.write_bytes(release.checksum_line(archive))
        return archive, sums

    def install(self, archive, sums, prefix=None, target=None):
        return install.install(archive, sums, self.version, target or self.target, prefix or self.root / "owned-prefix")

    def repack(self, archive, change):
        entries = {}
        if archive.suffix == ".zip":
            with zipfile.ZipFile(archive) as stream:
                entries = {entry.filename: stream.read(entry) for entry in stream.infolist()}
        else:
            with tarfile.open(archive) as stream:
                entries = {entry.name: stream.extractfile(entry).read() for entry in stream}
        change(entries)
        if archive.suffix == ".zip":
            with zipfile.ZipFile(archive, "w") as stream:
                for name, data in entries.items():
                    stream.writestr(name, data)
        else:
            with tarfile.open(archive, "w:gz") as stream:
                for name, data in entries.items():
                    entry = tarfile.TarInfo(name)
                    entry.size = len(data)
                    stream.addfile(entry, io.BytesIO(data))
        sums = archive.parent / "SHA256SUMS.txt"
        sums.write_bytes(release.checksum_line(archive))
        return sums

    def test_both_archive_types_and_all_six_architectures_install_complete_payload(self):
        for target in release.TARGETS:
            with self.subTest(target=target):
                archive, sums = self.archive(target, target)
                destination = self.install(archive, sums, self.root / target, target)
                self.assertEqual({p.name for p in destination.iterdir()}, install.payload_names(target))
                codec_worker.verify_installed_manifest(destination, self.version, target)
                self.assertEqual(json.loads((destination / codec_worker.MANIFEST).read_bytes())["profile"], codec_worker.AVIF_PROFILE)
                if os.name != "nt":
                    self.assertEqual((destination / install.executable(target)).stat().st_mode & 0o777, 0o755)

    def test_bad_checksum_and_incomplete_foreign_or_incompatible_payload_preserve_install(self):
        archive, sums = self.archive()
        destination = self.install(archive, sums)
        before = {p.name: p.read_bytes() for p in destination.iterdir()}
        original = archive.read_bytes()
        for change in [lambda entries: entries.pop(codec_worker.AVIF_NOTICE),
                       lambda entries: entries.update({"foreign.txt": b"foreign"}),
                       lambda entries: entries.update({"../escape": b"escape"}),
                       lambda entries: entries.update({install.executable(self.target): b"wrong architecture"}),
                       lambda entries: entries.update({codec_worker.MANIFEST: b"{}"}),
                       lambda entries: entries.update({codec_worker.filename(self.target): b"wrong build"}),
                       lambda entries: entries.update({codec_worker.AVIF_NOTICE: b"wrong license"})]:
            archive.write_bytes(original)
            sums = self.repack(archive, change)
            with self.assertRaises(ValueError):
                self.install(archive, sums)
            self.assertEqual({p.name: p.read_bytes() for p in destination.iterdir()}, before)
        archive.write_bytes(original)
        sums.write_bytes(b"0" * 64 + b"  " + archive.name.encode() + b"\n")
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            self.install(archive, sums)
        self.assertFalse((self.root / "escape").exists())

    def test_foreign_existing_directory_symlinks_and_active_lock_are_preserved(self):
        archive, sums = self.archive()
        prefix = self.root / "foreign-prefix"
        (prefix / "bin").mkdir(parents=True)
        foreign = prefix / "bin/notes.txt"
        foreign.write_bytes(b"keep me")
        with self.assertRaises(ValueError):
            self.install(archive, sums, prefix)
        self.assertEqual(foreign.read_bytes(), b"keep me")
        lock = prefix / ".install-lock"
        lock.mkdir()
        with self.assertRaisesRegex(ValueError, "in progress"):
            self.install(archive, sums, prefix)
        self.assertTrue(lock.exists())
        alias = self.root / "alias"
        try:
            alias.symlink_to(prefix, target_is_directory=True)
        except OSError:
            self.skipTest("symlink privilege unavailable")
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.install(archive, sums, alias)
        self.assertEqual(foreign.read_bytes(), b"keep me")

    def test_upgrade_rollback_and_failed_rollback_retain_original_bytes(self):
        archive, sums = self.archive()
        destination = self.install(archive, sums)
        before = {p.name: p.read_bytes() for p in destination.iterdir()}
        original_replace = os.replace

        def fail_publish(source, target):
            if Path(source).name == "payload":
                raise OSError("simulated publish failure")
            return original_replace(source, target)

        with patch.object(install.os, "replace", side_effect=fail_publish):
            with self.assertRaisesRegex(OSError, "simulated"):
                self.install(archive, sums)
        self.assertEqual({p.name: p.read_bytes() for p in destination.iterdir()}, before)

        def fail_restore(source, target):
            if Path(source).name == "payload" or Path(source).name.endswith("-previous"):
                raise OSError("simulated rename failure")
            return original_replace(source, target)

        with patch.object(install.os, "replace", side_effect=fail_restore):
            with self.assertRaisesRegex(ValueError, "preserved for recovery"):
                self.install(archive, sums)
        backup, = destination.parent.glob(".install-*-previous")
        self.assertEqual({p.name: p.read_bytes() for p in backup.iterdir()}, before)
        self.assertFalse((destination.parent / ".install-lock").exists())

    def test_existing_bin_junction_is_refused_before_any_file_is_replaced(self):
        archive, sums = self.archive()
        destination = self.install(archive, sums)
        before = {p.name: p.read_bytes() for p in destination.iterdir()}
        with patch.object(Path, "is_junction", lambda path: path == destination, create=True):
            with self.assertRaisesRegex(ValueError, "regular directory"):
                install.verify_payload(destination, self.version, self.target)
            with self.assertRaisesRegex(ValueError, "verified installation"):
                self.install(archive, sums)
        self.assertEqual({p.name: p.read_bytes() for p in destination.iterdir()}, before)
        self.assertFalse((destination.parent / ".install-lock").exists())

    @unittest.skipUnless(os.name == "nt", "actual Windows junction")
    def test_actual_windows_bin_junction_preserves_foreign_installation(self):
        archive, sums = self.archive()
        foreign = self.install(archive, sums, self.root / "foreign-installation")
        before = {p.name: p.read_bytes() for p in foreign.iterdir()}
        prefix = self.root / "junction-prefix"
        prefix.mkdir()
        result = subprocess.run(["cmd", "/c", "mklink", "/J", str(prefix / "bin"), str(foreign)], capture_output=True, text=True)
        if result.returncode:
            self.skipTest("junction creation unavailable: " + result.stderr)
        with self.assertRaisesRegex(ValueError, "verified installation"):
            self.install(archive, sums, prefix)
        self.assertTrue((prefix / "bin").is_junction())
        self.assertEqual({p.name: p.read_bytes() for p in foreign.iterdir()}, before)

    def test_successful_publication_reports_retained_backup_cleanup_as_success(self):
        archive, sums = self.archive()
        destination = self.install(archive, sums)
        before = {p.name: p.read_bytes() for p in destination.iterdir()}
        original_rmtree = shutil.rmtree

        def fail_backup_cleanup(path, *args, **kwargs):
            if Path(path).name.endswith("-previous"):
                raise OSError("simulated cleanup failure")
            return original_rmtree(path, *args, **kwargs)

        stdout, stderr = io.StringIO(), io.StringIO()
        with patch.object(install.shutil, "rmtree", side_effect=fail_backup_cleanup):
            with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                install.main(["--archive", str(archive), "--checksums", str(sums),
                              "--version", self.version, "--prefix", str(destination.parent)])
        self.assertIn("Installed PaintCraft CLI", stdout.getvalue())
        self.assertIn("Installation succeeded", stderr.getvalue())
        backup, = destination.parent.glob(".install-*-previous")
        self.assertIn(str(backup), stderr.getvalue())
        self.assertEqual({p.name: p.read_bytes() for p in backup.iterdir()}, before)
        install.verify_payload(destination, self.version, self.target)
        self.assertFalse((destination.parent / ".install-lock").exists())

    def test_duplicate_or_nonregular_archive_entries_are_refused(self):
        archive, sums = self.archive()
        original = archive.read_bytes()
        if archive.suffix == ".zip":
            with zipfile.ZipFile(archive, "w") as stream:
                entry = zipfile.ZipInfo("paintcraft-cli.exe")
                entry.external_attr = (stat.S_IFLNK | 0o777) << 16
                stream.writestr(entry, "../foreign")
        else:
            with tarfile.open(archive, "w:gz") as stream:
                entry = tarfile.TarInfo("paintcraft-cli")
                entry.type = tarfile.SYMTYPE
                entry.linkname = "../foreign"
                stream.addfile(entry)
        sums.write_bytes(release.checksum_line(archive))
        with self.assertRaises(ValueError):
            self.install(archive, sums)
        archive.write_bytes(original)
        sums.write_bytes(release.checksum_line(archive))
        destination = self.install(archive, sums)
        self.assertTrue(destination.is_dir())
        windows = "x86_64-pc-windows-msvc"
        archive, sums = self.archive(windows, "duplicate")
        with self.assertWarns(UserWarning):
            with zipfile.ZipFile(archive, "a") as stream:
                stream.writestr("LICENSE", b"duplicate")
        sums.write_bytes(release.checksum_line(archive))
        with self.assertRaises(ValueError):
            self.install(archive, sums, self.root / "duplicate-prefix", windows)

    def test_latest_fetch_uses_fixed_https_release_and_verified_checksum_paths(self):
        archive, sums = self.archive()
        original = archive.read_bytes()
        metadata = {"tag_name": "v" + self.version, "draft": False, "prerelease": False,
                    "assets": [{"name": archive.name}, {"name": "SHA256SUMS.txt"}]}
        urls = []

        def download(url, destination, limit):
            urls.append(url)
            if url == install.API + "/latest":
                destination.write_text(json.dumps(metadata))
            elif url.endswith("SHA256SUMS.txt"):
                destination.write_bytes(sums.read_bytes())
            else:
                destination.write_bytes(original)

        temporary = self.root / "fetch"
        temporary.mkdir()
        with patch.object(install, "download", side_effect=download):
            incoming, checksums, version = install.fetch(temporary, None, self.target)
        self.assertEqual(version, self.version)
        self.assertEqual(urls, [install.API + "/latest", f"{install.REPOSITORY}/releases/download/v{version}/{archive.name}", f"{install.REPOSITORY}/releases/download/v{version}/SHA256SUMS.txt"])
        self.install(incoming, checksums)

    def test_download_limits_and_https_are_checked_before_publication(self):
        from unittest.mock import MagicMock
        response = MagicMock()
        response.__enter__.return_value = response
        response.url = "https://release-assets.githubusercontent.com/test"
        response.headers = {"Content-Length": "20"}
        response.read.side_effect = [b"12345", b""]
        output = self.root / "download"
        with patch.object(install.urllib.request, "urlopen", return_value=response):
            with self.assertRaisesRegex(ValueError, "allowance"):
                install.download("https://github.com/example", output, 4)
        self.assertFalse(output.exists())
        response.headers = {}
        with patch.object(install.urllib.request, "urlopen", return_value=response):
            with self.assertRaisesRegex(ValueError, "allowance"):
                install.download("https://github.com/example", output, 4)
        self.assertEqual(output.read_bytes(), b"")
        with self.assertRaisesRegex(ValueError, "HTTPS"):
            install.download("http://github.com/example", output, 4)

    def test_checkout_version_check_stays_strict_but_installed_versions_are_independent(self):
        archive, sums = self.archive()
        destination = self.install(archive, sums)
        future = "0.2.0"
        helper = destination / codec_worker.filename(self.target)
        helper.write_bytes(helper.read_bytes().replace(self.version.encode(), future.encode()))
        metadata = codec_worker.installed_metadata(destination, future, self.target)
        (destination / codec_worker.MANIFEST).write_text(json.dumps(metadata))
        codec_worker.verify_installed_manifest(destination, future, self.target)
        with self.assertRaisesRegex(ValueError, "release contract"):
            codec_worker.verify_manifest(destination, future, self.target)

    def test_source_build_packages_both_binaries_with_avif_and_exact_notices(self):
        archive, sums = self.archive()
        binary_dir = archive.parent.parent
        target_directory = self.root / "cargo-target"
        built = target_directory / self.target / "release"
        built.mkdir(parents=True)
        for name in (install.executable(self.target), codec_worker.filename(self.target)):
            shutil.copy2(binary_dir / name, built / name)
        temporary = self.root / "source-output"
        temporary.mkdir()
        metadata = subprocess.CompletedProcess([], 0, stdout=json.dumps({"target_directory": str(target_directory)}))
        with patch.object(install.subprocess, "run", return_value=metadata) as run:
            archive, sums, version = install.source_archive(temporary, self.target)
        command = run.call_args_list[0].args[0]
        self.assertIn("paintcraft-cli", command)
        self.assertIn("paintcraft-codec-worker", command)
        self.assertIn("paintcraft-codec-worker/avif-codec", command)
        self.assertIn("--locked", command)
        self.assertEqual(version, self.version)
        destination = self.install(archive, sums)
        self.assertEqual({p.name for p in destination.iterdir()}, install.payload_names(self.target))

    @unittest.skipUnless(os.name == "nt", "actual PowerShell wrapper")
    def test_real_powershell_wrapper_offline_install_keeps_complete_payload(self):
        archive, sums = self.archive()
        prefix = self.root / "powershell-prefix"
        output = subprocess.run(["pwsh", "-NoProfile", "-File", str(release.ROOT / "scripts/packaging/install_cli.ps1"),
                                 "--prefix", str(prefix), "--archive", str(archive),
                                 "--checksums", str(sums), "--version", self.version],
                                capture_output=True, text=True)
        self.assertEqual(output.returncode, 0, output.stderr)
        self.assertEqual({p.name for p in (prefix / "bin").iterdir()}, install.payload_names(self.target))

    def test_real_shell_wrapper_offline_install_never_edits_profile_or_global_path(self):
        if os.name == "nt":
            self.skipTest("POSIX shell wrapper")
        archive, sums = self.archive()
        prefix = self.root / "wrapper-prefix"
        profile = self.root / ".profile"
        profile.write_text("keep profile\n")
        output = subprocess.run(["sh", str(release.ROOT / "scripts/packaging/install_cli.sh"),
                                 "--prefix", str(prefix), "--archive", str(archive),
                                 "--checksums", str(sums), "--version", self.version],
                                capture_output=True, text=True)
        self.assertEqual(output.returncode, 0, output.stderr)
        self.assertIn(str(prefix / "bin"), output.stdout)
        self.assertEqual(profile.read_text(), "keep profile\n")
        self.assertEqual({p.name for p in (prefix / "bin").iterdir()}, install.payload_names(self.target))


if __name__ == "__main__":
    unittest.main()
