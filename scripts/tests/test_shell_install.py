"""Exercise the real shell installer with local download fixtures, never the network."""
import hashlib
import io
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest

SCRIPT = Path(__file__).parents[1] / "install.sh"


@unittest.skipIf(os.name == "nt", "POSIX installer; Windows uses install.py")
class ShellInstallerTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.mock = self.root / "tools"
        self.mock.mkdir()
        self.releases = self.root / "release"
        self.releases.mkdir()
        self.destination = self.root / "bin with spaces"
        self.env = dict(os.environ, PATH=f"{self.mock}{os.pathsep}{os.environ['PATH']}",
                        FIXTURE_RELEASE=str(self.releases), FIXTURE_OS="Darwin", FIXTURE_ARCH="arm64")
        self.tool("uname", '#!/bin/sh\ncase "$1" in -s) echo "$FIXTURE_OS";; -m) echo "$FIXTURE_ARCH";; esac\n')
        self.tool("curl", '''#!/bin/sh
set -eu
output=''
while [ "$#" -gt 0 ]; do
    case "$1" in
        --output) output=$2; shift 2;;
        --proto|--proto-redir|--connect-timeout|--max-time|--max-filesize) shift 2;;
        --*) shift;;
        *) url=$1; shift;;
    esac
done
cp "$FIXTURE_RELEASE/${url##*/}" "$output"
''')

    def tool(self, name, text):
        path = self.mock / name
        path.write_text(text)
        path.chmod(0o755)

    def archive(self, target="aarch64-apple-darwin", symlink=False):
        name = f"ai-{target}.tar.gz"
        path = self.releases / name
        with tarfile.open(path, "w:gz") as archive:
            member = tarfile.TarInfo("ai")
            if symlink:
                member.type = tarfile.SYMTYPE
                member.linkname = "/tmp/not-the-binary"
                archive.addfile(member)
            else:
                data = b"#!/bin/sh\necho 'ai 1.0.0'\n"
                member.size = len(data)
                archive.addfile(member, io.BytesIO(data))
        manifest = f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {name}\n"
        (self.releases / "SHA256SUMS").write_text(manifest)
        return path

    def run_installer(self, *args):
        return subprocess.run(["sh", str(SCRIPT), "v1.0.0", "--bin-dir", str(self.destination), *args],
                              env=self.env, capture_output=True, text=True, timeout=10)

    def test_platforms_install_and_overwrite_protection(self):
        for system, arch, target in [("Darwin", "arm64", "aarch64-apple-darwin"),
                                     ("Darwin", "x86_64", "x86_64-apple-darwin"),
                                     ("Linux", "x86_64", "x86_64-unknown-linux-gnu")]:
            with self.subTest(target=target):
                self.env.update(FIXTURE_OS=system, FIXTURE_ARCH=arch)
                self.archive(target)
                result = self.run_installer()
                self.assertEqual(result.returncode, 0, result.stderr)
                binary = self.destination / "ai"
                self.assertEqual(subprocess.check_output([str(binary)], text=True).strip(), "ai 1.0.0")
                self.assertNotEqual(self.run_installer().returncode, 0)
                self.assertEqual(self.run_installer("--force").returncode, 0)
                self.assertEqual(list(self.destination.glob(".ainline-install.*")), [])
                binary.unlink()

    def test_checksum_failure_preserves_existing_binary(self):
        archive = self.archive()
        self.destination.mkdir()
        binary = self.destination / "ai"
        binary.write_text("original")
        archive.write_bytes(archive.read_bytes() + b"tampered")
        result = self.run_installer("--force")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("SHA-256 mismatch", result.stderr)
        self.assertEqual(binary.read_text(), "original")

    def test_symlinks_duplicate_checksums_and_unsupported_platform_fail(self):
        self.archive(symlink=True)
        self.assertNotEqual(self.run_installer().returncode, 0)
        self.assertFalse((self.destination / "ai").exists())
        self.archive()
        manifest = self.releases / "SHA256SUMS"
        manifest.write_text(manifest.read_text() * 2)
        self.assertNotEqual(self.run_installer().returncode, 0)
        self.archive()
        self.destination.mkdir(exist_ok=True)
        original = self.root / "original"
        original.write_text("untouched")
        (self.destination / "ai").symlink_to(original)
        self.assertNotEqual(self.run_installer("--force").returncode, 0)
        self.assertEqual(original.read_text(), "untouched")
        self.env.update(FIXTURE_OS="Linux", FIXTURE_ARCH="aarch64")
        self.assertNotEqual(self.run_installer().returncode, 0)

    def test_invalid_arguments_and_download_failure(self):
        self.assertNotEqual(self.run_installer("--unknown").returncode, 0)
        self.assertNotEqual(self.run_installer().returncode, 0)
        result = subprocess.run(["sh", str(SCRIPT), "v1.0.0/evil"], env=self.env, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
