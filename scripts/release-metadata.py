"""Generate checksums and a binary Homebrew formula from completed release archives."""
import argparse
import hashlib
from pathlib import Path
import re


def generate(directory, tag):
    if not re.fullmatch(r"v\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?", tag):
        raise ValueError("Invalid release tag")
    directory = Path(directory)
    archives = sorted([*directory.glob("ai-*.tar.gz"), *directory.glob("ai-*.zip")])
    hashes = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in archives}
    if "ai-x86_64-pc-windows-msvc.zip" not in hashes:
        raise ValueError("Missing Windows archive")
    lines = ['class Ainline < Formula', '  desc "AI providers with project context"', '  homepage "https://github.com/PabloAlcoleaSesse/ainline"', f'  version "{tag[1:]}"', '  license "MIT"']
    for os_name, targets in [("macos", [("arm", "aarch64-apple-darwin"), ("intel", "x86_64-apple-darwin")]), ("linux", [("intel", "x86_64-unknown-linux-gnu")])]:
        lines.append(f"  on_{os_name} do")
        for arch, target in targets:
            name = f"ai-{target}.tar.gz"
            digest = hashes[name]
            lines += [f"    on_{arch} do", f'      url "https://github.com/PabloAlcoleaSesse/ainline/releases/download/{tag}/{name}"', f'      sha256 "{digest}"', '    end']
        lines.append('  end')
    lines += ['  def install', '    bin.install "ai"', '    generate_completions_from_executable(bin/"ai", "completions", shells: [:bash, :zsh, :fish])', '  end', '  test do', '    assert_equal "hello\\n", shell_output("#{bin}/ai --dry-run --no-context hello")', '  end', 'end']
    formula = directory / "ainline.rb"
    formula.write_text("\n".join(lines) + "\n")
    hashes[formula.name] = hashlib.sha256(formula.read_bytes()).hexdigest()
    for name in ["install.py", "install.sh"]:
        path = directory / name
        if path.exists(): hashes[name] = hashlib.sha256(path.read_bytes()).hexdigest()
    (directory / "SHA256SUMS").write_text("".join(f"{digest}  {name}\n" for name, digest in sorted(hashes.items())))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tag")
    parser.add_argument("--directory", default="dist")
    args = parser.parse_args()
    generate(args.directory, args.tag)
