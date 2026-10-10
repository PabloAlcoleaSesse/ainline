# Usage reference

## Request flags

Put `ai` options after the provider (and optional model code) and before the first message word. If you omit the provider, put options first.

| Flag | Behavior |
| --- | --- |
| `--model MODEL` | Override with a full model ID or a configured model code. |
| `--no-context` | Exclude rules, notes, stack detection, and Git summary. |
| `--no-git` | Keep context but skip Git subprocesses. |
| `--dry-run` | Print the assembled prompt without running a model. |
| `--provider-arg=ARG` | Forward one native argument; repeat for separate flags and values. |
| `--help` | Show request help. |

```sh
ai claude --provider-arg=--effort --provider-arg=high review this design
ai copilot --no-context "What does this expression mean?"
ai ollama --dry-run --no-git explain the project
ai claude -- --help is the text I want explained
ai claude explain --help
```

Everything after the first message word is literal message text. Use `--` when
the message starts with a hyphen. Unknown `ai` flags fail explicitly rather than
silently becoming message text. For native options, use the escape hatch and pass
each argument separately: `--provider-arg="value containing spaces"` is one argv
element. No shell is used.

`ai` owns the prompt transport, model selection, response format, and fresh-session
behavior. Do not override these with native arguments; common conflicting flags
such as `--prompt`, `--output-format`, `--model`, and `--resume` are rejected.
Other native options retain the provider's semantics, including permission flags.
The supported escape hatch is for additional options, not provider subcommands.

## Scripting and model-to-model calls

stdin is appended to the message under `Input:`. If there is no message, stdin
becomes the entire request. An empty message with empty stdin fails, except bare
`ai` with no input shows help. stdout holds
only response text; errors go to stderr. No provider selection banners are printed.
Responses are currently buffered until the child finishes; this is not streaming.

```sh
# Let another model review a generated draft.
ai claude draft a short migration plan | ai copilot critique this plan

# Capture a response. The chosen provider's failure propagates to the caller.
answer=$(ai ollama --no-context "Explain ownership in one sentence")

# Inspect exactly what project context would be sent, without invoking a model.
ai copilot --dry-run review the project
```

For shell pipelines that must detect failure in an earlier stage, enable the
shell's `pipefail` option. A calling agent or skill should choose a provider
explicitly, pass the message as an argv element or through stdin, and check the
exit code. There is no automatic retry that could unexpectedly invoke a second
model. Native provider arguments are passed directly, without shell expansion.

Exit codes: 0 success/help, 1 local/config/protocol error, 2 argument parsing error,
130 interruption. Other nonzero provider exit codes are preserved. Antigravity
cancellation events also map to 130 even when its process returns a generic error.
As with normal shell processes, some exit codes overlap; stderr supplies details.
A closed downstream stdout pipe exits cleanly.

## Project context

Context behavior is unchanged. Starting at the current directory, `ai` looks
upward for the nearest `.ai/` directory or `.git` marker (including worktree files).
Without a marker, it uses the current directory.

```sh
ai context init
ai context note "Migrating the login endpoint"
ai context rule style "Use typed errors"
ai context rule preferences --local "Respond in Spanish"
ai context show
ai context clear
```

```text
.ai/
  .gitignore
  session.json
  rules/
    style.md
    preferences.local.md
```

Notes have an individual 24-hour TTL. Expired notes are removed from disk on the
next access; no daemon runs. Rules are read in alphabetical order. Local rules,
sessions, locks, and temporary files are ignored by the generated `.ai/.gitignore`;
shared rules can be committed. Existing ignore entries are preserved. Rule
symlinks are skipped. Adding a rule with the same name replaces it; clearing the
session preserves rules.

Go, Rust, JavaScript/TypeScript and Python are detected from root manifests.
Git context includes the current branch and the stat of unstaged tracked changes;
it does not include untracked files, staged changes, or full source code. Supply
source code via stdin when needed. `--no-git` avoids both Git subprocesses.

The assembled context is sent to the selected provider. `--dry-run` may prune
expired notes, but never invokes a model. `--no-context` skips local context
collection; provider CLIs may still have their own instructions, tools or hooks.

## Configuration and aliases

Configuration is optional. Defaults work with all five provider names.

```sh
ai setup cl hk4.5     # save preferred provider/model
ai config init       # creates defaults only if the file does not exist
ai config path
ai config show
ai config validate
ai status
```

The default path is `~/.config/ai/config.yaml`, including on macOS and Windows.
`XDG_CONFIG_HOME` changes its base directory; `AI_CONFIG_FILE` selects a complete
file path. See [config.example.yaml](../config.example.yaml).

An alias pins a provider, executable and model:

```yaml
default_provider: local-review
models:
  local-review:
    provider: ollama
    binary: ollama
    model: qwen2.5-coder:7b
```

```sh
ai local-review review this function
ai local-review --model another-installed-model explain the project
```

Model codes can also be added or overridden per provider kind. Use full provider
names under `model_aliases`; values are exact native model IDs:

```yaml
model_aliases:
  ollama:
    qw2.5: qwen2.5-coder:14b
  claude:
    hk4.5: claude-haiku-4-5-20251001
```

Built-in mappings not mentioned in the file remain available. `ai config show`
displays the effective mappings. Existing configuration files need no migration.
An exact provider alias in `models` takes precedence over a shortcut, preserving
any pre-existing `cl`, `cp`, etc. aliases. Model-code lookup then uses the actual
provider kind selected by that entry.

Built-in names remain available when a config supplies only custom aliases. A
matching config entry overrides the built-in default. `binary` accepts an
executable name on PATH or an absolute path. Only the selected binary is resolved
during a request; `status` checks all configured entries. `local` remains an alias
for `ollama` unless explicitly configured.

### Migrating from automatic routing

Before: `ai --task code_review review this function`

Now: `ai claude review this function`

Existing model entries and aliases are retained. Old `priorities`, `daily_requests`,
`daily_budget_usd` and `estimated_cost_usd` fields are ignored on load and omitted
from `config show`. Existing config files are not rewritten. Old `usage.json`
files are no longer read or updated and can be removed manually. `.ai/` needs no
migration. `--task` has been removed.

## Provider adapters

- **Claude Code:** `claude --print`, configured model, text output. Agent tools
  and provider session persistence are disabled by default.
  [CLI reference](https://code.claude.com/docs/en/cli-reference).
- **GitHub Copilot:** standalone `copilot`, prompt on stdin, silent text output.
  Agent tools, custom instructions, built-in MCPs and interactive questions are
  disabled by default. Authenticate with `copilot login`.
  [Programmatic usage](https://docs.github.com/en/copilot/how-tos/copilot-cli/automate-copilot-cli/run-cli-programmatically).
- **Antigravity:** `agy` CLI, one NDJSON user event on stdin, final `result`
  validation. Uses plan mode and a five-minute provider timeout. Plan mode is
  not a sandbox; the installed CLI's permissions, hooks and persistence apply.
  Authenticate in `agy`; inspect available model IDs with `agy models`.
  [Headless protocol](https://antigravity.google/docs/cli/headless/) ·
  [Execution modes](https://antigravity.google/docs/cli/modes/).
- **OpenAI:** current official `openai responses create`, JSON request body on
  stdin and extraction of assistant text from the response. This is not the
  legacy Python SDK CLI.
  [Official documentation](https://developers.openai.com/api/docs/libraries/openai-cli).
- **Ollama:** `ollama run MODEL`, prompt on stdin. Requires a running Ollama server
  and an installed model, for example `ollama pull qwen2.5-coder:7b`.
  [CLI reference](https://github.com/ollama/ollama/blob/main/docs/cli.mdx).

Credentials stay with the provider CLIs and their environment variables. Children
run in the original working directory. No SDK, asynchronous runtime or server is
embedded in `ai`. Add another adapter by implementing `Provider` and extending
`config::Kind`.

