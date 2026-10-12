"""Native Rust JQL suite via bench-jql (parse once, query many)."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

from common import ROOT, BenchStats, ensure_fixture, fmt_bytes


def resolve_binary() -> Path:
    release = ROOT / "target" / "release" / "bench-jql"
    debug = ROOT / "target" / "debug" / "bench-jql"
    if release.is_file():
        return release
    if debug.is_file():
        print(
            "warning: using debug bench-jql "
            "(prefer: cargo build --release --bin bench-jql)",
            file=sys.stderr,
        )
        return debug
    raise SystemExit(
        "bench-jql not found. Build with:\n"
        "  cargo build --release --bin bench-jql"
    )


def run(
    *,
    input_path=None,
    size_mb: float = 5.0,
    force_generate: bool = False,
    warmup: int = 1,
    runs: int = 5,
) -> list[BenchStats]:
    path = ensure_fixture(input_path, size_mb, force_generate)
    input_bytes = path.stat().st_size
    print(f"input: {path} ({fmt_bytes(input_bytes)})", file=sys.stderr)

    binary = resolve_binary()
    cmd = [
        str(binary),
        "--input",
        str(path),
        "--warmup",
        str(warmup),
        "--runs",
        str(runs),
    ]
    print(f"    cmd: {' '.join(cmd)}", file=sys.stderr)
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.stderr:
        sys.stderr.write(proc.stderr)
    if proc.returncode != 0:
        raise SystemExit(f"bench-jql failed (exit {proc.returncode})")

    results: list[BenchStats] = []
    for line in proc.stdout.splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        row = json.loads(line)
        walls = [float(x) for x in row.get("walls") or []]
        results.append(
            BenchStats(
                name=row["name"],
                suite="rust",
                walls=walls,
                input_bytes=int(row.get("input_bytes") or input_bytes),
                errors=int(row.get("errors") or 0),
                notes=str(row.get("query") or ""),
            )
        )
    return results
