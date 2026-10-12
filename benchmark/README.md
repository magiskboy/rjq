# Benchmarks

Unified JSON parse benchmarks for this project (`rs_jq` / `rjson` / CLI) against:

| Suite | Parsers | What is measured |
|---|---|---|
| **rust** | `rs_jq`, `serde_json` | Native value trees, in-process |
| **python** | `rjson`, `json`, `orjson`, `ujson` | Python object trees, in-process |
| **cli** | `rs-jq`, stock `jq` | Process wall time (+ peak RSS) |
| **criterion** | `rs_jq`, `serde_json` | Criterion microbench + HTML report |

All wall-time suites share fixtures under `benchmark/fixtures/`.

## Setup

```bash
# Rust binaries (CLI + oneshot rust suite)
cargo build --release --features "cli,bench" --bin jq --bin bench-json-parse

# Python peers + this project's binding
pip install -r benchmark/requirements.txt
pip install maturin
maturin develop --release

# Stock jq on PATH (cli suite)
# brew install jq   # macOS
```

## Run

```bash
# Default: rust + python + cli on a ~5 MiB fixture
python benchmark/run.py

# One suite
python benchmark/run.py --suite rust
python benchmark/run.py --suite python --only rjson,orjson,ujson --mode load
python benchmark/run.py --suite cli --runs 7

# Larger fixture / custom input
python benchmark/run.py --size-mb 10
python benchmark/run.py --input path/to/data.json

# Criterion microbench (also uses shared fixture when present)
python benchmark/run.py --suite criterion
# or:
cargo bench --bench json_parse
```

`run.py` auto-builds release binaries unless `--no-build` is passed.

## Layout

```text
benchmark/
  run.py              # entrypoint
  common.py           # fixtures + report helpers
  rust_parse.py       # wraps bench-json-parse
  python_parse.py     # rjson / json / orjson / ujson
  cli_parse.py        # rs-jq / jq
  requirements.txt
  fixtures/           # generated JSON (gitignored)
  rust/
    oneshot.rs        # bin: bench-json-parse
    json_parse.rs     # Criterion bench
```

## Notes

- Suites are **not** directly comparable across rows: Rust builds a native tree, Python builds `dict`/`list`, CLI also runs a query engine / process overhead.
- Prefer within-suite comparisons (e.g. `rs_jq` vs `serde_json`, or `rjson` vs `orjson`).
- Large fixtures are gitignored; regenerate with `python benchmark/run.py --suite fixture --size-mb 5`.
