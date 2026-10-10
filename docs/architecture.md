# Architecture

ainline is a synchronous Rust process wrapper. No model SDK or background daemon
is embedded. The normal request path invokes exactly one selected provider.

```text
argv + stdin
     │
     ├── administrative command → setup/config/context/status/models
     │
     └── provider override or saved default
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

The writer thread feeds stdin while the parent drains stdout and stderr through
`wait_with_output`, avoiding the basic pipe deadlock caused by sequential writing
and reading. The current output buffers are unbounded, and parent termination does
not yet provide comprehensive process-tree cleanup. These limitations are tracked
in ROADMAP.md.
