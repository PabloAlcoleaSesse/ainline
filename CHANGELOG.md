# Changelog

## 1.0.0

### Added

- Python-free macOS/Linux shell installer with checksum verification and overwrite protection.

- Reusable tasks with saved profiles, instructions and request flags.
- Shared prompt assembly and source/byte previews via `ai context explain` and `--explain`.
- Native cross-platform contract fixtures and pinned provider help-check workflow.
- Checksum-verifying release installer, Homebrew formulas and uninstall documentation.

- `ai doctor` with bounded native probes, JSON reports and actionable check results.
- Versioned profile export/import with embedded instructions and overwrite protection.

- One-command profile shortcut installation with collision protection.
- Linked instruction files and effective profile-aware status.
- Provider deadlines, output budgets, Unix group and Windows job cleanup.
- Claude delta streaming, Copilot/Ollama text forwarding, and buffered mode.
- Compatibility inspection and opt-in synthetic live probes.

- Numbered and named profiles with independent defaults and agent instructions,
  `--profile`, and `ai<N>` invocation support.
- `ai [provider] [model-code] [flags] <message>` with saved defaults.
- Claude, Copilot, Antigravity, OpenAI and Ollama adapters.
- Two-letter provider shortcuts and configurable model codes.
- `ai setup`, per-provider default models, configuration validation and model listing.
- `.ai/` project rules, local rules, 24-hour working notes, stack and Git context.
- stdin support, context preview, exit-code propagation and native argument forwarding.
- Automated tests, CI matrix, minimum Rust check and stable binary release workflow.
- MIT license, contribution guidance, security policy and issue templates.

### Fixed

- OpenAI provider now delegates to Codex CLI, with native model defaults, a `codex`
  alias, and migration of the former generated API CLI configuration.

- Preserve diagnostics written by successful providers.
- Reject attached and clustered short options that could override the request protocol.

### Known limitations

Full live-provider compatibility verification and Windows runtime validation are
not complete. OpenAI/Antigravity responses still use bounded buffering. Unix child
processes that deliberately detach can escape cleanup. See ROADMAP.md.
