"""Release boundaries and path omission using owned repositories and the pinned real tool."""
from pathlib import Path
import copy
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import changelog

CONFIG = r'''[changelog]
header = "# Changelog\n\n"
body = """## {{ version | default(value="Unreleased") }}
{% for commit in commits %}- {{ commit.message }}
{% endfor %}"""
trim = true
[git]
conventional_commits = false
filter_unconventional = false
tag_pattern = '^v[0-9]+\.[0-9]+\.[0-9]+$'
commit_parsers = [{message = "^docs:", skip = true}, {message = ".*", group = "Changes"}]
'''


class ChangelogTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="paintcraft-changelog-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.config = self.root / "cliff.toml"
        self.config.write_text(CONFIG)

    def test_documentation_paths_are_exact_and_support_raw_names(self):
        for path in [b"README.md", b"a/b.md", b"docs/notes.txt", b"assets/repo/demo.png",
                     b"docs/newline\nname", b"docs/nonutf8-\xff"]:
            self.assertTrue(changelog.documentation_path(path), path)
        for path in [b"src/lib.rs", b"docs-not-source.rs", b"README.MD", b"assets/icon.png"]:
            self.assertFalse(changelog.documentation_path(path), path)

    def test_pruning_keeps_release_metadata_and_an_empty_release_object(self):
        data = [{"version": "v0.1.1", "timestamp": 123, "commits": [{"id": "1" * 40}],
                 "previous": {"version": "v0.1.0", "commits": []}}]
        expected = copy.deepcopy(data)
        expected[0]["commits"] = []
        with patch.object(changelog, "run", return_value=b"README.md\0") as run:
            self.assertEqual(changelog.filter_context(data, self.root), expected)
        self.assertEqual(run.call_count, 1)

    def test_empty_commits_are_kept_and_repeated_ids_use_one_tree_query(self):
        data = [{"version": None, "commits": [{"id": "a" * 40}, {"id": "a" * 40}]}]
        with patch.object(changelog, "run", return_value=b"") as run:
            self.assertEqual(len(changelog.filter_context(data, self.root)[0]["commits"]), 2)
        self.assertEqual(run.call_count, 1)

    def test_invalid_context_ids_refuse_before_git(self):
        with patch.object(changelog, "run") as run:
            with self.assertRaisesRegex(ValueError, "commit ID"):
                changelog.filter_context([{"commits": [{"id": "--help"}]}], self.root)
            run.assert_not_called()
        for bad in [{}, [{"version": "v1"}], [{"commits": ["not a commit"]}]]:
            with self.assertRaises(ValueError):
                changelog.filter_context(bad, self.root)

    def test_path_filter_config_cannot_silently_reintroduce_boundary_loss(self):
        self.config.write_text(CONFIG + '\nexclude_paths = ["docs/**"]\n')
        with patch.object(changelog, "run") as run:
            with self.assertRaisesRegex(ValueError, "must not filter paths"):
                changelog.generate("unused", self.root, self.config)
            run.assert_not_called()

    def test_wrong_version_bad_context_and_tool_failure_are_not_swallowed(self):
        with patch.object(changelog, "run", return_value=b"git-cliff 2.13.0\n"):
            with self.assertRaisesRegex(ValueError, "2.14.2"):
                changelog.generate("unused", self.root, self.config)
        with patch.object(changelog, "run", side_effect=[b"git-cliff 2.14.2\n", b"bad JSON"]):
            with self.assertRaises(json.JSONDecodeError):
                changelog.generate("unused", self.root, self.config)
        failure = subprocess.CompletedProcess(["bad-tool"], 7, b"", b"tool refused")
        with patch.object(subprocess, "run", return_value=failure):
            with self.assertRaisesRegex(RuntimeError, "tool refused"):
                changelog.run(["bad-tool"], self.root)

    def test_real_annotated_docs_anchor_non_doc_subject_mixed_and_empty_commit(self):
        tool = os.environ.get("PAINTCRAFT_GIT_CLIFF") or shutil.which("git-cliff")
        if not tool:
            local = Path.cwd() / "target/release-tools/git-cliff-2.14.2/git-cliff"
            tool = str(local) if local.is_file() else None
        if not tool:
            self.skipTest("requires actual pinned git-cliff 2.14.2; set PAINTCRAFT_GIT_CLIFF")
        tool = str(Path(tool).resolve())
        env = dict(os.environ, GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1",
                   GIT_AUTHOR_NAME="Changelog fixture", GIT_AUTHOR_EMAIL="fixture@example.invalid",
                   GIT_COMMITTER_NAME="Changelog fixture", GIT_COMMITTER_EMAIL="fixture@example.invalid")

        def git(*args):
            return subprocess.run(["git", "-c", "commit.gpgsign=false", "-C", str(self.root), *args],
                                  env=env, check=True, stdout=subprocess.PIPE,
                                  stderr=subprocess.PIPE).stdout

        def commit(message, files):
            for name, content in files.items():
                path = self.root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content)
            git("add", "--all")
            git("commit", "--allow-empty", "-qm", message)

        git("init", "-q", "-b", "main")
        commit("feat: baseline engine", {"engine.rs": "base"})
        git("-c", "tag.gpgsign=false", "tag", "-a", "v0.1.0", "-m", "first")
        commit("feat: second engine", {"engine.rs": "second"})
        commit("docs: changelog anchor", {"CHANGELOG.md": "anchor"})
        git("-c", "tag.gpgsign=false", "tag", "-a", "v0.1.1", "-m", "second")
        commit("feat: README only", {"README.md": "not a feature"})
        commit("perf: screenshot only", {"assets/repo/scene.png": "not a perf change"})
        commit("fix: mixed source and documentation", {"engine.rs": "third", "docs/mixed.md": "note"})
        commit("chore: retained empty marker", {})
        for tag in [None, "v0.2.0"]:
            rendered = changelog.generate(tool, self.root, self.config, tag)
            self.assertTrue(rendered.endswith(b"\n"))
            self.assertFalse(rendered.endswith(b"\n\n"))
            output = rendered.decode()
            current = "v0.2.0" if tag else "Unreleased"
            self.assertEqual(output.count("## v0.1.1"), 1)
            self.assertEqual(output.count("## v0.1.0"), 1)
            self.assertLess(output.index("## " + current), output.index("## v0.1.1"))
            self.assertLess(output.index("## v0.1.1"), output.index("## v0.1.0"))
            unreleased, older = output.split("## v0.1.1", 1)
            self.assertIn("mixed source and documentation", unreleased)
            self.assertIn("retained empty marker", unreleased)
            self.assertNotIn("second engine", unreleased)
            self.assertIn("second engine", older.split("## v0.1.0")[0])
            self.assertIn("baseline engine", older.split("## v0.1.0")[1])
            for omitted in ["README only", "screenshot only", "changelog anchor"]:
                self.assertNotIn(omitted, output)
        self.assertEqual(git("tag", "--list", "v0.2.0"), b"")


if __name__ == "__main__":
    unittest.main()
