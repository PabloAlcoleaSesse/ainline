#!/bin/sh
# Install an explicit ainline release. No Python, Rust, sudo or shell-profile edits.
set -eu

fail() { printf 'Install failed: %s\n' "$*" >&2; exit 1; }
usage() {
    printf '%s\n' 'Usage: sh install.sh VERSION [--bin-dir DIR] [--force]' \
        'Example: sh install.sh v1.0.0' \
        'Supports macOS ARM64/Intel and Linux x86-64 (glibc).' \
        'Default directory: $HOME/.local/bin. --force replaces an existing regular binary.'
}
version=''
bin_dir=${HOME:?HOME must be set}/.local/bin
force=false
while [ "$#" -gt 0 ]; do
    case "$1" in
        -h|--help) usage; exit 0 ;;
        --bin-dir) [ "$#" -ge 2 ] || fail '--bin-dir requires a directory'; bin_dir=$2; shift 2 ;;
        --force) force=true; shift ;;
        -*) fail "Unknown option: $1" ;;
        *) [ -z "$version" ] || fail 'Specify only one version'; version=$1; shift ;;
    esac
done
[ -n "$version" ] || { usage >&2; exit 1; }
printf '%s\n' "$version" | LC_ALL=C grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.-]+)?$' || fail 'Expected a version such as v1.0.0'
[ -n "$bin_dir" ] || fail 'Installation directory cannot be empty'
for utility in curl tar mktemp awk chmod ln mv mkdir uname; do
    command -v "$utility" >/dev/null 2>&1 || fail "Required command not found: $utility"
done
case "$(uname -s):$(uname -m)" in
    Darwin:arm64|Darwin:aarch64) target=aarch64-apple-darwin ;;
    Darwin:x86_64) target=x86_64-apple-darwin ;;
    Linux:x86_64) target=x86_64-unknown-linux-gnu ;;
    *) fail 'Unsupported platform; use Cargo or download a compatible binary manually' ;;
esac
if command -v sha256sum >/dev/null 2>&1; then
    checksum=sha256sum
elif command -v shasum >/dev/null 2>&1; then
    checksum=shasum
else
    fail 'Install sha256sum or shasum to verify downloads'
fi
# Absolute paths avoid treating user-supplied relative paths as command options.
case "$bin_dir" in /*) ;; *) bin_dir=$PWD/$bin_dir ;; esac
output=$bin_dir/ai
check_destination() {
    [ ! -L "$output" ] || fail "Refusing symlink destination: $output"
    if [ -e "$output" ]; then
        [ -f "$output" ] || fail "Destination is not a regular file: $output"
        [ "$force" = true ] || fail "Destination exists: $output (use --force to replace)"
    fi
}
check_destination
work=$(mktemp -d "${TMPDIR:-/tmp}/ainline-install.XXXXXXXX")
staged=''
cleanup() {
    [ -z "$staged" ] || rm -f "$staged"
    rm -rf "$work"
}
trap cleanup 0
trap 'exit 130' INT
trap 'exit 143' TERM
archive=ai-$target.tar.gz
base=https://github.com/PabloAlcoleaSesse/ainline/releases/download/$version
fetch() {
    curl --fail --silent --show-error --location --proto '=https' --proto-redir '=https' \
        --connect-timeout 15 --max-time 120 --max-filesize "$3" --output "$2" "$1"
}
printf 'Downloading ainline %s for %s...\n' "$version" "$target"
fetch "$base/SHA256SUMS" "$work/SHA256SUMS" 1048576 || fail 'Could not download release checksums'
expected=$(awk -v name="$archive" '$2 == name || $2 == "*" name { print $1 }' "$work/SHA256SUMS")
[ "${#expected}" -eq 64 ] || fail 'Missing, duplicate or invalid archive checksum'
case "$expected" in *[!0-9a-fA-F]*) fail 'Invalid SHA-256 checksum' ;; esac
fetch "$base/$archive" "$work/$archive" 104857600 || fail 'Could not download release archive'
if [ "$checksum" = sha256sum ]; then
    actual=$(sha256sum "$work/$archive")
else
    actual=$(shasum -a 256 "$work/$archive")
fi
actual=${actual%% *}
[ "$actual" = "$expected" ] || fail 'Archive SHA-256 mismatch'
# Extract only the regular ai member to stdout, never paths from the archive.
tar -tvzf "$work/$archive" ai > "$work/member" || fail 'Archive has no ai executable'
awk 'substr($0,1,1) != "-" { bad=1 } END { exit (NR != 1 || bad) }' "$work/member" || fail 'Invalid executable archive member'
mkdir -p "$bin_dir"
staged=$(mktemp "$bin_dir/.ainline-install.XXXXXXXX")
# Bound extracted output using the shell file-size limit.
(ulimit -f 204800; tar -xOzf "$work/$archive" ai > "$staged") || fail 'Could not extract executable'
[ -s "$staged" ] || fail 'Archive executable is empty'
chmod 755 "$staged"
check_destination
if [ "$force" = true ]; then
    mv -f "$staged" "$output"
else
    ln "$staged" "$output" || fail 'Destination appeared during installation; refusing overwrite'
fi
printf 'Installed %s\nAdd %s to PATH if needed.\n' "$output" "$bin_dir"
