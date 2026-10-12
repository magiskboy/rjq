#!/usr/bin/env python3
"""Unified JSON parse benchmarks for rs-jq.

Suites (same fixture by default under benchmark/fixtures/):

  rust       — rs_jq, serde_json          (native trees)
  python     — rjson, json, orjson, ujson (Python object trees)
  cli        — rs-jq, stock jq            (process wall time)
  all        — rust + python + cli
  fixture    — only generate the shared fixture
  criterion  — Criterion microbench (rs_jq vs serde_json)

Prerequisites:
  cargo build --release --features "cli,bench" --bin rjq --bin bench-json-parse
  pip install -r benchmark/requirements.txt
  maturin develop --release   # for rjson

Examples:
  python benchmark/run.py
  python benchmark/run.py --suite rust,python --size-mb 5
  python benchmark/run.py --suite python --only rjson,orjson --mode load
  python benchmark/run.py --input path/to/data.json --runs 7
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path
from typing import Sequence

sys.path.insert(0, str(Path(__file__).resolve().parent))

from common import ROOT, ensure_fixture, print_report  # noqa: E402
import cli_parse  # noqa: E402
import python_parse  # noqa: E402
import rust_parse  # noqa: E402

RUST_PARSERS = ("rs_jq", "serde_json")
PYTHON_PARSERS = ("rjson", "json", "orjson", "ujson")
CLI_PARSERS = ("rs-jq", "jq")


def parse_suites(value: str) -> list[str]:
    value = value.strip().lower()
    if value == "all":
        return ["rust", "python", "cli"]
    parts = [p.strip() for p in value.split(",") if p.strip()]
    allowed = {"rust", "python", "cli", "fixture", "criterion"}
    if not parts:
        raise argparse.ArgumentTypeError("--suite must name at least one suite")
    bad = [p for p in parts if p not in allowed]
    if bad:
        raise argparse.ArgumentTypeError(
            f"unknown suite(s): {', '.join(bad)}; "
            "choose from all, rust, python, cli, fixture, criterion"
        )
    seen: set[str] = set()
    out: list[str] = []
    for p in parts:
        if p not in seen:
            seen.add(p)
            out.append(p)
    return out


def parse_only(value: str) -> list[str] | None:
    value = value.strip().lower()
    if not value or value == "all":
        return None
    parts = [p.strip() for p in value.split(",") if p.strip()]
    if not parts:
        raise argparse.ArgumentTypeError("--only must name at least one parser")
    return parts


def filter_only(only: list[str] | None, allowed: tuple[str, ...]) -> list[str] | None:
    """Return parsers to run for a suite, or None to skip the suite."""
    if only is None:
        return list(allowed)
    selected = [p for p in only if p in allowed]
    return selected or None


def maybe_build(args: argparse.Namespace, suites: list[str]) -> None:
    if args.no_build:
        return
    features: list[str] = []
    bins: list[str] = []
    if "cli" in suites:
        features.append("cli")
        bins.append("rjq")
    if "rust" in suites:
        features.append("bench")
        bins.append("bench-json-parse")
    if not bins:
        return
    cmd = [
        "cargo",
        "build",
        "--release",
        "--features",
        ",".join(dict.fromkeys(features)),
    ]
    for b in dict.fromkeys(bins):
        cmd.extend(["--bin", b])
    print(f"building: {' '.join(cmd)}", file=sys.stderr)
    proc = subprocess.run(cmd, cwd=ROOT)
    if proc.returncode != 0:
        raise SystemExit(f"cargo build failed (exit {proc.returncode})")


def run_criterion(input_path: Path) -> None:
    env = os.environ.copy()
    env["BENCH_FIXTURE"] = str(input_path)
    cmd = ["cargo", "bench", "--bench", "json_parse"]
    print(f"running: BENCH_FIXTURE={input_path} {' '.join(cmd)}", file=sys.stderr)
    proc = subprocess.run(cmd, cwd=ROOT, env=env)
    if proc.returncode != 0:
        raise SystemExit(f"cargo bench failed (exit {proc.returncode})")


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="Unified JSON parse benchmarks (rs-jq vs peers).",
    )
    p.add_argument(
        "--suite",
        type=parse_suites,
        default=["rust", "python", "cli"],
        metavar="SUITES",
        help="all | rust,python,cli,fixture,criterion (default: rust,python,cli)",
    )
    p.add_argument("--input", type=Path, help="JSON file (default: generate fixture)")
    p.add_argument(
        "--size-mb",
        type=float,
        default=5.0,
        help="fixture size in MiB when --input is omitted (default: 5)",
    )
    p.add_argument(
        "--force-generate",
        action="store_true",
        help="regenerate fixture even if it already exists",
    )
    p.add_argument("--warmup", type=int, default=1, help="warmup runs (default: 1)")
    p.add_argument("--runs", type=int, default=5, help="timed runs (default: 5)")
    p.add_argument(
        "--only",
        type=parse_only,
        default=None,
        metavar="PARSERS",
        help="comma-separated parsers to keep within each suite (default: all)",
    )
    p.add_argument(
        "--mode",
        choices=("loads", "load", "both"),
        default="both",
        help="python suite: loads=str, load=bytes (default: both)",
    )
    p.add_argument("--script", default=".", help="cli suite: jq/jql script (default: '.')")
    p.add_argument("--rs-jq", dest="rs_jq", help="path to rs-jq binary")
    p.add_argument("--jq", dest="jq", help="path to stock jq binary")
    p.add_argument(
        "--verify",
        action="store_true",
        help="python suite: check object shapes once vs json.loads",
    )
    p.add_argument(
        "--no-build",
        action="store_true",
        help="do not auto cargo-build release binaries",
    )
    return p.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    if args.runs < 1:
        raise SystemExit("--runs must be >= 1")

    suites: list[str] = args.suite

    if suites == ["fixture"]:
        path = ensure_fixture(args.input, args.size_mb, args.force_generate)
        print(f"fixture ready: {path}")
        return 0

    maybe_build(args, suites)
    fixture = ensure_fixture(args.input, args.size_mb, args.force_generate)

    if "criterion" in suites:
        run_criterion(fixture)

    results = []
    only = args.only

    if "rust" in suites:
        sel = filter_only(only, RUST_PARSERS)
        if sel:
            results.extend(
                rust_parse.run(
                    input_path=fixture,
                    size_mb=args.size_mb,
                    force_generate=False,
                    warmup=args.warmup,
                    runs=args.runs,
                    only=sel,
                )
            )

    if "python" in suites:
        sel = filter_only(only, PYTHON_PARSERS)
        if sel:
            results.extend(
                python_parse.run(
                    input_path=fixture,
                    size_mb=args.size_mb,
                    force_generate=False,
                    warmup=args.warmup,
                    runs=args.runs,
                    mode=args.mode,
                    only=sel,
                    verify=args.verify,
                )
            )

    if "cli" in suites:
        sel = filter_only(only, CLI_PARSERS)
        if sel:
            results.extend(
                cli_parse.run(
                    input_path=fixture,
                    size_mb=args.size_mb,
                    force_generate=False,
                    warmup=args.warmup,
                    runs=args.runs,
                    script=args.script,
                    rs_jq=args.rs_jq,
                    jq=args.jq,
                    only=sel,
                )
            )

    if results:
        print_report(
            results,
            title="JSON PARSE BENCHMARK",
            extra_lines=[
                f"fixture  : {fixture}",
                f"suites   : {', '.join(s for s in suites if s != 'criterion')}",
            ],
            baseline_name="rs_jq",
        )

    if any(s.errors and not s.walls for s in results):
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
