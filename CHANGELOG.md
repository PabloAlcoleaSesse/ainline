# Changelog

## Unreleased — 0.1.0 alpha

### Added

- `ai [provider] [model-code] [flags] <message>` with saved defaults.
- Claude, Copilot, Antigravity, OpenAI and Ollama adapters.
- Two-letter provider shortcuts and configurable model codes.
- `ai setup`, per-provider default models, configuration validation and model listing.
- `.ai/` project rules, local rules, 24-hour working notes, stack and Git context.
- stdin support, context preview, exit-code propagation and native argument forwarding.
- Automated tests, CI matrix, minimum Rust check and draft binary release workflow.
- MIT license, contribution guidance, security policy and issue templates.

### Fixed

- Preserve diagnostics written by successful providers.
- Reject attached and clustered short options that could override the request protocol.

### Known limitations

General timeouts, bounded/streamed provider output, process-tree cancellation,
and full live-provider compatibility verification are not complete. See ROADMAP.md.
