"""Release signing contracts; real encrypted-key test uses only a throwaway key."""
import hashlib
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import sign_release as signing
import package_release as release


class SigningTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="paintcraft-sign-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.assets = self.root / "assets"
        self.assets.mkdir()
        self.version = release.workspace_version()
        self.names = {"fixture-payload.tar.gz"}
        self.patch = patch.object(release, "expected_assets", return_value=self.names)
        self.patch.start()
        self.addCleanup(self.patch.stop)
        self.payload = self.assets / next(iter(self.names))
        self.payload.write_bytes(b"unchanged throwaway release bytes")
        (self.assets / "SHA256SUMS.txt").write_bytes(release.checksum_line(self.payload))
        self.public = self.root / "test-only.pub"
        self.public.write_text("throwaway verifier fixture\n")

    def signature(self, path):
        path.write_text("untrusted comment: fixture\nAA==\ntrusted comment: " +
                        signing.trusted_comment(self.version, path.name.removesuffix(signing.SUFFIX)) + "\nAA==\n")

    def test_complete_finalized_set_rejects_extra_missing_symlink_and_changed_checksum(self):
        self.assertEqual(len(signing.finalized(self.version, self.assets, False)), 2)
        extra = self.assets / "unexpected"
        extra.write_bytes(b"x")
        with self.assertRaises(ValueError): signing.finalized(self.version, self.assets, False)
        extra.unlink()
        data = self.payload.read_bytes()
        self.payload.unlink()
        other = self.root / "source"
        other.write_bytes(data)
        self.payload.symlink_to(other)
        with self.assertRaises(ValueError): signing.finalized(self.version, self.assets, False)
        self.payload.unlink()
        self.payload.write_bytes(data + b"changed")
        with self.assertRaises(ValueError): signing.finalized(self.version, self.assets, False)
        with self.assertRaises(ValueError): signing.finalized(self.version, self.assets, True)

    def test_secret_permissions_stdin_child_env_cleanup_and_no_partial_on_failure(self):
        key_paths = []
        environment = {"UPDATE_SIGNING_KEY": "DO_NOT_PRINT_SECRET", "UPDATE_SIGNING_PASSWORD": "DO_NOT_PRINT_PASSWORD"}
        def run(argv, **kwargs):
            self.assertNotIn(environment["UPDATE_SIGNING_KEY"], " ".join(argv))
            self.assertNotIn(environment["UPDATE_SIGNING_PASSWORD"], " ".join(argv))
            self.assertNotIn("UPDATE_SIGNING_KEY", kwargs["env"])
            self.assertNotIn("UPDATE_SIGNING_PASSWORD", kwargs["env"])
            self.assertIs(kwargs["stdout"], subprocess.DEVNULL)
            self.assertIs(kwargs["stderr"], subprocess.DEVNULL)
            if "-S" in argv:
                key = Path(argv[argv.index("-s") + 1]); key_paths.append(key)
                if os.name != "nt":
                    self.assertEqual(stat.S_IMODE(key.stat().st_mode), 0o600)
                    self.assertEqual(stat.S_IMODE(key.parent.stat().st_mode), 0o700)
                self.assertEqual(key.read_text(), environment["UPDATE_SIGNING_KEY"])
                self.assertEqual(kwargs["input"], b"DO_NOT_PRINT_PASSWORD\n")
                self.signature(Path(argv[argv.index("-x") + 1]))
                return subprocess.CompletedProcess(argv, 0)
            return subprocess.CompletedProcess(argv, 1)
        with patch.object(signing.subprocess, "run", side_effect=run), self.assertRaises(ValueError) as refusal:
            signing.sign(self.version, self.assets, self.public, environment=environment)
        self.assertNotIn("DO_NOT_PRINT", str(refusal.exception))
        self.assertTrue(key_paths)
        self.assertTrue(all(not p.exists() for p in key_paths))
        self.assertFalse(list(self.assets.glob("*.minisig")))

    def test_trusted_comment_must_bind_exact_version_filename_before_crypto(self):
        sig = self.root / "bad.minisig"
        sig.write_text("untrusted comment: fixture\nAA==\ntrusted comment: wrong\nAA==\n")
        with patch.object(signing.subprocess, "run") as command, self.assertRaises(ValueError):
            signing.verify_one(self.version, self.payload, sig, self.public, "minisign")
        command.assert_not_called()

    def test_verify_only_requires_no_key_and_checks_every_file_with_public_key(self):
        names = signing.finalized(self.version, self.assets, False)
        for name in names: self.signature(self.assets / (name + signing.SUFFIX))
        with patch.dict(os.environ, {}, clear=True), patch.object(signing, "command") as command:
            self.assertEqual(signing.verify(self.version, self.assets, self.public), 2)
        self.assertEqual(command.call_count, 2)
        self.assertTrue(all("-V" in call.args[0] for call in command.call_args_list))

    def test_actual_production_set_has_27_signed_files_including_checksum_manifest(self):
        self.patch.stop()
        try:
            names = release.expected_assets(self.version)
            self.assertEqual(len(names), 26)
            production = self.root / "production"
            production.mkdir()
            for name in names: (production / name).write_bytes(b"fixture " + name.encode())
            (production / "SHA256SUMS.txt").write_bytes(b"".join(
                release.checksum_line(production / name) for name in sorted(names)))
            files = signing.finalized(self.version, production, False)
            self.assertEqual(len(files), 27)
            self.assertIn("SHA256SUMS.txt", files)
            self.assertEqual(sum(name.endswith(".flatpak") for name in files), 2)
        finally:
            self.patch.start()

    @unittest.skipUnless(shutil.which("minisign"), "minisign CLI not installed")
    def test_real_encrypted_throwaway_key_sign_verify_tamper_and_wrong_public_key(self):
        executable = shutil.which("minisign")
        secret = self.root / "test-only.key"
        self.public.unlink()
        # No real user/release key is opened. All generated material dies with this directory.
        generated = subprocess.run([executable, "-G", "-p", str(self.public), "-s", str(secret)],
                                   input=b"THROWAWAY-test-password\nTHROWAWAY-test-password\n",
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False, timeout=120)
        self.assertEqual(generated.returncode, 0)
        before = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in self.assets.iterdir()}
        with patch.dict(os.environ, {}, clear=True), self.assertRaises(ValueError):
            signing.sign(self.version, self.assets, self.public, executable,
                {"UPDATE_SIGNING_KEY": secret.read_text(), "UPDATE_SIGNING_PASSWORD": "WRONG-throwaway-password"})
        self.assertFalse(list(self.assets.glob("*.minisig")))
        with patch.dict(os.environ, {}, clear=True):
            self.assertEqual(signing.sign(self.version, self.assets, self.public, executable,
                {"UPDATE_SIGNING_KEY": secret.read_text(), "UPDATE_SIGNING_PASSWORD": "THROWAWAY-test-password"}), 2)
            self.assertEqual(signing.verify(self.version, self.assets, self.public, executable), 2)
        for name, digest in before.items(): self.assertEqual(hashlib.sha256((self.assets / name).read_bytes()).hexdigest(), digest)
        signature = self.assets / (self.payload.name + signing.SUFFIX)
        original_signature = signature.read_bytes()
        lines = original_signature.splitlines()
        # Alter real signed bytes while keeping all four signature lines and the binding intact.
        damaged = bytearray(lines[1])
        damaged[20] = ord("A") if damaged[20] != ord("A") else ord("B")
        signature.write_bytes(b"\n".join([lines[0], bytes(damaged), lines[2], lines[3]]) + b"\n")
        with self.assertRaises(ValueError): signing.verify(self.version, self.assets, self.public, executable)
        signature.write_bytes(original_signature)
        signature.write_bytes(original_signature.replace(b"trusted comment: paintcraft-update ",
                                                        b"trusted comment: altered-update "))
        with self.assertRaises(ValueError): signing.verify(self.version, self.assets, self.public, executable)
        signature.write_bytes(original_signature)
        self.assertEqual(signing.verify(self.version, self.assets, self.public, executable), 2)
        wrong = self.root / "wrong.pub"
        generated = subprocess.run([executable, "-G", "-W", "-p", str(wrong), "-s", str(self.root / "wrong.key")],
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False, timeout=120)
        self.assertEqual(generated.returncode, 0)
        with self.assertRaises(ValueError): signing.verify(self.version, self.assets, wrong, executable)
        self.payload.write_bytes(b"tampered")
        with self.assertRaises(ValueError): signing.verify(self.version, self.assets, self.public, executable)


if __name__ == "__main__":
    unittest.main()
