"""Download a pinned model fixture manifest and verify every file's size and SHA-256.

Usage: python3 scripts/fetch_fixture.py tests/fixtures/<name>.json DESTINATION

A single-file manifest (`file`, `bytes`, `sha256`) is written to DESTINATION itself;
a multi-file manifest (`files`) is written beneath the DESTINATION directory.
Files that already match are kept, so an interrupted fetch can be rerun.
"""

import hashlib
import json
import sys
import urllib.request
from pathlib import Path

CHUNK_BYTES = 1024 * 1024


def digest(path):
    with open(path, "rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def fetch(repository, revision, name, expected, target):
    if target.is_file() and target.stat().st_size == expected["bytes"] and digest(target) == expected["sha256"]:
        return
    target.parent.mkdir(parents=True, exist_ok=True)
    partial = target.with_name(target.name + ".partial")
    url = f"https://huggingface.co/{repository}/resolve/{revision}/{name}"
    with urllib.request.urlopen(url, timeout=60) as response, open(partial, "wb") as output:
        while chunk := response.read(CHUNK_BYTES):
            output.write(chunk)
    if partial.stat().st_size != expected["bytes"]:
        raise SystemExit(f"{name}: size mismatch")
    if digest(partial) != expected["sha256"]:
        raise SystemExit(f"{name}: sha256 mismatch")
    partial.replace(target)


def main():
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    manifest = json.loads(Path(sys.argv[1]).read_text())
    destination = Path(sys.argv[2])
    if "file" in manifest:
        fetch(manifest["repository"], manifest["revision"], manifest["file"], manifest, destination)
        return
    for name, expected in manifest["files"].items():
        relative = Path(name)
        if relative.is_absolute() or ".." in relative.parts:
            raise SystemExit(f"{name}: manifest path escapes the destination")
        fetch(manifest["repository"], manifest["revision"], name, expected, destination / relative)


if __name__ == "__main__":
    main()
