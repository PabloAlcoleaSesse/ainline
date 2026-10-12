#!/usr/bin/env python3
"""Install one explicit GitHub release, verifying SHA-256 before extracting its binary."""
import argparse
import hashlib
import io
import os
from pathlib import Path
import platform
import re
import tarfile
import tempfile
import urllib.request
import zipfile

REPOSITORY = "PabloAlcoleaSesse/ainline"


def target(system=None, machine=None):
    system, machine = system or platform.system(), (machine or platform.machine()).lower()
    arch = {"x86_64": "x86_64", "amd64": "x86_64", "arm64": "aarch64", "aarch64": "aarch64"}.get(machine)
    suffix = {"Darwin": "apple-darwin", "Linux": "unknown-linux-gnu", "Windows": "pc-windows-msvc"}.get(system)
    if not arch or not suffix or (system == "Windows" and arch != "x86_64") or (system == "Linux" and arch != "x86_64"):
        raise ValueError(f"No release binary for {system}/{machine}; install with Cargo")
    return f"{arch}-{suffix}"


def download(url, maximum):
    with urllib.request.urlopen(url, timeout=60) as response:
        data = response.read(maximum + 1)
    if len(data) > maximum:
        raise ValueError("Download exceeds size limit")
    return data


def install(archive, manifest, name, destination, force=False):
    entries = [line.split() for line in manifest.splitlines() if line.strip()]
    matches = [digest for digest, filename in entries if filename.lstrip("*") == name]
    if len(matches) != 1 or hashlib.sha256(archive).hexdigest() != matches[0]:
        raise ValueError("Missing or mismatched SHA-256 checksum")
    binary = "ai.exe" if name.endswith(".zip") else "ai"
    if name.endswith(".zip"):
        with zipfile.ZipFile(io.BytesIO(archive)) as package:
            info = package.getinfo(binary)
            if info.file_size > 100 * 1024 * 1024:
                raise ValueError("Binary exceeds size limit")
            data = package.read(binary)
    else:
        with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as package:
            info = package.getmember(binary)
            if not info.isfile() or info.size > 100 * 1024 * 1024:
                raise ValueError("Invalid binary archive member")
            data = package.extractfile(info).read()
    destination = Path(destination)
    destination.mkdir(parents=True, exist_ok=True)
    output = destination / binary
    if output.is_symlink() or (output.exists() and not force):
        raise ValueError(f"Refusing to overwrite {output}; use --force for an existing regular file")
    fd, temporary = tempfile.mkstemp(prefix=".ainline-", dir=destination)
    try:
        with os.fdopen(fd, "wb") as file:
            file.write(data)
            file.flush()
            os.fsync(file.fileno())
        os.chmod(temporary, 0o755)
        if force:
            os.replace(temporary, output)
        else:
            # Atomic no-clobber publication on supported release platforms.
            os.link(temporary, output)
        return output
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version", help="Published release tag, e.g. v1.0.0")
    parser.add_argument("--bin-dir", type=Path, default=Path.home() / ".local" / "bin")
    parser.add_argument("--force", action="store_true", help="Replace an existing regular binary")
    args = parser.parse_args()
    if not re.fullmatch(r"v\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?", args.version):
        parser.error("Expected an explicit version tag such as v1.0.0")
    try:
        release_target = target()
        name = f"ai-{release_target}." + ("zip" if platform.system() == "Windows" else "tar.gz")
        base = f"https://github.com/{REPOSITORY}/releases/download/{args.version}"
        manifest = download(f"{base}/SHA256SUMS", 1024 * 1024).decode("utf-8")
        archive = download(f"{base}/{name}", 100 * 1024 * 1024)
        output = install(archive, manifest, name, args.bin_dir, args.force)
    except (OSError, ValueError, tarfile.TarError, zipfile.BadZipFile, KeyError) as error:
        parser.exit(1, f"Install failed: {error}\n")
    print(f"Installed {output}. Add {output.parent} to PATH if needed.")
    print("Configuration and provider installations were not changed.")


if __name__ == "__main__":
    main()
