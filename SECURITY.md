# Security policy

Security fixes target the latest released 1.x version; there is no long-term support or guaranteed response-time commitment.

## Reporting a vulnerability

Use GitHub's **Report a vulnerability** action in the repository's Security tab:
https://github.com/PabloAlcoleaSesse/ainline/security/advisories/new

If private reporting is unavailable, contact the maintainer through the contact
options on their GitHub profile. Do not publish exploit details, tokens, private
prompts, or sensitive files in a public issue. Provide a minimal reproduction,
affected version, platform, impact, and a proposed fix if you have one.

## Trust boundaries

- Project rules, working notes, Git metadata and piped content are sent to the
  selected provider. Inspect with `--dry-run`; exclude context with `--no-context`.
- Provider executables, credentials, model access, local configuration, plugins
  and hooks are outside ainline's control. Run binaries and workspaces you trust.
- Prompts go through stdin and child argv is constructed without a shell. Native
  arguments retain provider semantics; `--provider-arg` is not a sandbox.
- Antigravity plan mode is not a security boundary. Provider permission settings
  may allow additional reads or operations beyond the assembled prompt.
- Input/output limits, deadlines, atomic state writes and child group/job cleanup
  are implemented. Detached Unix processes may escape a group; this is not a sandbox.
  Stronger context symlink defenses remain on the roadmap.
- Linked instruction files are read on each request. Only link files you trust;
  `--no-context` does not suppress explicit profile instructions.

Never commit `.env` files, credentials, `.ai/session.json`, or local rules. The
provided ignore rules help prevent accidents but are not a secret scanner.
