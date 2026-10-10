"""Package a previously built target with a flat, installable archive layout."""
import argparse
from pathlib import Path
import tarfile
import zipfile

parser = argparse.ArgumentParser()
parser.add_argument("target")
parser.add_argument("binary", choices=["ai", "ai.exe"])
args = parser.parse_args()
files = [Path("target") / args.target / "release" / args.binary,
         Path("LICENSE"), Path("README.md")]
for path in files:
    if not path.is_file():
        parser.error(f"Missing release input: {path}")
output = Path("dist")
output.mkdir(exist_ok=True)
name = f"ai-{args.target}"
if args.binary.endswith(".exe"):
    with zipfile.ZipFile(output / f"{name}.zip", "w", zipfile.ZIP_DEFLATED) as archive:
        for path in files:
            archive.write(path, path.name)
else:
    with tarfile.open(output / f"{name}.tar.gz", "w:gz") as archive:
        for path in files:
            archive.add(path, arcname=path.name)
print(f"Packaged {name}")
