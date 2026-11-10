"""Exercise Git checkout conversion against exact upstream license notices."""

import hashlib
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
NOTICES = {
    "crates/services/paintcraft-formats/tests/jxr-fixtures/JXRLib-LICENSE.md":
        "packaging/desktop/licenses/JPEGXR_LICENSE.txt",
    "apps/paintcraft-codec-worker/vendor/dav1d/COPYING":
        "packaging/desktop/licenses/DAV1D_LICENSE.txt",
    "crates/adapters/paintcraft-update/LICENSE": "packaging/ThirdPartyLicenses.txt",
    "crates/adapters/paintcraft-update/NOTICE": "packaging/ThirdPartyLicenses.txt",
    "assets/fonts/w95fa/OFL.txt": "packaging/ThirdPartyLicenses.txt",
    "assets/fonts/w95fa/readme.txt": "packaging/ThirdPartyLicenses.txt",
}


class LicenseCheckoutTests(unittest.TestCase):
    def test_checkout_preserves_upstream_bytes_and_packaged_notices(self):
        for autocrlf in ("true", "input", "false"):
            with self.subTest(autocrlf=autocrlf), tempfile.TemporaryDirectory(
                prefix="paintcraft-license-checkout-"
            ) as temporary:
                root = Path(temporary)
                repository = root / "repository"
                repository.mkdir()
                paths = set(NOTICES) | set(NOTICES.values())
                paths.update((".gitattributes", "packaging/.gitattributes"))
                for relative in paths:
                    source = ROOT / relative
                    if source.exists():
                        destination = repository / relative
                        destination.parent.mkdir(parents=True, exist_ok=True)
                        shutil.copyfile(source, destination)

                def git(*arguments):
                    subprocess.run(
                        ["git", "-c", f"core.autocrlf={autocrlf}", *arguments],
                        cwd=repository, check=True, capture_output=True,
                    )

                git("init", "--quiet")
                git("add", "--all")
                exported = root / "checkout"
                git("checkout-index", f"--prefix={exported.as_posix()}/", "--all")
                for source, notice in NOTICES.items():
                    original = (ROOT / source).read_bytes()
                    actual = (exported / source).read_bytes()
                    self.assertEqual(
                        hashlib.sha256(actual).hexdigest(),
                        hashlib.sha256(original).hexdigest(),
                        f"Git changed upstream bytes: {source}, autocrlf={autocrlf}",
                    )
                    self.assertTrue(
                        actual in (exported / notice).read_bytes(),
                        f"Packaged notice differs from {source}, autocrlf={autocrlf}",
                    )


if __name__ == "__main__":
    unittest.main()
