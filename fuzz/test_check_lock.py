"""Control the dependency identity check without changing either real lockfile."""
import unittest
from pathlib import Path
import subprocess
import tempfile
import tomllib
from check_lock import differences, missing_edges, path_manifests


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

    def test_real_manifest_edges_missing_from_an_otherwise_identical_lock(self):
        # Reproduce the actual stale-lock shape: dependency packages still
        # exist transitively, but formats' new direct edge is absent. Cargo
        # supplies workspace, alias and target interpretation, not a hand-built
        # metadata object that repeats the checker's implementation.
        with tempfile.TemporaryDirectory(prefix="paintcraft-lock-edge-") as temp:
            root = Path(temp)
            (root / "Cargo.toml").write_text('''[workspace]
members = ["formats", "png", "gif", "zune-jpeg", "dev", "optional"]
resolver = "2"
[workspace.dependencies]
renamed-png = { package = "png", path = "png" }
''')
            for name in ["formats", "png", "gif", "zune-jpeg", "dev", "optional"]:
                directory = root / name
                (directory / "src").mkdir(parents=True)
                (directory / "src/lib.rs").write_text("")
                manifest = f'[package]\nname = "{name}"\nversion = "1.0.0"\nedition = "2024"\n'
                if name == "formats":
                    manifest += '''[dependencies]
renamed-png.workspace = true
zune-jpeg = { path = "../zune-jpeg" }
optional = { path = "../optional", optional = true }
[target.'cfg(target_os = "nonexistent")'.dependencies]
gif = { path = "../gif" }
[dev-dependencies]
dev = { path = "../dev" }
'''
                (directory / "Cargo.toml").write_text(manifest)
            subprocess.run(["cargo", "tree", "--offline", "--depth", "0", "--manifest-path", str(root / "Cargo.toml")], check=True, capture_output=True)
            lock = tomllib.loads((root / "Cargo.lock").read_text())["package"]
            packages = path_manifests(root / "Cargo.toml")
            self.assertEqual(missing_edges(packages, lock), [])
            for dependency in ["png", "gif", "zune-jpeg"]:
                stale = [dict(p) for p in lock]
                formats = next(p for p in stale if p["name"] == "formats")
                formats["dependencies"] = [d for d in formats["dependencies"] if d != dependency]
                self.assertEqual(differences(lock, stale), [])
                self.assertEqual(missing_edges(packages, stale), [("formats", dependency)])
            excluded = [dict(p) for p in lock]
            formats = next(p for p in excluded if p["name"] == "formats")
            formats["dependencies"] = [d for d in formats["dependencies"] if d not in ["dev", "optional"]]
            self.assertEqual(missing_edges(packages, excluded), [])
            self.assertEqual(missing_edges(packages, [{"name": "unknown", "version": "1.0.0"}]), [("unknown", "missing path manifest")])


if __name__ == "__main__":
    unittest.main()
