#!/usr/bin/env python3
"""JQL query benchmarks for rs-jq.

Suites (same fixture by default under benchmark/fixtures/):

  rust       — in-process jql_execute (parse once): access + filter
  cli        — rjq vs stock jq (process wall time): access + filter
  all        — rust + cli
  fixture    — only generate the shared fixture

Cases (always both):
  access  — key/path access   rjq: .[0].meta.city
  filter  — array filter      rjq: filter(.active == true)
                              jq:  map(select(.active == true))

Prerequisites:
  cargo build --release --features cli --bin rjq --bin bench-jql
  # stock jq on PATH for the cli suite

Examples:
  python benchmark/run.py
  python benchmark/run.py --suite rust
  python benchmark/run.py --suite cli --runs 7
  python benchmark/run.py --size-mb 10
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path
from typing import Sequence

sys.path.insert(0, str(Path(__file__).resolve().parent))

from common import ROOT, ensure_fixture, print_report  # noqa: E402
import cli_jql  # noqa: E402
import rust_jql  # noqa: E402

CLI_ENGINES = ("rjq", "jq")


def parse_suites(value: str) -> list[str]:
    value = value.strip().lower()
    if value == "all":
        return ["rust", "cli"]
    parts = [p.strip() for p in value.split(",") if p.strip()]
    allowed = {"rust", "cli", "fixture"}
    if not parts:
        raise argparse.ArgumentTypeError("--suite must name at least one suite")
    bad = [p for p in parts if p not in allowed]
    if bad:
        raise argparse.ArgumentTypeError(
            f"unknown suite(s): {', '.join(bad)}; "
            "choose from all, rust, cli, fixture"
        )
    seen: set[str] = set()
    out: list[str] = []
    for p in parts:
        if p not in seen:
            seen.add(p)
            out.append(p)
    return out


def parse_engines(value: str) -> list[str]:
    value = value.strip().lower()
    if not value or value == "all":
        return list(CLI_ENGINES)
    parts = [p.strip() for p in value.split(",") if p.strip()]
    if not parts:
        raise argparse.ArgumentTypeError("--only must name at least one engine")
    bad = [p for p in parts if p not in CLI_ENGINES]
    if bad:
        raise argparse.ArgumentTypeError(
            f"unknown engine(s): {', '.join(bad)}; "
            f"choose from {', '.join(CLI_ENGINES)}"
        )
    return parts


def maybe_build(args: argparse.Namespace, suites: list[str]) -> None:
    if args.no_build:
        return
    features: list[str] = []
    bins: list[str] = []
    if "cli" in suites:
        features.append("cli")
        bins.append("rjq")
    if "rust" in suites:
        bins.append("bench-jql")
    if not bins:
        return
    cmd = ["cargo", "build", "--release"]
    if features:
        cmd.extend(["--features", ",".join(dict.fromkeys(features))])
    for b in dict.fromkeys(bins):
        cmd.extend(["--bin", b])
    print(f"building: {' '.join(cmd)}", file=sys.stderr)
    proc = subprocess.run(cmd, cwd=ROOT)
    if proc.returncode != 0:
        raise SystemExit(f"cargo build failed (exit {proc.returncode})")


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="JQL query benchmarks (access + filter).",
    )
    p.add_argument(
        "--suite",
        type=parse_suites,
        default=["rust", "cli"],
        metavar="SUITES",
        help="all | rust,cli,fixture (default: rust,cli)",
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
        type=parse_engines,
        default=list(CLI_ENGINES),
        metavar="ENGINES",
        help="cli suite engines: rjq,jq (default: both)",
    )
    p.add_argument("--rs-jq", dest="rs_jq", help="path to rjq binary")
    p.add_argument("--jq", dest="jq", help="path to stock jq binary")
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

    results = []

    if "rust" in suites:
        results.extend(
            rust_jql.run(
                input_path=fixture,
                size_mb=args.size_mb,
                force_generate=False,
                warmup=args.warmup,
                runs=args.runs,
            )
        )

    if "cli" in suites:
        results.extend(
            cli_jql.run(
                input_path=fixture,
                size_mb=args.size_mb,
                force_generate=False,
                warmup=args.warmup,
                runs=args.runs,
                engines=args.only,
                rs_jq=args.rs_jq,
                jq=args.jq,
            )
        )

    if results:
        case_notes = [
            f"  {c}: rjq `{rjq_q}`  |  jq `{jq_q}`"
            for c, (rjq_q, jq_q) in cli_jql.CASES.items()
        ]
        print_report(
            results,
            title="JQL QUERY BENCHMARK",
            extra_lines=[
                f"fixture  : {fixture}",
                f"suites   : {', '.join(suites)}",
                "cases    : access, filter",
                *case_notes,
            ],
            baseline_name="access",
        )

    if any(s.errors and not s.walls for s in results):
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
