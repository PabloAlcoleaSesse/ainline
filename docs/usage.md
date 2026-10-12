# Usage reference

## Profiles and numbered commands

Profiles store provider/model defaults and optional agent instructions separately
from the ordinary `ai` defaults. Create any numbered or named profile:

```sh
ai profile set 2 cl hk4.5 --instructions "Review code and identify edge cases."
ai profile set 42 ol qw2.5
ai profile set reviewer cp hk4.5
ai --profile 2 explain this function
ai --profile reviewer --dry-run --no-git review this project
```

Put `--profile NAME` (or `--profile=NAME`) **before** the provider and request
flags. Names accept 1–64 ASCII letters, digits, hyphens, or underscores. An unknown
profile fails explicitly; it never silently uses a different provider.

Install a shortcut automatically after installing `ai`:

```sh
ai profile create 2 cl hk4.5 --install
ai profile create reviewer cp hk4.5 --install
ai2 review this endpoint
ai-reviewer status
```

`create` is an alias for `set`: it creates or updates a profile. Numbered profiles
install as `ai2`, `ai42`, etc.; names install as `ai-reviewer`. Shortcuts go beside
the running `ai` executable unless `--bin-dir DIR` is supplied. The directory must
be on PATH; the CLI reports when it isn't. Unix uses symlinks; Windows uses `.cmd`
launchers that forward to the original executable. Existing commands are never
overwritten, and identical installations are idempotent. An installation failure
leaves the saved profile intact. Moving `ai` requires recreating its shortcut.
Delete the shortcut separately when removing a profile.

Alternatively, create convenient bash/zsh commands:

```sh
alias ai2='ai --profile 2'
alias ai42='ai --profile 42'
ai2 review this endpoint
ai42 explain this error
ai2 setup cl sn4.5
```

Put these aliases in `~/.zshrc` or `~/.bashrc` to persist them. The CLI does not
edit shell startup files. Aliases are interactive conveniences; scripts and other
agents should use `ai --profile 2 ...` directly.

In PowerShell, add a function to your PowerShell profile:

```powershell
function ai2 { ai --profile 2 @args }
```

An executable launched as `ai2`, `ai42`, etc. (including `.exe`) also selects that
numbered profile. On Unix, a symlink to `ai` in a directory on PATH can provide
this without a shell alias. An explicit leading `--profile` overrides the number
in the executable name. Avoid copying binaries for shortcuts: copies do not
follow future updates.

Resolution rules:

- Plain messages use the profile provider, or the global provider if omitted.
- With no explicit provider, the profile model overrides that provider's default.
- An explicit provider uses that provider's own default model; a positional model
  code or `--model` overrides it. Profile instructions still apply.
- Project rules, notes and environment context are unchanged. Profile instructions
  are prepended even with `--no-context`, which skips only project context.

Instructions are plain prompt text sent to the selected model. They do not enable
tools, grant permissions, create separate sessions, or guarantee agent behavior.
Use `--dry-run` to inspect the assembled request. Use ordinary `ai` to omit a
profile's instructions.

Manage profiles without changing your global defaults:

```sh
ai profile list
ai profile set 2 --instructions "Focus on concurrency and correctness."
ai profile set 2 --instructions ""   # clear instructions
ai --profile 2 setup cl sn4.5         # also works as ai2 setup via the alias
ai profile remove 42
```

Updates preserve omitted fields. Changing the provider clears the old profile
model unless a new one is supplied. Removing a profile does not delete a shell
alias; remove that separately. `setup` and `status` use the active profile. `status` shows the effective provider,
resolved model, instruction source/text, context sources, and default limits without
calling a provider. It may prune expired notes. `config`, `models`, and `context`
retain their global/project meaning.

Profiles live in the existing global configuration file. Quote numeric YAML keys:

```yaml
profiles:
  '2':
    provider: cl
    model: hk4.5
    instructions: |
      Review code for correctness.
      Explain each finding and suggest a minimal fix.
  reviewer:
    instructions: Keep responses concise and actionable.
```

An instructions-only profile inherits the normal provider/model defaults.

### Instructions from a file

```sh
ai profile set 2 --instructions-file ./reviewer.md
ai2 --dry-run review this project
```

The file must exist, contain UTF-8 text, and be at most 1 MiB. Its canonical absolute
path is saved, and its contents are read on every request, so edits take effect
immediately. Missing/unreadable files fail explicitly. Inline instructions and a
file are mutually exclusive: setting one replaces the other. Use
`--instructions ""` to clear either source. In YAML use `instructions_file` with
an absolute path instead of `instructions`.

Request assembly is profile instructions, project context, then the user request
and piped input. These are text sections, not enforceable permission levels.
Project instructions may conflict with a profile; inspect them with `--dry-run`.
`--no-context` retains the selected profile instructions.

## Request flags

Put `ai` options after the provider (and optional model code) and before the first message word. If you omit the provider, put options first.

| Flag | Behavior |
| --- | --- |
| `--model MODEL` | Override with a full model ID or a configured model code. |
| `--no-context` | Exclude rules, notes, stack detection, and Git summary. |
| `--no-git` | Keep context but skip Git subprocesses. |
| `--dry-run` | Print the assembled prompt without running a model. |
| `--timeout SECONDS` | Provider/output deadline, default 300; range 1–86400. |
| `--max-output-bytes BYTES` | Combined raw provider stdout/stderr cap, default 16777216; max 1 GiB. |
| `--no-stream` | Buffer the response until provider success and protocol validation. |
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
Claude text deltas are decoded incrementally; Copilot is asked to stream text,
and Ollama text is forwarded as emitted. Codex final-answer text is buffered until
success; Antigravity remains buffered until its structured response validates. `--no-stream` buffers any provider.
Streamed requests can leave partial stdout even when they ultimately fail; always
check the exit code. Raw JSON events, tool events, and metadata are not written to
stdout. Provider diagnostics go to stderr as they arrive.

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
124 timeout, 130 interruption. Other nonzero provider exit codes are preserved. Antigravity
cancellation events also map to 130 even when its process returns a generic error.
As with normal shell processes, some exit codes overlap; stderr supplies details.
A closed downstream stdout pipe exits cleanly, after child cleanup.

Limits begin at provider execution and cover output delivery, including blocked
pipes; they do not cover initial stdin or project context collection. Output limits
count protocol bytes before decoding as well as stderr; diagnostics from `ai` itself
are additional. On cancellation, timeout, malformed streaming data, or output-limit
failure, the provider group/job is terminated. Ordinary child exit also cleans up
remaining descendants. Unix children that deliberately detach from the group are
outside this guarantee. Windows children are assigned to a kill-on-close Job Object
before their suspended primary thread is resumed. Windows console Ctrl+C/Ctrl+Break
are handled; other forced OS termination behavior depends on the platform.

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

- **Claude Code:** `claude --print`, configured model, decoded stream-JSON text deltas
  (native text output with `--no-stream`). Agent tools
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
- **OpenAI Codex:** `oa`, `openai`, and `codex` select the `codex` executable.
  Runs `codex exec` with stdin, ephemeral sessions, read-only sandboxing, no approval
  prompts, and no shared daemon. Codex configuration/authentication remain native.
  `model: default` omits `--model`, letting Codex select its configured model.
  Use `--model <ID>` or configured model aliases for an explicit choice. Its final
  answer is stdout; progress and diagnostics remain stderr. JSON and output-file
  overrides are rejected to preserve that contract. `--no-context` only disables
  ainline context, not Codex's own project instructions and configuration.
  [Noninteractive reference](https://learn.chatgpt.com/docs/non-interactive-mode).
- **Ollama:** `ollama run MODEL`, prompt on stdin. Requires a running Ollama server
  and an installed model, for example `ollama pull qwen2.5-coder:7b`.
  [CLI reference](https://github.com/ollama/ollama/blob/main/docs/cli.mdx).

Credentials stay with the provider CLIs and their environment variables. Children
run in the original working directory. No SDK, asynchronous runtime or server is
embedded in `ai`. Add another adapter by implementing `Provider` and extending
`config::Kind`.


### Migration from the former OpenAI API CLI adapter

The OpenAI entry now uses Codex. Loading an old `provider: openai` entry with
`binary: openai` changes its effective binary to `codex`; the former generated
`gpt-4.1-mini` default becomes `default`. This is an in-memory migration until a
configuration update saves it. Custom binary paths and alternative model IDs are
preserved. The built-in `gm4.1` mapping is removed; explicitly saved model aliases
remain intact and may need updating for models available in your Codex account.

```sh
ai setup oa default
ai oa explain this project
ai codex explain this project
ai profile create 2 oa default --install
```

Existing profiles with an API-only model can be reset with
`ai --profile NAME setup oa default`. Update custom executable paths to Codex
manually. Ollama remains separate under `ol` / `ollama`.

## Diagnostics

```sh
ai doctor                       # Current default provider
ai doctor oa                    # Codex executable and login status
ai --profile 2 doctor --json     # Machine-readable report
ai doctor cl --timeout 3         # Seconds per subprocess (default 5, maximum 30)
```

Doctor validates configuration, checks selected profile instructions, resolves the
provider/model, locates its executable, and runs `--version`. It uses native
`claude auth status` and `codex login status` authentication checks, or `ollama list`
for service reachability. Copilot and Antigravity authentication is reported as
unchecked. It does not submit prompts, verify model access or quota, or fix settings.
Native CLIs may access their own services or update their own caches.

Each subprocess has a deadline and a combined 64 KiB output limit. Raw probe output
and instruction text are omitted from reports. JSON contains `healthy` and a `checks`
array with `name`, `status` (`ok`, `warning`, `error`), and `message`. Exit status is 1
for failed checks, 0 for success (possibly with warnings), and 130 for interruption.
Thus `healthy: true` does not certify checks marked as warnings.

## Share a profile

```sh
ai profile export 2 --output reviewer.yaml
ai profile export 2                         # YAML on stdout
ai profile import reviewer.yaml --name reviewer
ai --profile reviewer review this module
ai profile import reviewer.yaml --name reviewer --replace
```

Version 1 exports contain only the provider kind, resolved model, and instruction
text. Inherited provider/model defaults are captured at export time; native model
selectors such as Codex's `default` retain their native meaning. Linked instruction
files are embedded as text. **Review that text before sharing: it is not scrubbed
for secrets.** Project rules, sessions, executable paths, credentials stored by the
provider, and other profiles are not exported.

Exports never overwrite an existing destination. Imports reject unknown fields,
unsupported versions, files larger than 1 MiB, and profile collisions unless
`--replace` is supplied. Imports use the recipient's canonical provider entry and
executable, preserve global defaults, and install no shortcuts. Conflicting local
provider kinds or model shorthands are rejected. An imported profile has inline
instructions and no dependency on the sender's filesystem.

## Reusable tasks

```sh
ai profile set reviewer cl hk4.5 --instructions 'Focus on correctness'
ai task set review --profile reviewer --instructions 'Review these changes' --arg=--diff
ai task review
ai task review --file src/main.rs focus on error handling
ai task review --model sonnet --timeout 60
ai --profile 2 task review
ai task review --dry-run
ai task review --explain
ai task list
ai task show review
ai task remove review
```

Tasks live in `tasks.yaml` beside your config (including an `AI_CONFIG_FILE`
override). `set` creates or replaces a task. A task saves instructions, an optional
profile name, and request arguments, without copying credentials or executing shell
commands. Instructions precede your additional message. A leading `--profile`
overrides the saved profile; otherwise an omitted task profile uses global defaults.
Single-value request flags supplied at invocation override saved values. Repeated
`--file` and `--provider-arg` values accumulate. Boolean saved flags remain enabled;
edit the task to clear them. Relative file paths resolve from the current directory.
Use flags before message text, as for ordinary requests. Tasks never guess providers
or fall back; select one with a profile. `set`, `list`, `show`, `remove` and `help`
are reserved task names. Task instructions and YAML are bounded by the same 1 MiB
storage limit as configuration. Deleting a referenced profile makes the task fail
with a clear error until it is updated.

## Explain the context

```sh
ai context explain
ai --profile reviewer context explain --json --no-git --file src/main.rs review
ai context explain --diff --staged review
cat error.log | ai context explain --json investigate
ai cl --explain --file src/main.rs review
```

Lists only included sources: profile instructions, stack/Git summary, sorted project
rules, unexpired notes, explicit files, diffs, message and piped input. Each entry's
size includes its framing; entries sum to the assembled prompt's exact UTF-8 byte
count. This is not a token estimate and excludes the newline printed by `--dry-run`.
Contents are not printed; use `--dry-run` to inspect them. Sources absent or excluded
by flags do not appear. Expired notes are ignored without rewriting the session
file during preview. No provider is invoked.

`--no-context` excludes automatic project context but retains profile instructions,
explicit attachments and diffs. `--no-git` disables Git collection and conflicts
with explicitly requested `--diff` or `--staged`. `--explain` on a normal request or
task uses the same assembly path as `ai context explain` and actual execution.

Explicit Git diffs are limited to 1 MiB and five seconds per Git process. A failed
requested diff produces an error instead of silently omitting it.
