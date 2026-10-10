# Publishing and releases

## First publication

The intended public repository is `PabloAlcoleaSesse/ainline`. No release has been
published by preparing these files. Review the source, documentation, and license
before the first push; public Git history is visible to everyone.

Install Git, the GitHub CLI, and Rust, then run from the project directory:

```sh
gh auth login
rustup component add rustfmt clippy
bash scripts/publish.sh
```

Skip login when `gh auth status` already shows the correct working account.
The script requires the `PabloAlcoleaSesse` account, runs formatting, tests,
Clippy and a release build, initializes `main`, commits the explicit project
paths, creates a **public** repository and pushes it. The initial commit uses
the account's GitHub noreply email. It does not install `ai`, publish to crates.io,
or create a release tag. Existing staged changes or an unexpected origin stop it.

If a push fails after creation, fix authentication/connectivity and rerun. A
matching existing origin is accepted, without force-pushing. If GitHub already
has a repository by this name but there is no local origin, creation stops;
inspect that repository before connecting it manually.

After publication:

- Confirm the Linux, macOS, Windows and minimum Rust version CI jobs pass.
- Enable private vulnerability reporting in Settings → Security if available.
- Configure a ruleset for `main` requiring the CI checks before merging PRs.
- Review Discussions, the description, topics, and the README on GitHub.

## Alpha release

1. Check the roadmap, update `CHANGELOG.md`, and set the package version in
   `Cargo.toml`. Refresh `Cargo.lock` with `cargo check` and commit both.
2. Run the contribution checks and verify a clean working tree. Smoke-test real
   providers manually and record the tested CLI versions in the release notes.
   Automated tests use fixtures and cannot prove live compatibility.
3. Push the commit and wait for CI to pass. Tag that exact commit:

   ```sh
   git tag -a v0.1.0-alpha.1 -m 'ainline 0.1.0-alpha.1'
   git push origin v0.1.0-alpha.1
   ```

   Use the same version in the manifest and tag; the example requires setting
   the package version to `0.1.0-alpha.1` first.

4. The release workflow runs CI again, builds Linux x86-64, macOS ARM64/x86-64,
   and Windows x86-64 archives, then creates a **draft prerelease** with SHA-256
   checksums. Archives contain `ai` (or `ai.exe`), `LICENSE`, and `README.md`.
5. Download and test the archives on their target platforms, review checksums and
   notes, and explicitly publish the draft. Build success alone is insufficient.

The Linux binary targets the GitHub runner's glibc environment; compatibility
with older distributions is not guaranteed. Universal binaries, signed artifacts,
package-manager distribution, and crates.io publication are future work.

## Local packaging check

After building a target, use the same packaging command as CI:

```sh
cargo build --locked --release --target aarch64-apple-darwin
python3 scripts/package-release.py aarch64-apple-darwin ai
```

Choose your installed Rust target and use `ai.exe` for Windows. Output is written
to the ignored `dist/` directory. Never put credentials or project context there.
