# Architecture

ainline is a synchronous Rust process wrapper. No model SDK or background daemon
is embedded. The normal request path invokes exactly one selected provider.

```text
argv + stdin
     │
     ├── administrative command → setup/profile/config/context/status/models
     │
     └── profile (optional) → provider override or saved default
                  │
           resolve model/code
                  │
           collect .ai context
                  │
           adapter builds argv + stdin
                  │
           selected child process
                  │
           decode output / preserve failure
```

| Module | Responsibility |
| --- | --- |
| `cli` | Administrative syntax, request flags, default/provider distinction |
| `aliases` | Explicit provider and model shorthand tables |
| `config` | YAML defaults, aliases, validation, legacy-field migration |
| `profiles` | Numbered/named defaults, agent instructions, atomic profile updates |
| `context` | Root discovery, rules, expiring notes, stack/Git metadata |
| `storage` | Bounded reads, atomic writes, OS advisory locks |
| `provider` | Child lifecycle, native argv, provider protocol adapters |
| `main` | Orchestration, stdin, stdout, diagnostics and exit codes |

Precedence is explicit: a recognized first-token provider overrides the saved
provider; a positional code or `--model` overrides that provider's saved model.
An exact configured provider alias wins over a built-in shortcut. A literal `--`
prevents interpreting message text as a provider/command.

Context is resolved from the original working directory; provider processes run
there too. Session expiry is lazy and per-note. Model mappings are local data, not
live availability checks. Native provider settings remain outside this wrapper.

A leading `--profile NAME`, `ai<N>`, or `ai-<name>` executable selects a profile.
Explicit providers bypass its provider/model defaults; explicit model flags or
codes override model defaults. Instructions are prepended as plain prompt text,
including when project context is disabled. Profiles share the project's rules
and session; they do not create isolated conversations or a new agent runtime.

The process supervisor uses bounded channels and separate pipe workers to stay
responsive under stdin/stdout/stderr backpressure. It enforces deadlines and a
combined raw-output budget; response delivery remains inside the deadline. A
stalled diagnostic consumer cannot indefinitely delay the final error exit.

Unix providers run in a dedicated process group. Windows providers start suspended,
are assigned to a kill-on-close Job Object, and resume only after assignment. RAII
cleanup terminates remaining group/job members and reaps the direct child. Signal
handlers only store an atomic cancellation flag. This is lifecycle management,
not a sandbox: deliberate Unix process-group escape is outside the guarantee.

Claude streaming uses newline-delimited JSON with partial messages; only text
deltas are delivered, and the final result is validated without printing it twice.
Copilot/Ollama stream native text. Codex final-answer text is bounded and buffered
until successful exit; Antigravity uses bounded buffering and protocol validation. Native failures preserve their exit codes, timeout returns
124, and cancellation returns 130. Streaming can produce partial output on failure.

Profiles can link an absolute UTF-8 instruction path, read each request under the
same file-size bound as other local input. Shortcut installation never overwrites
existing commands and never edits shell startup files. Status resolves profile
defaults and context sources without executing a provider.
