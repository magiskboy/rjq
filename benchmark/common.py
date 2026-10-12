"""Shared helpers for JQL query benchmarks."""

from __future__ import annotations

import json
import platform
import statistics
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Sequence

ROOT = Path(__file__).resolve().parents[1]
BENCH_DIR = Path(__file__).resolve().parent
FIXTURES_DIR = BENCH_DIR / "fixtures"


def generate_json(target_bytes: int, seed: int = 1) -> str:
    """Build a compact JSON array of objects until size >= target_bytes."""
    items: list[dict[str, Any]] = []
    i = seed
    while True:
        batch = []
        for _ in range(200):
            batch.append(
                {
                    "id": i,
                    "name": f"user-{i}",
                    "active": i % 2 == 0,
                    "score": (i * 17) % 10_000 / 100.0,
                    "tags": [f"t{j}" for j in range(i % 5)],
                    "meta": {
                        "city": f"city-{i % 50}",
                        "level": i % 10,
                        "note": "x" * (8 + (i % 24)),
                    },
                }
            )
            i += 1
        items.extend(batch)
        payload = json.dumps(items, separators=(",", ":"))
        if len(payload.encode("utf-8")) >= target_bytes:
            return payload


def ensure_fixture(path: Path | None, size_mb: float, force: bool) -> Path:
    """Return an existing or newly generated fixture under benchmark/fixtures/."""
    if path is not None:
        resolved = path.expanduser().resolve()
        if not resolved.is_file():
            raise SystemExit(f"input not found: {resolved}")
        return resolved

    FIXTURES_DIR.mkdir(parents=True, exist_ok=True)
    out = FIXTURES_DIR / f"fixture_{size_mb:g}mb.json"
    target = int(size_mb * 1024 * 1024)
    if out.exists() and not force and out.stat().st_size >= target:
        return out
    print(f"generating fixture ~{size_mb:g} MiB → {out}", file=sys.stderr)
    out.write_text(generate_json(target), encoding="utf-8")
    return out


def fmt_bytes(n: int | float | None) -> str:
    if n is None:
        return "n/a"
    n = float(n)
    for unit, div in (("GiB", 1024**3), ("MiB", 1024**2), ("KiB", 1024), ("B", 1)):
        if n >= div or unit == "B":
            return f"{n / div:.2f} {unit}"
    return f"{n:.0f} B"


def fmt_num(n: float | None, suffix: str = "") -> str:
    if n is None or n != n:  # NaN
        return "n/a"
    if abs(n) >= 1_000_000:
        return f"{n / 1_000_000:.2f}M{suffix}"
    if abs(n) >= 1_000:
        return f"{n / 1_000:.2f}k{suffix}"
    return f"{n:.2f}{suffix}"


def ratio(a: float, b: float) -> str:
    if b == 0 or a != a or b != b:
        return "n/a"
    return f"{a / b:.2f}x"


@dataclass
class BenchStats:
    """One named measurement (wall times in seconds)."""

    name: str
    suite: str
    walls: list[float] = field(default_factory=list)
    input_bytes: int = 0
    errors: int = 0
    peak_rss_bytes: int | None = None
    notes: str = ""

    def median(self) -> float:
        return statistics.median(self.walls) if self.walls else float("nan")

    def mean(self) -> float:
        return statistics.mean(self.walls) if self.walls else float("nan")

    def stdev(self) -> float:
        return statistics.stdev(self.walls) if len(self.walls) >= 2 else 0.0

    def mb_per_s(self) -> float:
        t = self.median()
        return (self.input_bytes / (1024 * 1024)) / t if t > 0 else float("nan")


def print_report(
    rows: Sequence[BenchStats],
    *,
    title: str,
    extra_lines: Sequence[str] | None = None,
    baseline_name: str | None = None,
) -> None:
    if not rows:
        return

    print()
    print("=" * 78)
    print(title)
    print("=" * 78)
    print(f"platform : {platform.system()} {platform.release()} ({platform.machine()})")
    print(f"python   : {sys.version.split()[0]}")
    if extra_lines:
        for line in extra_lines:
            print(line)
    print(f"input    : {fmt_bytes(rows[0].input_bytes)}")
    print()

    headers = ["suite", "name", "median ms", "mean ms", "±stdev", "MB/s", "peak RSS", "ok"]
    table: list[list[str]] = []
    for s in rows:
        ok = f"{len(s.walls)}/{len(s.walls) + s.errors}"
        table.append(
            [
                s.suite,
                s.name,
                f"{s.median() * 1000:.2f}" if s.walls else "n/a",
                f"{s.mean() * 1000:.2f}" if s.walls else "n/a",
                f"{s.stdev() * 1000:.2f}" if s.walls else "n/a",
                f"{s.mb_per_s():.2f}" if s.walls else "n/a",
                fmt_bytes(s.peak_rss_bytes),
                ok,
            ]
        )

    widths = [max(len(h), *(len(r[i]) for r in table)) for i, h in enumerate(headers)]
    fmt = "  ".join(f"{{:<{w}}}" for w in widths)
    print(fmt.format(*headers))
    print(fmt.format(*("-" * w for w in widths)))
    for row in table:
        print(fmt.format(*row))

    by_suite: dict[str, list[BenchStats]] = {}
    for s in rows:
        by_suite.setdefault(s.suite, []).append(s)

    printed_any = False
    for suite, members in by_suite.items():
        ok = [s for s in members if s.walls]
        if len(ok) < 2:
            continue

        # Prefer pairing rjq vs jq per case within the cli suite.
        if suite == "cli":
            by_case: dict[str, list[BenchStats]] = {}
            for s in ok:
                # names look like "rjq/access" or "jq/filter"
                case = s.name.split("/", 1)[-1] if "/" in s.name else s.name
                by_case.setdefault(case, []).append(s)
            for case, group in by_case.items():
                if len(group) < 2:
                    continue
                baseline = next((s for s in group if s.name.startswith("rjq")), group[0])
                others = [s for s in group if s is not baseline]
                if not printed_any:
                    print()
                    printed_any = True
                print(f"relative to {baseline.name} [{suite}/{case}]:")
                for other in others:
                    print(
                        f"  {other.name:<28}  wall {ratio(other.median(), baseline.median())}"
                        f"  throughput {ratio(other.mb_per_s(), baseline.mb_per_s())}"
                        "  (>1 wall = slower, >1 thr = faster)"
                    )
            continue

        baseline = None
        if baseline_name:
            baseline = next((s for s in ok if s.name == baseline_name), None)
        if baseline is None:
            baseline = ok[0]
        others = [s for s in ok if s is not baseline]
        if not printed_any:
            print()
            printed_any = True
        print(f"relative to {baseline.name} [{suite}]:")
        for other in others:
            print(
                f"  {other.name:<28}  wall {ratio(other.median(), baseline.median())}"
                f"  throughput {ratio(other.mb_per_s(), baseline.mb_per_s())}"
                "  (>1 wall = slower, >1 thr = faster)"
            )

    print()
    print("notes:")
    print("  - rust suite: JSON is parsed once; timed loops are jql_execute only.")
    print("  - cli suite: end-to-end process time (parse + query + process overhead).")
    print("  - filter jq script uses map(select(...)) as the closest stock-jq form.")
    print("=" * 78)
