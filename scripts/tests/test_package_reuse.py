"""Reused release artifacts require tagged source and every supported build."""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "packaging"))
import verify_release_run as reuse


class ArtifactProvenanceTests(unittest.TestCase):
    def setUp(self):
        self.sha = "a" * 40
        self.run = {"status": "completed", "head_sha": self.sha,
                    "path": ".github/workflows/release.yml", "event": "push"}
        self.jobs = [
            {"name": "Build " + target, "status": "completed", "conclusion": "success"}
            for target in ("x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc",
                           "aarch64-apple-darwin", "x86_64-apple-darwin",
                           "x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu")
        ]

    def test_removed_platform_failure_does_not_override_six_successful_builds(self):
        self.jobs.append({"name": "FreeBSD x86-64", "conclusion": "failure"})
        reuse.verify(self.run, self.jobs, self.sha)

    def test_different_tagged_source_and_incomplete_runs_are_refused(self):
        for field, value in (("head_sha", "b" * 40), ("status", "in_progress")):
            with self.subTest(field=field), self.assertRaises(ValueError):
                reuse.verify(self.run | {field: value}, self.jobs, self.sha)

    def test_missing_failed_and_duplicate_required_builds_are_refused(self):
        for jobs in (self.jobs[:-1], self.jobs + [self.jobs[0]],
                     self.jobs[:-1] + [self.jobs[-1] | {"conclusion": "failure"}]):
            with self.subTest(jobs=jobs), self.assertRaises(ValueError):
                reuse.verify(self.run, jobs, self.sha)

    def test_other_workflows_and_pull_request_artifacts_are_refused(self):
        for field, value in (("path", ".github/workflows/ci.yml"), ("event", "pull_request")):
            with self.subTest(field=field), self.assertRaises(ValueError):
                reuse.verify(self.run | {field: value}, self.jobs, self.sha)


if __name__ == "__main__":
    unittest.main()
