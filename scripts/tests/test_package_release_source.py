"""Immutable release checkout and final tag provenance, without executing Git."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import verify_release_tag as source

COMMIT = "a" * 40
OTHER = "b" * 40
OBJECT = "c" * 40
TAG = "v0.2.0"
REF = "refs/tags/" + TAG


class ReleaseSourceTests(unittest.TestCase):
    def test_lightweight_and_annotated_tags_select_the_actual_commit(self):
        self.assertEqual(source.remote_commit(f"{COMMIT}\t{REF}\n", TAG), COMMIT)
        self.assertEqual(source.remote_commit(f"{OBJECT}\t{REF}\n{COMMIT}\t{REF}^{{}}\n", TAG), COMMIT)
        self.assertEqual(source.remote_commit(f"{COMMIT}\t{REF}^{{}}\n{OBJECT}\t{REF}\n", TAG), COMMIT)

    def test_missing_deleted_malformed_duplicate_or_unexpected_refs_refuse(self):
        cases = ["", "\n", f"{COMMIT}\t{REF}^{{}}\n", f"{COMMIT}\t{REF}\n" * 2,
                 f"{COMMIT}\t{REF}\n{COMMIT}\t{REF}^{{}}\n", f"{COMMIT}\trefs/heads/main\n",
                 f"{COMMIT} {REF}\n", f"{COMMIT.upper()}\t{REF}\n", f"{COMMIT[:-1]}\t{REF}\n",
                 f"{COMMIT}\t{REF}\textra\n", f"{COMMIT}\t{REF}\n\n", "x" * 1025]
        for value in cases:
            with self.subTest(value=value), self.assertRaises(ValueError):
                source.remote_commit(value, TAG)
        for tag in ("main", "v", "v0.2.0\nrefs/heads/main", "--upload-pack=evil", "v../../x"):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                source.tag_ref(tag)

    def test_resolve_exports_actual_full_commit_without_overwriting_other_outputs(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "output"
            output.write_bytes(b"version=0.2.0\n")
            with patch.object(source, "git_output", return_value=COMMIT + "\n") as git, \
                    patch.dict(source.os.environ, {"GITHUB_OUTPUT": str(output)}):
                self.assertEqual(source.resolve(), COMMIT)
            git.assert_called_once_with(["rev-parse", "--verify", "HEAD^{commit}"])
            self.assertEqual(output.read_bytes(), f"version=0.2.0\nsource-sha={COMMIT}\n".encode())

    def test_final_verify_requires_checkout_and_remote_to_match_resolved_commit(self):
        for advertisement in (f"{COMMIT}\t{REF}\n", f"{OBJECT}\t{REF}\n{COMMIT}\t{REF}^{{}}\n"):
            with patch.object(source, "git_output", side_effect=[COMMIT + "\n", advertisement]) as git:
                self.assertEqual(source.verify(TAG, COMMIT), COMMIT)
                self.assertEqual(git.call_args_list[1].args[0],
                                 ["ls-remote", "--exit-code", "--tags", "origin", REF, REF + "^{}"])
        for advertisement in (f"{OTHER}\t{REF}\n", f"{OBJECT}\t{REF}\n{OTHER}\t{REF}^{{}}\n", ""):
            with patch.object(source, "git_output", side_effect=[COMMIT + "\n", advertisement]), \
                    self.assertRaises(ValueError):
                source.verify(TAG, COMMIT)
        with patch.object(source, "git_output", return_value=OTHER + "\n") as git, self.assertRaises(ValueError):
            source.verify(TAG, COMMIT)
        self.assertEqual(git.call_count, 1)
        with patch.object(source, "git_output") as git, self.assertRaises(ValueError):
            source.verify("--invalid", COMMIT)
        git.assert_not_called()

    def test_git_is_literal_noninteractive_read_only_and_errors_hide_diagnostics(self):
        with patch.object(source.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, COMMIT)) as run:
            self.assertEqual(source.current_commit(), COMMIT)
        args, kwargs = run.call_args
        self.assertEqual(args[0], ["git", "rev-parse", "--verify", "HEAD^{commit}"])
        self.assertNotIn("shell", kwargs)
        self.assertEqual(kwargs["env"]["GIT_TERMINAL_PROMPT"], "0")
        self.assertEqual(kwargs["stderr"], subprocess.DEVNULL)
        for result in (subprocess.CompletedProcess([], 2, "secret"), OSError("secret"),
                       subprocess.TimeoutExpired("secret", 30)):
            with patch.object(source.subprocess, "run", **({"side_effect": result} if isinstance(result, Exception)
                                                           else {"return_value": result})), \
                    self.assertRaisesRegex(ValueError, "^Could not read Git release provenance$"):
                source.git_output(["ls-remote"])

    def test_workflow_pins_all_five_checkouts_and_checks_before_publish_side_effects(self):
        workflow = (Path(__file__).resolve().parents[2] / ".github/workflows/release.yml").read_text()
        self.assertEqual(workflow.count("uses: actions/checkout@v7"), 5)
        self.assertEqual(workflow.count("ref: ${{ inputs.publish-tag || github.sha }}"), 1)
        self.assertEqual(workflow.count("ref: ${{ needs.version.outputs.source-sha }}"), 4)
        self.assertNotIn("ref: ${{ inputs.publish-tag || github.ref }}", workflow)
        self.assertIn("source-sha: ${{ steps.source.outputs.source-sha }}", workflow)
        self.assertIn("id: source\n        run: python scripts/packaging/verify_release_tag.py resolve", workflow)
        publish = workflow.split("\n  publish:", 1)[1]
        gate = 'python scripts/packaging/verify_release_tag.py verify --tag "$RELEASE_TAG" --source-sha "$RELEASE_SOURCE_SHA"'
        self.assertEqual(publish.count(gate), 2)
        self.assertLess(publish.index(gate), publish.index("gh release create"))
        self.assertLess(publish.index(gate), publish.index("gh release upload"))
        self.assertLess(publish.rindex(gate), publish.index("gh release edit"))
        self.assertIn("RELEASE_SOURCE_SHA: ${{ needs.version.outputs.source-sha }}", publish)


if __name__ == "__main__":
    unittest.main()
