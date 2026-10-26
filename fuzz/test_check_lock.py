"""Control the dependency identity check without changing either real lockfile."""
import unittest
from check_lock import differences


class LockIdentity(unittest.TestCase):
    def test_matching_identity_and_each_kind_of_drift(self):
        package = {"name": "parser", "version": "1.0.0", "source": "registry+index", "checksum": "a" * 64}
        self.assertEqual(differences([package], [dict(package)]), [])
        for field, value in [("version", "2.0.0"), ("source", "registry+other"), ("checksum", "b" * 64)]:
            with self.subTest(field=field):
                self.assertEqual(len(differences([package], [{**package, field: value}])), 1)
        missing = dict(package)
        del missing["checksum"]
        self.assertEqual(len(differences([package], [missing])), 1)

    def test_path_packages_and_exact_developer_exemptions(self):
        path = {"name": "paintcraft-formats", "version": "0.1.0"}
        self.assertEqual(differences([path], [path]), [])
        self.assertEqual(len(differences([], [path])), 1)
        for name in ["paintcraft-container-fuzz", "libfuzzer-sys", "arbitrary"]:
            self.assertEqual(differences([], [{"name": name, "version": "0.0.0"}]), [])


if __name__ == "__main__":
    unittest.main()
