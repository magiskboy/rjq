"""CLI JQL suite: rjq vs stock jq for access / filter / map cases."""

from __future__ import annotations

import platform
import re
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Sequence

from common import CASES, ROOT, BenchStats, ensure_fixture, fmt_bytes


@dataclass(frozen=True)
class RunResult:
    wall_s: float
    peak_rss_bytes: int | None
    exit_code: int
    stderr: str


def _parse_time_l(stderr: str) -> int | None:
    m = re.search(r"(\d+)\s+maximum resident set size", stderr)
    return int(m.group(1)) if m else None


def _parse_time_v(stderr: str) -> int | None:
    m = re.search(r"Maximum resident set size \(kbytes\):\s*(\d+)", stderr)
    return int(m.group(1)) * 1024 if m else None


def _time_wrapper() -> tuple[list[str], Callable[[str], int | None]] | None:
    time_bin = shutil.which("time")
    candidates = ["/usr/bin/time", time_bin]
    system = platform.system()
    for candidate in candidates:
        if not candidate or not Path(candidate).exists():
            continue
        if system == "Darwin":
            return [candidate, "-l"], _parse_time_l
        return [candidate, "-v"], _parse_time_v
    return None


def run_once(cmd: Sequence[str]) -> RunResult:
    wrapper = _time_wrapper()
    if wrapper is None:
        t0 = time.perf_counter()
        proc = subprocess.run(
            list(cmd),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
        )
        return RunResult(
            wall_s=time.perf_counter() - t0,
            peak_rss_bytes=None,
            exit_code=proc.returncode,
            stderr=proc.stderr or "",
        )

    prefix, parse_rss = wrapper
    t0 = time.perf_counter()
    proc = subprocess.run(
        prefix + list(cmd),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
    )
    return RunResult(
        wall_s=time.perf_counter() - t0,
        peak_rss_bytes=parse_rss(proc.stderr or ""),
        exit_code=proc.returncode,
        stderr=proc.stderr or "",
    )


def resolve_rs_jq(explicit: str | None) -> Path:
    if explicit:
        p = Path(explicit).expanduser().resolve()
        if not p.is_file():
            raise SystemExit(f"rjq binary not found: {p}")
        return p
    release = ROOT / "target" / "release" / "rjq"
    debug = ROOT / "target" / "debug" / "rjq"
    if release.is_file():
        return release
    if debug.is_file():
        print(
            "warning: using debug build (prefer: cargo build --features cli --release)",
            file=sys.stderr,
        )
        return debug
    raise SystemExit(
        "rjq binary not found. Build with:\n"
        "  cargo build --features cli --release"
    )


def resolve_stock_jq(explicit: str | None) -> Path:
    if explicit:
        p = Path(explicit).expanduser().resolve()
        if not p.is_file():
            raise SystemExit(f"jq binary not found: {p}")
        return p
    found = shutil.which("jq")
    if not found:
        raise SystemExit("stock jq not found on PATH (brew install jq)")
    return Path(found)


def _benchmark(
    name: str,
    cmd: list[str],
    *,
    input_bytes: int,
    warmup: int,
    runs: int,
    notes: str = "",
) -> BenchStats:
    print(f"\n==> cli / {name}", file=sys.stderr)
    print(f"    cmd: {' '.join(cmd)}", file=sys.stderr)

    for i in range(warmup):
        r = run_once(cmd)
        if r.exit_code != 0:
            print(
                f"    warmup {i + 1} failed (exit {r.exit_code}):\n{r.stderr}",
                file=sys.stderr,
            )
            break
        print(f"    warmup {i + 1}/{warmup}: {r.wall_s * 1000:.2f} ms", file=sys.stderr)

    walls: list[float] = []
    errors = 0
    rss_vals: list[int] = []
    for i in range(runs):
        r = run_once(cmd)
        if r.exit_code == 0:
            walls.append(r.wall_s)
            if r.peak_rss_bytes is not None:
                rss_vals.append(r.peak_rss_bytes)
            status = "ok"
        else:
            errors += 1
            status = f"FAIL({r.exit_code})"
            print(r.stderr, file=sys.stderr)
        rss = f", rss={fmt_bytes(r.peak_rss_bytes)}" if r.peak_rss_bytes else ""
        print(
            f"    run {i + 1}/{runs}: {r.wall_s * 1000:.2f} ms [{status}]{rss}",
            file=sys.stderr,
        )

    return BenchStats(
        name=name,
        suite="cli",
        walls=walls,
        input_bytes=input_bytes,
        errors=errors,
        peak_rss_bytes=max(rss_vals) if rss_vals else None,
        notes=notes,
    )


def run(
    *,
    input_path=None,
    size_mb: float = 5.0,
    force_generate: bool = False,
    warmup: int = 1,
    runs: int = 5,
    engines: Sequence[str] = ("rjq", "jq"),
    rs_jq: str | None = None,
    jq: str | None = None,
) -> list[BenchStats]:
    path = ensure_fixture(input_path, size_mb, force_generate)
    input_bytes = path.stat().st_size
    print(f"input: {path} ({fmt_bytes(input_bytes)})", file=sys.stderr)

    results: list[BenchStats] = []

    rjq_bin = resolve_rs_jq(rs_jq) if "rjq" in engines else None
    jq_bin = resolve_stock_jq(jq) if "jq" in engines else None

    for case in CASES:
        rjq_script, jq_script = CASES[case]
        if rjq_bin is not None:
            results.append(
                _benchmark(
                    f"rjq/{case}",
                    [str(rjq_bin), "--input", str(path), "--script", rjq_script],
                    input_bytes=input_bytes,
                    warmup=warmup,
                    runs=runs,
                    notes=rjq_script,
                )
            )
        if jq_bin is not None:
            results.append(
                _benchmark(
                    f"jq/{case}",
                    [str(jq_bin), jq_script, str(path)],
                    input_bytes=input_bytes,
                    warmup=warmup,
                    runs=runs,
                    notes=jq_script,
                )
            )

    return results
