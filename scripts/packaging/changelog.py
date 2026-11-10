#!/usr/bin/env python3
"""Generate release notes without filtering release-tag anchors out of history."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[2]
VERSION = "git-cliff 2.14.2"
COMMIT_ID = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})\Z")


def run(command: list[str], repository: Path) -> bytes:
    # No inherited git-cliff path/range/output flags may change the collection.
    env = {key: value for key, value in os.environ.items()
           if not key.startswith("GIT_CLIFF_")}
    result = subprocess.run(command, cwd=repository, env=env,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    if result.returncode:
        detail = result.stderr.decode("utf-8", errors="replace").strip()
        raise RuntimeError(f"{command[0]} failed ({result.returncode}): {detail}")
    return result.stdout


def documentation_path(path: bytes) -> bool:
    return path.startswith((b"docs/", b"assets/repo/")) or path.endswith(b".md")


def filter_context(releases: list[dict], repository: Path) -> list[dict]:
    """Prune commit lists only. Tags, dates, previous releases and ordering stay intact."""
    classified: dict[str, bool] = {}

    def retain(commit: dict) -> bool:
        if not isinstance(commit, dict):
            raise ValueError("git-cliff context has an invalid commit")
        commit_id = commit.get("id")
        if not isinstance(commit_id, str) or not COMMIT_ID.fullmatch(commit_id):
            raise ValueError("git-cliff context has an invalid full commit ID")
        if commit_id not in classified:
            raw = run(["git", "diff-tree", "--root", "-r", "--no-commit-id",
                       "--name-only", "-z", commit_id], repository)
            paths = raw.split(b"\0")
            if paths and paths[-1] == b"":
                paths.pop()
            # Empty commits (and merges without a tree diff) are not docs-only.
            classified[commit_id] = bool(paths) and all(map(documentation_path, paths))
        return not classified[commit_id]

    def visit(release: dict) -> None:
        if not isinstance(release, dict) or not isinstance(release.get("commits"), list):
            raise ValueError("git-cliff context has an invalid release")
        release["commits"] = [commit for commit in release["commits"] if retain(commit)]
        previous = release.get("previous")
        if previous is not None:
            visit(previous)

    if not isinstance(releases, list):
        raise ValueError("git-cliff context must contain a release list")
    for release in releases:
        visit(release)
    return releases


def generate(tool: str, repository: Path, config: Path, tag: str | None = None) -> bytes:
    repository, config = repository.resolve(), config.resolve()
    settings = tomllib.loads(config.read_text(encoding="utf-8"))
    git = settings.get("git", {})
    if git.get("exclude_paths") or git.get("include_paths"):
        raise ValueError("cliff.toml must not filter paths before release-tag discovery")
    if run([tool, "--version"], repository).decode("utf-8").strip() != VERSION:
        raise ValueError(f"release changelogs require {VERSION}")
    command = [tool, "--offline", "--no-exec", "--config", str(config), "--context"]
    if tag:
        if not re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.-]+)?", tag):
            raise ValueError("invalid release tag")
        command += ["--tag", tag]
    context = filter_context(json.loads(run(command, repository)), repository)
    with tempfile.TemporaryDirectory(prefix="paintcraft-changelog-") as temp:
        path = Path(temp) / "context.json"
        path.write_text(json.dumps(context), encoding="utf-8")
        rendered = run([tool, "--offline", "--no-exec", "--config", str(config),
                        "--from-context", str(path)], repository)
        return rendered.rstrip(b"\n") + b"\n"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--git-cliff", default="git-cliff")
    parser.add_argument("--repository", type=Path, default=ROOT)
    parser.add_argument("--config", type=Path, default=ROOT / "cliff.toml")
    parser.add_argument("--output", type=Path, default=Path("CHANGELOG.md"))
    parser.add_argument("--tag")
    args = parser.parse_args()
    try:
        data = generate(args.git_cliff, args.repository, args.config, args.tag)
        # Only replace the destination after collection, filtering and rendering succeed.
        with tempfile.NamedTemporaryFile(dir=args.output.resolve().parent, delete=False) as tmp:
            temporary = Path(tmp.name)
            try:
                tmp.write(data)
                tmp.close()
                os.replace(temporary, args.output)
            finally:
                temporary.unlink(missing_ok=True)
    except (OSError, ValueError, RuntimeError) as error:
        parser.exit(1, f"changelog: {error}\n")


if __name__ == "__main__":
    main()
