# Installation and removal

The executable is `ai`; the package and Homebrew formula are named `ainline`.
Provider CLIs must be installed and authenticated separately. No installer modifies
your shell configuration, provider credentials, profiles, tasks or project rules.

## From source (available now)

```sh
git clone https://github.com/PabloAlcoleaSesse/ainline.git
cd ainline
cargo install --path . --locked
```

Requires Rust 1.89+. Cargo normally installs into `~/.cargo/bin`.
Uninstall with `cargo uninstall ainline`.

## Homebrew from source

Once these changes are on the repository's main branch:

```sh
brew tap PabloAlcoleaSesse/ainline https://github.com/PabloAlcoleaSesse/ainline
brew install --HEAD PabloAlcoleaSesse/ainline/ainline
```

The tap includes a source formula and installs Bash, Zsh and Fish completions.
Update it with `brew upgrade --fetch-HEAD ainline`; remove it with
`brew uninstall ainline`. Optionally remove the tap using
`brew untap PabloAlcoleaSesse/ainline`.

## Published release binaries

The release workflow builds Linux x86-64 (glibc), macOS ARM64/x86-64 and Windows
x86-64 archives. A tag matching the Cargo version runs CI and publishes a **stable**
release with archives, `SHA256SUMS`, `install.sh`, `install.py`, and a generated binary Homebrew
formula (`ainline.rb`). All platform checks and builds must pass before publication.
This implementation does not itself publish a release.

On macOS/Linux, download and review the shell installer from a published release:

```sh
curl -fL --proto '=https' --proto-redir '=https' -o install.sh \
  https://github.com/PabloAlcoleaSesse/ainline/releases/download/v1.0.0/install.sh
sh install.sh v1.0.0
# Custom location or explicit replacement:
sh install.sh v1.0.0 --bin-dir "$HOME/.local/bin"
sh install.sh v1.0.0 --force
```

No Python or Rust is required. The script uses `curl`, `tar`, standard Unix tools,
and `sha256sum` or `shasum`. It supports macOS ARM64/Intel and Linux x86-64 (glibc),
uses HTTPS downloads with timeouts, verifies the archive, and installs only its
regular `ai` executable. It requires no sudo and does not edit shell startup files.
The command requires a published release; it will fail while v1.0.0 is unpublished.

Windows retains the Python installer. After downloading and reviewing `install.py`
from a published release:

```sh
python3 install.py v1.0.0
# Custom location; requires an existing published tag:
python3 install.py v1.0.0 --bin-dir /your/bin
# Explicit replacement for upgrades:
python3 install.py v1.0.1 --force
```

On Windows use `py install.py v1.0.0`. The default directory is `.local/bin` in your
home directory; add it to PATH yourself. The installer selects your platform,
downloads the explicit tag's archive and checksum manifest over HTTPS, verifies
SHA-256, then installs only `ai`/`ai.exe`. It refuses existing destinations unless
`--force` is supplied and always refuses symlink destinations. A checksum verifies
integrity against the release manifest; it is not an independent publisher signature.
Unsupported architectures should use Cargo.

To install manually, download your archive and `SHA256SUMS`, verify that archive's
hash, then unpack and copy its executable to a directory on PATH. On Linux use
`sha256sum`, on macOS `shasum -a 256`, or on Windows
`Get-FileHash .\ai-x86_64-pc-windows-msvc.zip -Algorithm SHA256` and compare with
the matching manifest line. Do not run an archive whose hash differs.

For binary Homebrew installation, maintainers can replace `Formula/ainline.rb` in
the tap with the generated release formula **after publishing the release**. Users
can then `brew update` and `brew install PabloAlcoleaSesse/ainline/ainline` without
`--HEAD`. The generated formula includes platform-specific archive checksums.

## Remove a downloaded binary

Delete only the installed executable, using the directory selected at installation:

```sh
rm -- "$HOME/.local/bin/ai"
```

Windows PowerShell: `Remove-Item "$HOME/.local/bin/ai.exe"`.
Remove any `ai2`, `ai3`, or `ai-reviewer` shortcuts you explicitly installed too;
on Windows these are `.cmd` files. Do not use a wildcard that might match unrelated
commands. `ai profile remove NAME` removes its saved settings, not its launcher.

Uninstalling preserves your data. `ai config path` identifies the configuration
before removal; `tasks.yaml` lives beside it. Project rules and notes remain under
individual projects' `.ai/` directories. Delete those only if you also want to
remove your saved instructions/context.

## Maintainer checks

```sh
python3 -m unittest discover -s scripts/tests -v
python3 scripts/release-metadata.py v1.0.0 --directory dist
```

Metadata generation requires all three Unix archives and the Windows ZIP. Release CI checks that the
tag exactly matches `Cargo.toml`, verifies all generated checksums, and attaches
assets to a stable release. Native Windows installer tests run in the main CI matrix.
Formula conventions follow the [Homebrew Formula Cookbook](https://docs.brew.sh/Formula-Cookbook),
and release publication uses [GitHub CLI release creation](https://cli.github.com/manual/gh_release_create).
