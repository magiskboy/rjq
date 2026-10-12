"""In-process Python JSON parse suite: rjson, json, orjson, ujson."""

from __future__ import annotations

import json
import sys
import time
from typing import Any, Callable, Sequence

from common import BenchStats, ensure_fixture, fmt_bytes


def try_import(name: str, pip_hint: str) -> Any | None:
    try:
        return __import__(name)
    except ImportError:
        print(f"skip {name}: not installed (pip install {pip_hint})", file=sys.stderr)
        return None


def import_rjson():
    try:
        import rjson  # type: ignore

        return rjson
    except ImportError as exc:
        raise SystemExit(
            "cannot import rjson (Python binding for rs-jq).\n"
            "Build/install it with:\n"
            "  pip install maturin\n"
            "  maturin develop --release\n"
            f"\nOriginal error: {exc}"
        ) from exc


def _same_shape(a: Any, b: Any) -> bool:
    if isinstance(a, dict) and isinstance(b, dict):
        if a.keys() != b.keys():
            return False
        return all(_same_shape(a[k], b[k]) for k in a)
    if isinstance(a, list) and isinstance(b, list):
        if len(a) != len(b):
            return False
        return all(_same_shape(x, y) for x, y in zip(a, b))
    if isinstance(a, bool) and isinstance(b, bool):
        return a is b
    if (
        isinstance(a, (int, float))
        and isinstance(b, (int, float))
        and not isinstance(a, bool)
        and not isinstance(b, bool)
    ):
        return abs(float(a) - float(b)) <= 1e-6 * max(1.0, abs(float(a)))
    return a == b


def _bench(
    name: str,
    fn: Callable[[], Any],
    *,
    input_bytes: int,
    warmup: int,
    runs: int,
) -> BenchStats:
    print(f"\n==> python / {name}", file=sys.stderr)
    for i in range(warmup):
        t0 = time.perf_counter()
        try:
            fn()
        except Exception as exc:
            print(f"    warmup {i + 1} FAILED: {exc}", file=sys.stderr)
            return BenchStats(
                name=name, suite="python", walls=[], input_bytes=input_bytes, errors=1
            )
        dt = time.perf_counter() - t0
        print(f"    warmup {i + 1}/{warmup}: {dt * 1000:.2f} ms", file=sys.stderr)

    walls: list[float] = []
    errors = 0
    for i in range(runs):
        t0 = time.perf_counter()
        try:
            fn()
            dt = time.perf_counter() - t0
            walls.append(dt)
            print(f"    run {i + 1}/{runs}: {dt * 1000:.2f} ms [ok]", file=sys.stderr)
        except Exception as exc:
            errors += 1
            print(f"    run {i + 1}/{runs}: FAIL — {exc}", file=sys.stderr)

    return BenchStats(
        name=name, suite="python", walls=walls, input_bytes=input_bytes, errors=errors
    )


def run(
    *,
    input_path=None,
    size_mb: float = 5.0,
    force_generate: bool = False,
    warmup: int = 1,
    runs: int = 5,
    mode: str = "both",
    only: Sequence[str] = ("rjson", "json", "orjson", "ujson"),
    verify: bool = False,
) -> list[BenchStats]:
    path = ensure_fixture(input_path, size_mb, force_generate)
    raw = path.read_bytes()
    text = raw.decode("utf-8")
    input_bytes = len(raw)
    print(f"input: {path} ({fmt_bytes(input_bytes)})", file=sys.stderr)

    selected = list(only)
    modules: dict[str, Any] = {}
    if "json" in selected:
        modules["json"] = json
    if "rjson" in selected:
        modules["rjson"] = import_rjson()
    if "orjson" in selected:
        mod = try_import("orjson", "orjson")
        if mod is None:
            if selected == ["orjson"]:
                raise SystemExit("orjson is required for --only orjson")
            selected = [p for p in selected if p != "orjson"]
        else:
            modules["orjson"] = mod
    if "ujson" in selected:
        mod = try_import("ujson", "ujson")
        if mod is None:
            if selected == ["ujson"]:
                raise SystemExit("ujson is required for --only ujson")
            selected = [p for p in selected if p != "ujson"]
        else:
            modules["ujson"] = mod

    if not selected:
        raise SystemExit("no Python parsers available to benchmark")

    if verify:
        ref = json.loads(text)
        checks: list[tuple[str, Any]] = []
        if "rjson" in modules:
            checks.append(("rjson.rjson_loads", modules["rjson"].rjson_loads(text)))
            checks.append(("rjson.rjson_load", modules["rjson"].rjson_load(raw)))
        if "orjson" in modules:
            checks.append(("orjson.loads(str)", modules["orjson"].loads(text)))
            checks.append(("orjson.loads(bytes)", modules["orjson"].loads(raw)))
        if "ujson" in modules:
            checks.append(("ujson.loads(str)", modules["ujson"].loads(text)))
            checks.append(("ujson.loads(bytes)", modules["ujson"].loads(raw)))
        for label, value in checks:
            if not _same_shape(ref, value):
                raise SystemExit(f"verify failed: {label} shape != json.loads")
        print(
            f"verify: ok ({len(checks)} parsers; numbers may be float-only)",
            file=sys.stderr,
        )

    results: list[BenchStats] = []
    modes = ("loads", "load") if mode == "both" else (mode,)

    for m in modes:
        for name in selected:
            if name == "json":
                if m == "loads":
                    fn: Callable[[], Any] = lambda t=text: json.loads(t)
                    label = "json.loads"
                else:
                    fn = lambda b=raw: json.loads(b)
                    label = "json.loads(bytes)"
            elif name == "orjson":
                mod = modules["orjson"]
                if m == "loads":
                    fn = lambda t=text, mod=mod: mod.loads(t)
                    label = "orjson.loads"
                else:
                    fn = lambda b=raw, mod=mod: mod.loads(b)
                    label = "orjson.loads(bytes)"
            elif name == "ujson":
                mod = modules["ujson"]
                if m == "loads":
                    fn = lambda t=text, mod=mod: mod.loads(t)
                    label = "ujson.loads"
                else:
                    fn = lambda b=raw, mod=mod: mod.loads(b)
                    label = "ujson.loads(bytes)"
            elif name == "rjson":
                mod = modules["rjson"]
                if m == "loads":
                    fn = lambda t=text, mod=mod: mod.rjson_loads(t)
                    label = "rjson.rjson_loads"
                else:
                    fn = lambda b=raw, mod=mod: mod.rjson_load(b)
                    label = "rjson.rjson_load"
            else:
                continue

            results.append(
                _bench(label, fn, input_bytes=input_bytes, warmup=warmup, runs=runs)
            )

    return results
