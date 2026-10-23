"""Stage the qualified SVGPNG candidate packages before Bundler resolves gems."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import zipfile


ARTIFACT_ID = "12495376361"
ARCHIVE_SHA256 = "faa382ee2f3c801c2396350aee725baf3b13d78ba778e4852a3f7a05b09a500e"
MANIFEST_SHA256 = "d7561c29755a66c6f381d22c0bfdbdc3ac9e99b0bbb1c793beb0a988a808cf28"
TARGETS = {
    "arm64-darwin", "x86_64-darwin", "x86_64-linux", "aarch64-linux",
    "x86_64-linux-musl", "aarch64-linux-musl", "x64-mingw-ucrt", "ruby",
}


def verified_packages(manifest, read_package):
    if hashlib.sha256(manifest).hexdigest() != MANIFEST_SHA256:
        raise ValueError("SVGPNG qualified manifest checksum mismatch")
    entries = json.loads(manifest)["artifacts"]
    if len(entries) != 8 or {entry["target"] for entry in entries} != TARGETS:
        raise ValueError("SVGPNG requires all seven native packages and Ruby fallback")
    packages = {}
    for entry in entries:
        name = entry["path"]
        if Path(name).name != name:
            raise ValueError("SVGPNG package path must be a basename")
        data = read_package(name)
        if hashlib.sha256(data).hexdigest() != entry["sha256"]:
            raise ValueError(f"SVGPNG package checksum mismatch: {name}")
        packages[name] = data
    return packages


def stage_directory(source, destination):
    manifest = (source / "checksums.json").read_bytes()
    packages = verified_packages(manifest, lambda name: (source / name).read_bytes())
    destination.mkdir(parents=True, exist_ok=True)
    for name, data in packages.items():
        (destination / name).write_bytes(data)
    (destination / "checksums.json").write_bytes(manifest)


def fetch_upstream(destination):
    # This immutable artifact qualified source 896faa05a9e0c3d96d17bf1a79b92e1c566bb0ce.
    # Access/retention failures must fail CI, never fall back to rebuilding packages.
    with tempfile.TemporaryDirectory() as temporary:
        archive = Path(temporary) / "candidate.zip"
        with archive.open("wb") as output:
            subprocess.run([
                "gh", "api", f"repos/claricle/svgpng/actions/artifacts/{ARTIFACT_ID}/zip",
            ], stdout=output, check=True)
        if hashlib.sha256(archive.read_bytes()).hexdigest() != ARCHIVE_SHA256:
            raise ValueError("SVGPNG qualified archive checksum mismatch")
        with zipfile.ZipFile(archive) as bundle:
            manifests = [name for name in bundle.namelist()
                         if Path(name).name == "checksums.json"
                         and hashlib.sha256(bundle.read(name)).hexdigest() == MANIFEST_SHA256]
            if len(manifests) != 1:
                raise ValueError("SVGPNG archive must contain the qualified manifest")
            prefix = manifests[0].removesuffix("checksums.json")
            manifest = bundle.read(manifests[0])
            packages = verified_packages(manifest, lambda name: bundle.read(prefix + name))
            destination.mkdir(parents=True, exist_ok=True)
            for name, data in packages.items():
                (destination / name).write_bytes(data)
            (destination / "checksums.json").write_bytes(manifest)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["fetch", "stage"])
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source", type=Path)
    args = parser.parse_args()
    if args.mode == "fetch":
        fetch_upstream(args.output)
    else:
        if args.source is None:
            parser.error("stage requires --source")
        stage_directory(args.source, args.output)
    print("Verified and staged all eight qualified SVGPNG packages")


if __name__ == "__main__":
    main()
