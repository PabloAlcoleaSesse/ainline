# Publish the prepared 1.0 release

The working tree contains the complete 1.0 implementation, including new files.
Review `git status --short` and `git diff` before committing. Do not tag the old
commit: the tag must include all prepared source, scripts, tests and release files.

Local checks:

```sh
cargo fmt --all --check
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
python3 -m unittest discover -s scripts/tests -v
cargo build --locked --release
./target/release/ai --version
```

On a machine with GitHub access, after reviewing and committing the prepared files
(using `chore: prepare v1.0.0 release` as the commit subject):

```sh
git push origin main
```

Wait for the main branch CI to pass on Linux, macOS, Windows and minimum Rust 1.89.
Do not bypass a failing check. Then create a new tag (do not replace an existing tag):

```sh
git tag -a v1.0.0 -m 'ainline 1.0.0'
git push origin v1.0.0
```

The tag-triggered Release workflow repeats CI, builds four platform archives,
generates checksums and the binary Homebrew formula, and publishes a stable GitHub
release using `docs/releases/v1.0.0.md`. No manual `gh release create` is needed.
Do not create a separate release in advance; that would conflict with the workflow.
Crates.io publication is disabled.

After the workflow succeeds, verify all archives, `SHA256SUMS`, `install.sh`, `install.py`, and
`ainline.rb` appear at the release, and smoke-test the installer from that release.
Update `Formula/ainline.rb` from the generated release formula in a follow-up commit
to enable the binary Homebrew install; until then the tap supports `--HEAD` builds.

If CI fails before publication, fix the problem and use a new version/tag rather
than silently moving a release tag. Never label locally unrun Windows or live model
checks as verified. The release workflow's actual results are authoritative for
platform validation.
