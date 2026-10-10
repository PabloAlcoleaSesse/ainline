"""Measure warm end-to-end local CLI overhead, excluding real model latency."""
import argparse
import os
from pathlib import Path
import statistics
import subprocess
import tempfile
import time

parser = argparse.ArgumentParser()
parser.add_argument("binary", nargs="?", default="target/release/ai")
parser.add_argument("--runs", type=int, default=100)
args = parser.parse_args()
if args.runs < 1:
    parser.error("--runs must be positive")
binary = str(Path(args.binary).resolve())
with tempfile.TemporaryDirectory() as tmp:
    env = dict(os.environ, AI_CONFIG_FILE=str(Path(tmp) / "config.yaml"))
    subprocess.run(["git", "init", "--quiet", tmp], check=True)
    for name, command in [
        ("help", ["--help"]),
        ("inline preparation, no Git", ["claude", "--dry-run", "--no-git", "hello"]),
        ("inline preparation, with Git probes", ["claude", "--dry-run", "hello"]),
    ]:
        samples = []
        for i in range(args.runs + 5):
            start = time.perf_counter_ns()
            subprocess.run([binary, *command], cwd=tmp, env=env, stdin=subprocess.DEVNULL,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True)
            elapsed = (time.perf_counter_ns() - start) / 1e6
            if i >= 5:
                samples.append(elapsed)
        samples.sort()
        print(f"{name}: median={statistics.median(samples):.2f} ms "
              f"p95={samples[min(len(samples)-1, int(len(samples)*.95))]:.2f} ms")
