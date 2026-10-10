<p align="center">
  <img src="assets/banner.svg" alt="ainline — Your models. Your terminal. Shared context." width="100%">
</p>

<p align="center">
  <a href="https://github.com/PabloAlcoleaSesse/ainline/actions/workflows/ci.yml"><img src="https://github.com/PabloAlcoleaSesse/ainline/actions/workflows/ci.yml/badge.svg" alt="CI status"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-c4f36d?labelColor=101719" alt="MIT license"></a>
  <img src="https://img.shields.io/badge/Rust-1.89%2B-c4f36d?labelColor=101719" alt="Rust 1.89 or newer">
  <img src="https://img.shields.io/badge/status-alpha-e9bf75?labelColor=101719" alt="Alpha software">
</p>

**`ai` adds shared project context to the model CLIs you already use.** Pick a
provider, use a short model code, or save your defaults and just ask.

```sh
ai setup cl hk4.5
ai explain this project
ai cp hk4.5 review this design
cat handler.go | ai oa find edge cases
```

Written in Rust. One selected provider per request. Your rules and working notes
follow you between models. Credentials stay with the provider CLIs.

[Quick start](#quick-start) · [Usage](docs/usage.md) · [Architecture](docs/architecture.md) · [Roadmap](ROADMAP.md) · [Contribute](CONTRIBUTING.md)

## Quick start

Install [Rust](https://rustup.rs/) 1.89+ and at least one supported provider CLI,
then authenticate that provider using its own instructions.

```sh
git clone https://github.com/PabloAlcoleaSesse/ainline.git
cd ainline
cargo install --path . --locked

ai status
ai setup cl hk4.5
ai what is this project about
```

The package is named **`ainline`**; the installed executable is **`ai`**. The package
is not published to crates.io. Binary releases are planned; source installation
is the supported path today. Uninstall with `cargo uninstall ainline`.

Before a real request, preview exactly what `ai` assembles:

```sh
ai cl hk4.5 --dry-run --no-git explain this project
```

## A small command surface

```text
ai [provider] [model-code] [flags] <message>
```

| What you want | Command |
| --- | --- |
| Use saved defaults | `ai explain this error` |
| Choose a provider | `ai cp review this design` |
| Choose provider + model | `ai cl hk4.5 explain this function` |
| Use a full model ID | `ai cl --model claude-haiku-4-5-20251001 explain this` |
| Set your defaults | `ai setup cl hk4.5` |
| Set another provider's model | `ai config model oa gm4.1` |
| List model shortcuts | `ai models cl` |
| Skip project context | `ai ol --no-context explain ownership` |
| Forward a native option | `ai cl --provider-arg=--effort --provider-arg=high review this` |

| Provider | Short name | Example model code |
| --- | --- | --- |
| Claude Code | `cl` | `hk4.5`, `sn4.5`, `op4.5` |
| GitHub Copilot CLI | `cp` | `hk4.5` |
| Google Antigravity CLI (`agy`) | `ag` | `gp3.1` |
| OpenAI CLI | `oa` | `gm4.1` |
| Ollama | `ol` | `qw2.5` |

Model codes are explicit, configurable mappings. They do not guarantee account
access or install models. Full provider names continue to work.

A recognized provider in the first argument selects it; otherwise the words are
the message for your default. Reserved administrative commands keep their usual
meaning. Use `ai -- ...` to make everything after `--` literal message text.
Flags go before the message. See the [full syntax and mappings](docs/usage.md).

## Context that stays with the project

```sh
ai context init
ai context rule style "Use typed errors and keep functions small"
ai context note "Migrating the login endpoint"
ai context rule preferences --local "Respond in Spanish"
```

```text
.ai/
├── .gitignore
├── session.json             # working notes, expires after 24 hours
└── rules/
    ├── style.md             # share with your team
    └── preferences.local.md # keep local
```

`ai` finds the nearest `.ai/` or `.git` marker, loads rules and live notes, detects
the stack, and adds a short Git summary. Expired notes are pruned on access.
Use `--no-git` to avoid Git subprocesses or `--no-context` to skip this collection.

Configuration lives in `~/.config/ai/config.yaml`; `AI_CONFIG_FILE` and
`XDG_CONFIG_HOME` are supported. Start with [config.example.yaml](config.example.yaml).

## Built for pipes and scripts

```sh
cat error.log | ai cp explain the failure
ai cl draft a migration plan | ai cp critique this plan
answer=$(ai ol --no-context "Explain ownership in one sentence")
```

stdout carries the response; stderr carries diagnostics. Provider failures keep
their exit codes. There is no implicit retry or fallback. Enable your shell's
`pipefail` when failure in an earlier pipeline stage should fail the pipeline.
Responses are currently buffered until completion.

## Status: usable alpha

This project is ready for experimentation and feedback. The current local suite
has **30 tests**, including simulated providers, configuration migrations, default
selection, shorthand parsing, context persistence, and provider exit handling.
Provider integration tests do not make paid requests.

Known limits matter before you rely on it unattended:

- Live compatibility and minimum versions for all five providers still need
  systematic verification. Provider flags and model availability can change.
- Provider output is buffered without a size cap. A general timeout and robust
  process-tree cancellation are planned; Antigravity has its own five-minute limit.
- Provider permissions, hooks, and local settings still apply. Antigravity's plan
  mode is not a sandbox, and native flags can alter provider behavior.
- The cross-platform workflow is configured; inspect its actual results before
  assuming a release works on every operating system.

A prior warm benchmark on macOS ARM64 measured **2.23 ms** median preparation with
`--no-git`, and **16.74 ms** with Git. These exclude model execution and predate the
latest defaults/alias work. Run `python3 scripts/benchmark.py` for your machine.
There is no blanket sub-15-ms performance guarantee.

## Development

```sh
cargo fmt --all --check
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release
```

Install missing lint tools with `rustup component add clippy rustfmt`.
See [CONTRIBUTING.md](CONTRIBUTING.md) for the development workflow and
[RELEASING.md](docs/RELEASING.md) for publication and release steps.

Useful contributions include provider compatibility reports, Windows testing,
process lifecycle improvements, and clearer docs. Read the [roadmap](ROADMAP.md),
open a focused issue, or send a small PR.

## License and community

[MIT](LICENSE). Contributions are welcome under the same license.
Please follow the [Code of Conduct](CODE_OF_CONDUCT.md). Report vulnerabilities
through the process in [SECURITY.md](SECURITY.md), without posting secrets publicly.

An independent community project; not affiliated with or endorsed by the provider
vendors. Provider subscriptions, terms, and usage charges apply separately.
# ainline
