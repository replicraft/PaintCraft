#!/usr/bin/env python3
"""Pin release jobs to one commit and reject deleted or moved publishing tags."""
import argparse
import os
from pathlib import Path
import re
import subprocess

import package_release as release

SHA = re.compile(r"[0-9a-f]{40}")


def valid_sha(value):
    if not isinstance(value, str) or SHA.fullmatch(value) is None:
        raise ValueError("Release source must be one complete lowercase Git commit SHA")
    return value


def tag_ref(tag):
    if not isinstance(tag, str) or not tag.startswith("v") or not release.VERSION.fullmatch(tag[1:]):
        raise ValueError("Publishing requires a version tag")
    return "refs/tags/" + tag


def remote_commit(advertisement, tag):
    """Read the exact lightweight ref or the fully peeled annotated-tag object."""
    direct = tag_ref(tag)
    peeled = direct + "^{}"
    refs = {}
    if not advertisement or len(advertisement) > 1024:
        raise ValueError("Publishing tag is missing or its advertisement is invalid")
    for line in advertisement.splitlines():
        fields = line.split("\t")
        if len(fields) != 2:
            raise ValueError("Malformed remote tag advertisement")
        sha, ref = fields
        valid_sha(sha)
        if ref not in (direct, peeled) or ref in refs:
            raise ValueError("Unexpected or duplicate remote tag reference")
        refs[ref] = sha
    if direct not in refs:
        raise ValueError("Publishing tag no longer exists")
    if peeled in refs:
        if refs[direct] == refs[peeled]:
            raise ValueError("Malformed annotated tag advertisement")
        return refs[peeled]
    return refs[direct]


def git_output(arguments):
    env = dict(os.environ)
    env["GIT_TERMINAL_PROMPT"] = "0"
    try:
        result = subprocess.run(["git", *arguments], stdout=subprocess.PIPE,
                                stderr=subprocess.DEVNULL, text=True, encoding="ascii",
                                env=env, timeout=30, check=False)
    except (OSError, subprocess.TimeoutExpired, UnicodeError):
        raise ValueError("Could not read Git release provenance") from None
    if result.returncode != 0:
        raise ValueError("Could not read Git release provenance")
    return result.stdout


def current_commit():
    return valid_sha(git_output(["rev-parse", "--verify", "HEAD^{commit}"]).strip())


def resolve():
    sha = current_commit()
    if output := os.environ.get("GITHUB_OUTPUT"):
        with Path(output).open("a", encoding="utf-8", newline="\n") as stream:
            stream.write(f"source-sha={sha}\n")
    return sha


def verify(tag, source_sha):
    source_sha = valid_sha(source_sha)
    ref = tag_ref(tag)
    if current_commit() != source_sha:
        raise ValueError("Publishing checkout differs from the resolved release source")
    actual = remote_commit(git_output(["ls-remote", "--exit-code", "--tags", "origin", ref, ref + "^{}"]), tag)
    if actual != source_sha:
        raise ValueError("Publishing tag moved away from the resolved release source")
    return actual


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("resolve")
    check = commands.add_parser("verify")
    check.add_argument("--tag", required=True)
    check.add_argument("--source-sha", required=True)
    args = parser.parse_args()
    try:
        sha = resolve() if args.command == "resolve" else verify(args.tag, args.source_sha)
    except (OSError, ValueError) as error:
        parser.exit(1, f"Release source verification failed: {error}\n")
    print(sha)


if __name__ == "__main__":
    main()
