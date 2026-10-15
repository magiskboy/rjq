#!/usr/bin/env python3
"""Compare rjq (eager + lazy) vs stock jq (CLI wall time). Prints a table and writes a plot.

  python benchmark/cli.py --rjq ./target/release/rjq --jq $(which jq)
  python benchmark/cli.py --rjq ./target/release/rjq --jq /opt/homebrew/bin/jq --plot out.png
"""

from __future__ import annotations

import argparse
import json
import statistics
import subprocess
import sys
import time
from pathlib import Path

FIXTURES = Path(__file__).resolve().parent / "fixtures"

# (case, rjq_script, jq_script)
CASES = [
    ("access", ".[0].profile.prefs.theme", ".[0].profile.prefs.theme"),
    (
        "filter",
        "filter(.active == true && .meta.level > 5 && .score > 50)",
        "map(select(.active == true and .meta.level > 5 and .score > 50))",
    ),
    (
        "map",
        "map({ id: .id, city: .meta.city, roles: .profile.roles, lat: .meta.geo.lat })",
        "map({ id: .id, city: .meta.city, roles: .profile.roles, lat: .meta.geo.lat })",
    ),
    (
        "filter_map",
        "filter(.active == true) | map({ name: .name, city: .meta.city, sku: .orders[0].sku })",
        "map(select(.active == true)) | map({ name: .name, city: .meta.city, sku: .orders[0].sku })",
    ),
    (
        "if_else",
        'map(if .score > 50 then "high" elif .score > 20 then "mid" else "low" end)',
        'map(if .score > 50 then "high" elif .score > 20 then "mid" else "low" end)',
    ),
]


def make_fixture(path: Path, size_mb: float) -> None:
    target = int(size_mb * 1024 * 1024)
    items: list[dict] = []
    i = 1
    cities = ("HN", "HCM", "DN", "HP", "CT")
    themes = ("dark", "light", "system")
    roles = ("user", "admin", "ops", "viewer")
    while True:
        for _ in range(200):
            items.append(
                {
                    "id": i,
                    "name": f"user-{i}",
                    "active": i % 2 == 0,
                    "score": (i * 17) % 10_000 / 100.0,
                    "tags": [f"t{j}" for j in range(i % 5)],
                    "meta": {
                        "city": f"city-{i % 50}",
                        "level": i % 10,
                        "geo": {
                            "lat": (i % 180) - 90 + (i % 100) / 100.0,
                            "lon": (i % 360) - 180 + (i % 100) / 100.0,
                        },
                        "note": "x" * (8 + (i % 24)),
                    },
                    "orders": [
                        {
                            "sku": f"{cities[i % 5]}-{(i + k) % 1000:03d}",
                            "qty": 1 + ((i + k) % 5),
                            "price": ((i * 13 + k * 7) % 5000) / 100.0,
                            "paid": (i + k) % 3 != 0,
                        }
                        for k in range(1 + (i % 3))
                    ],
                    "profile": {
                        "email": f"user-{i}@example.com",
                        "roles": [roles[(i + r) % 4] for r in range(1 + (i % 3))],
                        "prefs": {"theme": themes[i % 3], "notify": i % 3 == 0},
                    },
                    "metrics": [((i + m) * 3) % 100 / 10.0 for m in range(i % 4)],
                    "extra": None if i % 7 == 0 else {"ok": True, "n": i % 9},
                }
            )
            i += 1
        payload = json.dumps(items, separators=(",", ":"))
        if len(payload.encode()) >= target:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(payload, encoding="utf-8")
            return


def resolve_input(path: Path | None, size_mb: float) -> Path:
    if path is not None:
        path = path.expanduser().resolve()
        if not path.is_file():
            sys.exit(f"input not found: {path}")
        return path
    out = FIXTURES / f"fixture_records_v2_{size_mb:g}mb.json"
    target = int(size_mb * 1024 * 1024)
    if not out.exists() or out.stat().st_size < target:
        print(f"generating fixture ~{size_mb:g} MiB → {out}", file=sys.stderr)
        make_fixture(out, size_mb)
    return out


def run_once(cmd: list[str]) -> float:
    t0 = time.perf_counter()
    proc = subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    wall = time.perf_counter() - t0
    if proc.returncode != 0:
        err = (proc.stderr or b"").decode(errors="replace")
        sys.exit(f"command failed ({proc.returncode}): {' '.join(cmd)}\n{err}")
    return wall


def bench(cmd: list[str], *, warmup: int, runs: int) -> list[float]:
    for _ in range(warmup):
        run_once(cmd)
    return [run_once(cmd) for _ in range(runs)]


def write_plot(
    cases: list[str],
    rjq_ms: list[float],
    lazy_ms: list[float],
    jq_ms: list[float],
    out: Path,
    *,
    size_label: str,
) -> None:
    try:
        import matplotlib.pyplot as plt
        import numpy as np
    except ImportError:
        sys.exit("matplotlib required for --plot (e.g. uv run --with matplotlib ...)")

    x = np.arange(len(cases))
    width = 0.25
    fig, ax = plt.subplots(figsize=(11.5, 5.4), dpi=150)
    ax.bar(x - width, rjq_ms, width, label="rjq", color="#2A6F97", zorder=3)
    ax.bar(x, lazy_ms, width, label="rjq --parse lazy", color="#2A9D8F", zorder=3)
    ax.bar(x + width, jq_ms, width, label="jq", color="#E76F51", zorder=3)
    ax.set_ylabel("Median wall time (ms)")
    ax.set_xlabel("Query case")
    ax.set_title(f"CLI performance: rjq vs jq\n({size_label})")
    ax.set_xticks(x, cases)
    ax.legend(frameon=False, loc="upper left")
    ax.set_ylim(0, max(rjq_ms + lazy_ms + jq_ms) * 1.12)
    ax.spines["top"].set_visible(False)
    ax.spines["right"].set_visible(False)
    ax.grid(axis="y", linestyle="--", alpha=0.35, zorder=0)
    fig.tight_layout()
    out.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(out, bbox_inches="tight")
    plt.close(fig)


def main() -> int:
    p = argparse.ArgumentParser(description="CLI benchmark: rjq vs jq")
    p.add_argument("--rjq", required=True, type=Path, help="path to rjq binary")
    p.add_argument("--jq", required=True, type=Path, help="path to jq binary")
    p.add_argument("--input", type=Path, help="JSON file (default: generate fixture)")
    p.add_argument("--size-mb", type=float, default=5.0, help="fixture size if no --input")
    p.add_argument("--warmup", type=int, default=1)
    p.add_argument("--runs", type=int, default=5)
    p.add_argument(
        "--plot",
        type=Path,
        default=Path(__file__).resolve().parent / "plot.png",
        help="output plot path (default: benchmark/plot.png)",
    )
    p.add_argument("--no-plot", action="store_true", help="skip writing the plot")
    args = p.parse_args()

    if args.runs < 1:
        sys.exit("--runs must be >= 1")

    rjq = args.rjq.expanduser().resolve()
    jq = args.jq.expanduser().resolve()
    if not rjq.is_file():
        sys.exit(f"rjq not found: {rjq}")
    if not jq.is_file():
        sys.exit(f"jq not found: {jq}")

    fixture = resolve_input(args.input, args.size_mb)
    size_mib = fixture.stat().st_size / (1024 * 1024)
    print(f"input : {fixture} ({size_mib:.2f} MiB)", file=sys.stderr)
    print(f"rjq   : {rjq}", file=sys.stderr)
    print(f"jq    : {jq}", file=sys.stderr)

    rows: list[tuple[str, float, float, float]] = []  # case, rjq_ms, lazy_ms, jq_ms
    for case, rjq_script, jq_script in CASES:
        print(f"==> {case}", file=sys.stderr)
        base = [str(rjq), "--input", str(fixture), "--script", rjq_script, "--no-color"]
        rjq_walls = bench(base, warmup=args.warmup, runs=args.runs)
        lazy_walls = bench(
            [*base, "--parse", "lazy"],
            warmup=args.warmup,
            runs=args.runs,
        )
        jq_walls = bench(
            [str(jq), jq_script, str(fixture)],
            warmup=args.warmup,
            runs=args.runs,
        )
        rows.append(
            (
                case,
                statistics.median(rjq_walls) * 1000,
                statistics.median(lazy_walls) * 1000,
                statistics.median(jq_walls) * 1000,
            )
        )

    def fmt_rjq(ms: float, jq_ms: float) -> str:
        return f"{ms:.2f} ms (x{jq_ms / ms:.2f} jq)"

    print()
    print(f"{'case':<12} {'rjq eager':<22} {'rjq lazy':<22} {'jq ms':>10}")
    print("-" * 68)
    for case, eager_ms, lazy_ms, jq_ms in rows:
        print(
            f"{case:<12} {fmt_rjq(eager_ms, jq_ms):<22} {fmt_rjq(lazy_ms, jq_ms):<22} {jq_ms:10.2f}"
        )

    if not args.no_plot:
        write_plot(
            [c for c, *_ in rows],
            [r for _, r, _, _ in rows],
            [lz for _, _, lz, _ in rows],
            [j for _, _, _, j in rows],
            args.plot,
            size_label=f"~{size_mib:.0f} MiB nested records",
        )
        print(f"\nplot  : {args.plot}", file=sys.stderr)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
