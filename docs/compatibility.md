# Provider compatibility

Checked on **2026-10-10**, macOS ARM64, in a restricted development environment.
The installed CLI help matched the flags used by the four available adapters.
No successful live model response was verified here. These versions are observations,
not established minimum supported versions.

| Provider | Installed version | Local contract check | Live synthetic request |
| --- | --- | --- | --- |
| Claude Code | 2.1.224 | Help includes wrapper and streaming flags | Blocked: provider reports not logged in |
| GitHub Copilot CLI | 1.0.80 | Help includes wrapper flags and `--stream on` | Blocked: no authentication available |
| Google Antigravity CLI | 1.3.2 | Help includes model, plan mode and stream-JSON flags | Blocked: environment denies log writes and local-server binding |
| OpenAI Codex CLI | 0.162.0 | Help includes exec, sandbox, model and ephemeral flags | Blocked: environment denies in-process app-server initialization |
| Ollama | Not installed | Not checked locally | Not run |

Automated fixtures cover exact stdin/argv handling, provider errors, structured
response decoding, Claude stream framing, text delivery before completion, limits,
blocked pipes, descendant cleanup, and Unix interruption. They do not prove account
access, model availability, production networking, or live provider behavior.

## Repeat the checks

```sh
cargo build --locked --release
python3 scripts/check-providers.py

# Explicitly send a synthetic request using your configured provider account:
python3 scripts/check-providers.py --live --provider claude --timeout 20
```

The first command only inspects versions/help. `--live` sends `Reply exactly OK.`
in a temporary working directory with project context disabled and a separate empty
ai config path. Provider-owned configuration and hooks still apply. It uses normal
provider account access; usage charges can apply. Ollama may pull its configured
model if it is not already installed. Add `--provider` more than once to select a
subset; omitting it checks all five providers. Missing binaries are reported, never
installed automatically. Results are JSON lines; a successful response must be `OK`.

Before publishing results, review diagnostic text for private paths or account
information. Record the OS, CLI version, model, and whether a response completed.
Do not label a help-only check as a verified live integration.

## Protocol references

- Claude: [CLI flags](https://code.claude.com/docs/en/cli-reference) and
  [programmatic streaming](https://code.claude.com/docs/en/headless). Streaming
  uses partial-message JSON events, extracts text deltas, and validates the final
  result; buffered mode uses native text output.
- Copilot: [programmatic usage](https://docs.github.com/en/copilot/how-tos/copilot-cli/automate-copilot-cli/run-cli-programmatically).
  Piped input selects noninteractive execution; the wrapper requests streamed text.
- Antigravity: [headless protocol](https://antigravity.google/docs/cli/headless/).
  One user event is sent, then the final result is validated. Deltas remain buffered.
- OpenAI Codex: [noninteractive mode](https://learn.chatgpt.com/docs/non-interactive-mode).
  Uses `codex exec` with a read-only sandbox, native auth/model defaults, stdin
  input, and final-answer stdout. The API-oriented `openai` executable is not used.
- Ollama: [CLI reference](https://github.com/ollama/ollama/blob/main/docs/cli.mdx).
  The adapter runs the configured model and forwards its text output.

## Platform coverage

Local macOS tests verify process-group cleanup, timeouts, streaming, instruction
files, and installed symlinks. Linux and Windows tests are configured in CI but were
not run in this environment. Windows uses Job Objects and `.cmd` launchers; its
runtime behavior and console cancellation still require Windows validation.
A local Windows cross-check was blocked by unavailable offline dependencies.
Clippy is also unavailable in this environment; CI requires it before release.

## Contract gates

`tests/contracts.rs` compiles a native Rust fixture on every CI OS, including
Windows. All five adapters are exercised in streamed and buffered modes, checking
exact argv, literal Unicode stdin, structured decoding, stdout and nonzero exits.
These fixtures establish the wrapper's contract, not the behavior of installed CLIs.

The separate `Native provider contracts` workflow runs weekly and manually on Linux,
macOS and Windows. It installs pinned Claude 2.1.224, Copilot 1.0.80 and Codex 0.162.0,
then requires version/help checks to pass. These are compatibility targets derived
from the observed versions above, not verified minimums. The workflow has no account
secrets and never requests a model response. Installation failures are visible CI
failures, not silently skipped tests. Antigravity and Ollama remain fixture-covered;
their installed CLI checks must currently be run where those binaries are available.

```sh
python3 scripts/check-providers.py --provider claude --expected-version 2.1.224 --strict
python3 scripts/check-providers.py --provider antigravity --provider ollama --strict
```

`--strict` returns failure for absent binaries, incompatible help, version mismatch,
or a failed explicitly requested live probe. Codex checks both root and `exec` help.
The new workflow must run successfully before these pinned versions can be labeled
verified on a given platform. Update the pins deliberately when updating adapters.

Local follow-up on 2026-10-12: strict version/help checks passed for Claude 2.1.224,
Copilot 1.0.80, Codex 0.162.0 and the now-installed Antigravity 1.3.3. These were
help-only probes; no new live model calls or Windows runs were performed.
