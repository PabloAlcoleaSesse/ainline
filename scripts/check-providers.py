"""Report installed CLI contracts; --live explicitly sends one synthetic request each.

No project files or ai configuration are sent. Provider-owned hooks/settings still apply.
Run with --live only when the provider accounts are configured for use.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import sys

CONTRACTS = {
    "claude": ("claude", ["--print", "--model", "--output-format", "--tools", "--no-session-persistence", "--include-partial-messages", "--verbose"]),
    "copilot": ("copilot", ["--model", "--silent", "--stream", "--output-format", "--no-ask-user", "--no-auto-update", "--no-color", "--no-custom-instructions", "--disable-builtin-mcps", "--available-tools"]),
    "antigravity": ("agy", ["--model", "--mode", "--input-format", "--output-format", "--print-timeout"]),
    "openai": ("codex", ["--model", "--sandbox", "--ephemeral", "--skip-git-repo-check", "--color"]),
    "ollama": ("ollama", []),
}
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--strict", action="store_true", help="Fail on missing executables, version/help failures, or live failures")
parser.add_argument("--expected-version", help="Require this version in version output; select exactly one provider")
parser.add_argument("--live", action="store_true", help="Send synthetic model requests (provider usage charges may apply)")
parser.add_argument("--provider", choices=CONTRACTS, action="append")
parser.add_argument("--ai", default="target/release/ai.exe" if os.name == "nt" else "target/release/ai")
parser.add_argument("--timeout", type=int, default=20)
args = parser.parse_args()
if not 1 <= args.timeout <= 300:
    parser.error("--timeout must be between 1 and 300 seconds")
if args.expected_version and len(args.provider or []) != 1:
    parser.error("--expected-version requires exactly one --provider")
ai = str(Path(args.ai).resolve())
failed = False
if args.live and not Path(ai).is_file():
    parser.error("Build ai first: cargo build --locked --release")


def inspect(binary, flags, cwd):
    try:
        result = subprocess.run([binary, *flags], cwd=cwd, stdin=subprocess.DEVNULL,
                                capture_output=True, text=True, timeout=10)
        return result.returncode, result.stdout + result.stderr
    except subprocess.TimeoutExpired:
        return 124, "inspection timed out"


with tempfile.TemporaryDirectory(prefix="ainline-compat-") as tmp:
    env = dict(os.environ, AI_CONFIG_FILE=str(Path(tmp) / "config.yaml"))
    for provider in args.provider or CONTRACTS:
        name, flags = CONTRACTS[provider]
        binary = shutil.which(name)
        row = {"provider": provider, "installed": binary is not None,
               "live": "not requested" if not args.live else "not run"}
        if binary:
            code, version = inspect(binary, ["--version"], tmp)
            row.update(version=version.strip().splitlines()[0] if version.strip() else "unknown",
                       version_exit=code)
            help_args = ["exec", "--help"] if provider == "openai" else ["run", "--help"] if provider == "ollama" else ["--help"]
            code, help_text = inspect(binary, help_args, tmp)
            if provider == "openai":
                root_code, root_help = inspect(binary, ["--help"], tmp)
                flags = flags + ["--no-daemon", "--ask-for-approval"]
                help_text += root_help
                code = code or root_code
            row.update(help_exit=code, missing_flags=[flag for flag in flags if flag not in help_text])
            if args.expected_version:
                row["version_matches"] = args.expected_version in row["version"]
            if args.live:
                # ai's own deadline and process supervision bound the model invocation.
                try:
                    result = subprocess.run([ai, provider, "--no-context", "--timeout", str(args.timeout),
                                             "--max-output-bytes", "1048576", "Reply exactly OK."],
                                            cwd=tmp, env=env, input="", text=True, capture_output=True,
                                            timeout=args.timeout + 10)
                    row.update(live="passed" if result.returncode == 0 and result.stdout.strip() == "OK" else "failed",
                               live_exit=result.returncode, response=result.stdout[:200],
                               diagnostic=result.stderr[:1200])
                except subprocess.TimeoutExpired:
                    row.update(live="supervisor watchdog timeout")
        failed |= (not binary or row.get("version_exit", 1) != 0 or row.get("help_exit", 1) != 0
                   or bool(row.get("missing_flags")) or row.get("version_matches") is False
                   or (args.live and row.get("live") != "passed"))
        print(json.dumps(row), flush=True)
if args.strict and failed:
    sys.exit(1)
