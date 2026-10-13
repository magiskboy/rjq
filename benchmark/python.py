#!/usr/bin/env python3
"""Benchmark rjson_load (bytes) vs rjson_loads (str). Prints a table to stdout.

  uv run python benchmark/python.py
  uv run python benchmark/python.py --input path/to/data.json --runs 7
"""

from __future__ import annotations

import argparse
import json
import statistics
import sys
import time
from pathlib import Path

FIXTURES = Path(__file__).resolve().parent / "fixtures"


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


def bench(fn, data, *, warmup: int, runs: int) -> list[float]:
    for _ in range(warmup):
        fn(data)
    walls: list[float] = []
    for _ in range(runs):
        t0 = time.perf_counter()
        fn(data)
        walls.append(time.perf_counter() - t0)
    return walls


def main() -> int:
    p = argparse.ArgumentParser(description="Python rjson_load vs rjson_loads")
    p.add_argument("--input", type=Path, help="JSON file (default: generate fixture)")
    p.add_argument("--size-mb", type=float, default=5.0, help="fixture size if no --input")
    p.add_argument("--warmup", type=int, default=1)
    p.add_argument("--runs", type=int, default=5)
    args = p.parse_args()

    if args.runs < 1:
        sys.exit("--runs must be >= 1")

    try:
        from rjson import rjson_load, rjson_loads
    except ImportError:
        sys.exit("rjson not installed (e.g. uv sync / maturin develop --features python)")

    fixture = resolve_input(args.input, args.size_mb)
    raw = fixture.read_bytes()
    text = raw.decode("utf-8")
    size_mib = len(raw) / (1024 * 1024)

    print(f"input : {fixture} ({size_mib:.2f} MiB)", file=sys.stderr)

    load_walls = bench(rjson_load, raw, warmup=args.warmup, runs=args.runs)
    loads_walls = bench(rjson_loads, text, warmup=args.warmup, runs=args.runs)

    rows = [
        ("rjson_load (bytes)", load_walls),
        ("rjson_loads (str)", loads_walls),
    ]

    print()
    print(f"{'method':<22} {'median ms':>10} {'mean ms':>10} {'±stdev':>10} {'MB/s':>10}")
    print("-" * 66)
    for name, walls in rows:
        med = statistics.median(walls)
        mean = statistics.mean(walls)
        stdev = statistics.stdev(walls) if len(walls) >= 2 else 0.0
        mbps = size_mib / med if med > 0 else float("nan")
        print(
            f"{name:<22} {med * 1000:10.2f} {mean * 1000:10.2f} "
            f"{stdev * 1000:10.2f} {mbps:10.2f}"
        )

    a, b = statistics.median(load_walls), statistics.median(loads_walls)
    print()
    if a <= b:
        print(f"relative: rjson_load {b / a:.2f}x faster than rjson_loads")
    else:
        print(f"relative: rjson_loads {a / b:.2f}x faster than rjson_load")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
