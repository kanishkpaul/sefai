"""Assemble one release archive from a built sefai binary.

Usage:
  python3 scripts/package.py --binary target/release/sefai --name sefai-0.1.0-macos-arm64 \
      --out dist [-- CARGO_FEATURE_ARGS...]

CARGO_FEATURE_ARGS are the feature flags the binary was built with, for example
`--features metal,mlx`; they select which dependencies the third-party notices cover.
The archive holds the executable, any runtime library it loads from beside itself,
LICENSE.txt, THIRD_PARTY_NOTICES.txt and INSTALL.txt, and gets a `.sha256` file.
No network access and no build happen here.
"""

import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
import tarfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LICENSE_PREFIXES = ("license", "licence", "copying", "notice", "unlicense")
# Libraries a feature makes the executable load from its own directory at run time.
SIDECAR_PATTERNS = ("libwebgpu_dawn.*", "libonnxruntime*.so*", "onnxruntime*.dll", "*.dylib")

MIT = """MIT License

Copyright (c) {holders}

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
"""

INSTALL = """sefai {version} for {name}

Install: move this directory anywhere and put it on your PATH, or run ./sefai from it.
Keep every file in this directory together; libraries and mlx.metallib load from beside the executable.
Start: sefai chat, then /setup to choose a model. sefai never downloads a model or runtime on its own.
Update: replace this directory with a newer archive; your workspace Markdown is never inside it.
Uninstall: delete this directory. Your workspace stays in your platform's configuration directory until you delete it.

Full guide: https://github.com/kanishkpaul/sefai/blob/main/docs/install.md
"""


def host_triple():
    output = subprocess.run(["rustc", "-vV"], capture_output=True, text=True, check=True).stdout
    return next(line.split(": ", 1)[1] for line in output.splitlines() if line.startswith("host: "))


def resolved_packages(feature_args):
    metadata = json.loads(subprocess.run(
        ["cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", host_triple(),
         "--manifest-path", str(ROOT / "Cargo.toml"), *feature_args],
        capture_output=True, text=True, check=True).stdout)
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    root = next(identifier for identifier, package in packages.items() if package["name"] == "sefai" and package["source"] is None)
    seen, pending = set(), [root]
    while pending:
        identifier = pending.pop()
        if identifier in seen:
            continue
        seen.add(identifier)
        for dependency in nodes[identifier]["deps"]:
            # Build scripts and proc macros run at build time; their code is not in the binary.
            if any(kind["kind"] is None for kind in dependency["dep_kinds"]):
                pending.append(dependency["pkg"])
    return sorted((packages[identifier] for identifier in seen), key=lambda package: (package["name"], package["version"]))


def license_texts(directory):
    texts = []
    for path in sorted(directory.iterdir()):
        if path.is_file() and path.name.lower().startswith(LICENSE_PREFIXES):
            texts.append((path.name, path.read_text(errors="replace")))
    return texts


def native_notices(names):
    notices = []
    if "llama-cpp-sys-2" in names:
        crate = next(Path(package["manifest_path"]).parent for package in names["llama-cpp-sys-2"])
        notices.append(("llama.cpp and ggml", crate / "llama.cpp" / "LICENSE"))
    if "mlx-sys" in names:
        notices.append(("mlx-c", ROOT / "vendor/mlx-sys/src/mlx-c/LICENSE"))
        built = sorted(ROOT.glob("target/*/build/mlx-sys-*/out/build/_deps/mlx-src/LICENSE"))
        if not built:
            raise SystemExit("MLX license not found; build the mlx feature before packaging")
        notices.append(("MLX", built[-1]))
    if "ort-sys" in names:
        notices.append(("ONNX Runtime 1.31.0", ROOT / "licenses/onnxruntime-LICENSE.txt"))
        notices.append(("ONNX Runtime third-party notices", ROOT / "licenses/onnxruntime-ThirdPartyNotices.txt"))
    return notices


def notices(feature_args, sidecars):
    # Everything except sefai's own crates is third-party, including the vendored mlx crates.
    own = ROOT / "crates"
    packages = [package for package in resolved_packages(feature_args) if not Path(package["manifest_path"]).is_relative_to(own)]
    names = {}
    for package in packages:
        names.setdefault(package["name"], []).append(package)
    lines = ["Third-party software in this sefai build", "", "Rust crates compiled into the executable:", ""]
    for package in packages:
        lines.append(f"{package['name']} {package['version']} — {package.get('license') or 'see license file'} — {package.get('repository') or ''}")
    lines.append("")
    native = native_notices(names)
    if any(path.name.startswith("libwebgpu_dawn") for path in sidecars):
        native.append(("Dawn (WebGPU)", ROOT / "licenses/dawn-LICENSE.txt"))
    for title, path in native:
        lines += ["=" * 72, title, "=" * 72, path.read_text(errors="replace"), ""]
    for package in packages:
        texts = license_texts(Path(package["manifest_path"]).parent)
        lines += ["=" * 72, f"{package['name']} {package['version']}", "=" * 72]
        if texts:
            for name, text in texts:
                lines += [f"--- {name}", text, ""]
        elif "MIT" in (package.get("license") or "").replace("/", " OR ").split(" OR "):
            # The crate ships no license file; its declared MIT option is reproduced with its authors.
            holders = ", ".join(package.get("authors") or [f"the {package['name']} authors"])
            lines += [f"Used under MIT, one option of its declared license {package.get('license')}.", MIT.format(holders=holders), ""]
        else:
            raise SystemExit(f"{package['name']} ships no license file and offers no MIT option; add its text")
    return "\n".join(lines)


def sha256(path):
    with open(path, "rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def main():
    split = sys.argv.index("--") if "--" in sys.argv else len(sys.argv)
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--name", required=True)
    parser.add_argument("--out", type=Path, required=True)
    options = parser.parse_args(sys.argv[1:split])
    feature_args = sys.argv[split + 1:]
    version = subprocess.run([str(options.binary), "--version"], capture_output=True, text=True, check=True).stdout.split()[-1]
    stage = options.out / options.name
    if stage.exists():
        shutil.rmtree(stage)
    stage.mkdir(parents=True)
    shutil.copy2(options.binary, stage / options.binary.name)
    sidecars = []
    for pattern in SIDECAR_PATTERNS:
        for path in options.binary.parent.glob(pattern):
            if path.resolve().is_file() and path.name not in {sidecar.name for sidecar in sidecars}:
                shutil.copyfile(path.resolve(), stage / path.name)
                sidecars.append(stage / path.name)
    if any("mlx" in argument for argument in feature_args):
        metallib = Path(os.environ.get("MLX_RS_METAL_PATH", ROOT / "target/mlx-metal")) / "mlx.metallib"
        if not metallib.is_file():
            raise SystemExit(f"{metallib} is missing; MLX builds need it beside the executable")
        shutil.copyfile(metallib, stage / "mlx.metallib")
    shutil.copyfile(ROOT / "LICENSE", stage / "LICENSE.txt")
    (stage / "THIRD_PARTY_NOTICES.txt").write_text(notices(feature_args, sidecars))
    (stage / "INSTALL.txt").write_text(INSTALL.format(version=version, name=options.name))
    if platform.system() == "Windows":
        archive = options.out / f"{options.name}.zip"
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as output:
            for path in sorted(stage.rglob("*")):
                output.write(path, path.relative_to(options.out))
    else:
        archive = options.out / f"{options.name}.tar.gz"
        with tarfile.open(archive, "w:gz") as output:
            output.add(stage, arcname=options.name)
    (options.out / f"{archive.name}.sha256").write_text(f"{sha256(archive)}  {archive.name}\n")
    print(archive)


if __name__ == "__main__":
    main()
