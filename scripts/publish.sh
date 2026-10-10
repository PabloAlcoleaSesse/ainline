#!/usr/bin/env bash
# Initial public publication. Run from your own terminal after reviewing the files.
set -euo pipefail
cd "$(dirname "$0")/.."
repo='PabloAlcoleaSesse/ainline'
owner=${repo%/*}

for tool in git gh cargo; do
  command -v "$tool" >/dev/null || { echo "Required tool missing: $tool" >&2; exit 1; }
done
gh auth status >/dev/null
login=$(gh api user --jq .login)
if [[ "$login" != "$owner" ]]; then
  echo "Switch GitHub CLI to $owner before publishing $repo." >&2
  exit 1
fi
email=$(gh api user --jq '"\(.id)+\(.login)@users.noreply.github.com"')

# Finish validation before creating anything on GitHub.
cargo fmt --all --check
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release

if git rev-parse --show-toplevel >/dev/null 2>&1; then
  root=$(git rev-parse --show-toplevel)
  [[ "$root" == "$(pwd -P)" ]] || { echo 'Refusing to publish a parent repository.' >&2; exit 1; }
  [[ "$(git symbolic-ref --short HEAD)" == main ]] || { echo 'Expected branch main.' >&2; exit 1; }
  git diff --cached --quiet || { echo 'Review and commit already staged changes first.' >&2; exit 1; }
else
  git init -b main
fi

# Only these reviewed project paths are included. Never stage local context or secrets.
git add Cargo.toml Cargo.lock src tests scripts docs assets .github \
  README.md CONTRIBUTING.md CODE_OF_CONDUCT.md SECURITY.md ROADMAP.md CHANGELOG.md \
  LICENSE config.example.yaml release.toml .gitignore .gitattributes .editorconfig
if ! git diff --cached --quiet; then
  git -c user.name="$login" -c user.email="$email" commit -m 'feat: launch ainline alpha'
fi

if git remote get-url origin >/dev/null 2>&1; then
  remote=$(git remote get-url origin)
  case "$remote" in
    "https://github.com/$repo"|"https://github.com/$repo.git"|"git@github.com:$repo.git") ;;
    *) echo "Refusing unexpected origin: $remote" >&2; exit 1 ;;
  esac
else
  gh repo create "$repo" --public --source . --remote origin \
    --description 'Your AI CLIs, one short command. Built in Rust, with shared project context.'
fi

visibility=$(gh repo view "$repo" --json isPrivate --jq .isPrivate)
[[ "$visibility" == false ]] || { echo 'Expected a public repository; stopping before push.' >&2; exit 1; }
git push -u origin main
gh repo edit "$repo" --enable-issues --enable-discussions --enable-wiki=false \
  --add-topic rust --add-topic cli --add-topic ai --add-topic developer-tools \
  --add-topic ollama --add-topic terminal || echo 'Published; repository topics/settings need a manual check.' >&2
echo "Published: https://github.com/$repo"
echo 'Check Actions before tagging a release. See docs/RELEASING.md.'
