# Contributing

Small, focused contributions are welcome: provider compatibility fixes, parser
edge cases, platform coverage, and documentation. For large changes, open an
issue describing the problem and proposed behavior before implementing it.
See the [roadmap](ROADMAP.md) for current priorities and
[architecture](docs/architecture.md) for the code layout.

## Get started

```sh
git clone https://github.com/PabloAlcoleaSesse/ainline.git
cd ainline
rustup component add rustfmt clippy
cargo build --locked
cargo run -- cl hk4.5 --dry-run --no-context hello
```

The dry run requires no provider installation or credentials. Use a temporary
`AI_CONFIG_FILE` when trying configuration commands to protect your own defaults.

## Before a pull request

Use Rust 1.89 or newer. Before opening a PR run `cargo fmt --all --check`,
`cargo test --locked --all-targets`, and `cargo clippy --locked --all-targets -- -D warnings`.
Build release binaries before measuring startup; do not benchmark `cargo run`.

Keep provider adapters synchronous and independent of network SDKs. Pass prompts
through stdin and never invoke a shell. Preserve explicit provider overrides, configured defaults and
child exit codes; do not introduce implicit retries or fallback. Test adapters
with fake executables; tests must not require credentials or spend money.
Preserve atomic writes and locks for context updates. Never overwrite user config
or silently reset corrupt sessions. Commit Cargo.lock changes with dependency updates.

Describe behavior changes and checks run. Include OS and provider CLI versions
when reporting compatibility issues. Never include API keys, private rules or
session contents in issue reports.
