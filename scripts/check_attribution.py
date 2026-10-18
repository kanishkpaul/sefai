#!/usr/bin/env python3
"""Check commit messages, added lines, and pull request metadata."""

import argparse
import re
import subprocess
import sys
from pathlib import Path

ATTRIBUTION = re.compile(
    r"co[\s-]*authored[\s-]*by\s*:|"
    r"(?:generated|written|created|authored|assisted|powered)\s+(?:with|by)\s+"
    r"[^\n]*(?:claude|anthropic|openai|chatgpt|gpt[- ]|codex|copilot|cursor|gemini|"
    r"devin|aider|windsurf|codeium|cline|tabnine|\bai\b|coding agent|assistant)|"
    r"noreply@anthropic\.com",
    re.IGNORECASE,
)


def check(text, label):
    if ATTRIBUTION.search(text):
        print(f"attribution is not allowed in {label}", file=sys.stderr)
        return False
    return True


def git(*arguments):
    return subprocess.check_output(["git", *arguments], text=True, encoding="utf-8")


def check_range(base, head):
    valid = True
    for commit in git("rev-list", f"{base}..{head}").splitlines():
        valid = check(git("show", "-s", "--format=%B", commit), commit) and valid
    return valid


def check_diff(base, head):
    patch = git("diff", "--no-ext-diff", "--unified=0", base, head, "--", ".", ":(exclude)scripts/check_attribution.py", ":(exclude)tests/test_attribution.py")
    added = "\n".join(line[1:] for line in patch.splitlines() if line.startswith("+") and not line.startswith("+++"))
    valid = True
    signature = re.compile(r"^\s*(?:(?:<!--|//|[#*;!>-])+\s*)?(?:co[\s-]*authored[\s-]*by\s*:|(?:generated|written|created|authored|assisted|powered)\s+(?:with|by)\s+)", re.IGNORECASE)
    for line in added.splitlines():
        if signature.search(line):
            valid = check(line, "added content") and valid
    return valid


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    message = commands.add_parser("message")
    message.add_argument("path", type=Path)
    revision = commands.add_parser("range")
    revision.add_argument("base")
    revision.add_argument("head")
    content = commands.add_parser("diff")
    content.add_argument("base")
    content.add_argument("head")
    arguments = parser.parse_args()
    if arguments.command == "message":
        valid = check(arguments.path.read_text(encoding="utf-8"), str(arguments.path))
    elif arguments.command == "range":
        valid = check_range(arguments.base, arguments.head)
    else:
        valid = check_diff(arguments.base, arguments.head)
    return 0 if valid else 1


if __name__ == "__main__":
    sys.exit(main())
