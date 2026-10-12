import hashlib
import importlib.util
import io
from pathlib import Path
import tarfile
import tempfile
import unittest
import zipfile


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).parents[1] / filename)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


installer = module("installer", "install.py")
metadata = module("metadata", "release-metadata.py")


class ReleaseTests(unittest.TestCase):
    def test_platform_mapping(self):
        self.assertEqual(installer.target("Windows", "AMD64"), "x86_64-pc-windows-msvc")
        self.assertEqual(installer.target("Darwin", "arm64"), "aarch64-apple-darwin")
        with self.assertRaises(ValueError): installer.target("Linux", "riscv64")

    def test_verified_install_collision_and_tamper_protection(self):
        for windows in [False, True]:
            buffer = io.BytesIO()
            binary = "ai.exe" if windows else "ai"
            if windows:
                with zipfile.ZipFile(buffer, "w") as archive: archive.writestr(binary, b"binary")
            else:
                with tarfile.open(fileobj=buffer, mode="w:gz") as archive:
                    member = tarfile.TarInfo(binary)
                    member.size = 6
                    archive.addfile(member, io.BytesIO(b"binary"))
            data = buffer.getvalue()
            name = "ai-test.zip" if windows else "ai-test.tar.gz"
            manifest = f"{hashlib.sha256(data).hexdigest()}  {name}\n"
            with tempfile.TemporaryDirectory() as directory:
                output = installer.install(data, manifest, name, directory)
                self.assertEqual(output.read_bytes(), b"binary")
                with self.assertRaises(ValueError): installer.install(data, manifest, name, directory)
                with self.assertRaises(ValueError): installer.install(data + b"tampered", manifest, name, directory, True)
                self.assertEqual(output.read_bytes(), b"binary")
                installer.install(data, manifest, name, directory, True)

    def test_formula_uses_actual_archive_hashes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for target in ["aarch64-apple-darwin", "x86_64-apple-darwin", "x86_64-unknown-linux-gnu"]:
                (root / f"ai-{target}.tar.gz").write_bytes(target.encode())
            (root / "ai-x86_64-pc-windows-msvc.zip").write_bytes(b"windows")
            (root / "install.sh").write_text("#!/bin/sh\n")
            metadata.generate(root, "v0.1.0")
            formula = (root / "ainline.rb").read_text()
            self.assertIn(hashlib.sha256(b"aarch64-apple-darwin").hexdigest(), formula)
            self.assertIn("install.sh", (root / "SHA256SUMS").read_text())
            for line in (root / "SHA256SUMS").read_text().splitlines():
                digest, name = line.split()
                self.assertEqual(digest, hashlib.sha256((root / name).read_bytes()).hexdigest())
            with self.assertRaises(ValueError): metadata.generate(root, 'v0.1.0"evil')


if __name__ == "__main__": unittest.main()
