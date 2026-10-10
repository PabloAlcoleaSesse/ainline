# Roadmap

The scope is a small, predictable CLI: choose a provider or use saved defaults,
add project context, and return the result. This is an alpha roadmap, not a delivery
schedule. Proposals should include an example workflow and its tradeoffs.

## Before a stable release

- General request timeout and reliable cleanup of child process trees.
- Bound provider output and support streaming where the provider protocol allows.
- Verify live adapters against documented minimum CLI versions and keep a
  compatibility matrix; extend real cancellation tests.
- Complete Linux, macOS, Windows and minimum-Rust validation in CI.
- Validate downloaded release archives and document platform installation paths.
- Tighten context symlink handling and concurrent-update coverage.

## Improve everyday use

- Shell completions and a compact model/provider discovery experience.
- More actionable diagnostics for authentication, missing models and incompatible CLIs.
- Reproducible startup benchmarks with representative project contexts.
- A concise integration guide for agent tools and model-to-model scripts.

## Already implemented

- Five provider adapters with explicit selection and configurable defaults.
- Provider and model shorthand, full model IDs and native-argument escape hatch.
- Project rules, local rules, expiring notes, stack detection and Git summaries.
- stdin pipelines, dry runs, model listing and native failure exit codes.
- Cross-platform CI configuration, release gating, checksums and draft prereleases.

Automatic task classification, hidden fallback and provider billing estimation
are outside the current scope.
