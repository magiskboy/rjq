# Benchmarks

| Script / bin | What |
|---|---|
| [`../benchmarks/parse_bench.rs`](../benchmarks/parse_bench.rs) | parse-only: `json_load_slice` vs `SIMDParser` |
| [`cli.py`](cli.py) | `rjq` (eager / `--simd` / lazy) vs stock `jq` → table + plot |
| [`python.py`](python.py) | `rjson_load` (bytes) vs `rjson_loads` (str) → table |

## Parse-only (`parse_bench`)

```bash
cargo run --release --bin parse_bench -- benchmark/fixtures/fixture_records_v2_5mb.json
cargo run --release --bin parse_bench -- path.json --iters 50 --warmup 5
```

## CLI (`rjq` vs `jq`)

Build binaries yourself; the script does not compile anything.

```bash
cargo build --release --features cli --bin rjq
# brew install jq

uv run --with matplotlib python benchmark/cli.py \
  --rjq ./target/release/rjq \
  --jq "$(which jq)"

# options
uv run --with matplotlib python benchmark/cli.py \
  --rjq ./target/release/rjq --jq "$(which jq)" \
  --size-mb 10 --runs 7 --plot benchmark/cli_rjq_vs_jq.png
```

Variants per case: **rjq** (eager), **rjq --simd**, **rjq --parse lazy**, **jq**.

Cases: **access**, **filter**, **map**, **filter_map**, **if_else** on nested records.

## Python (`rjson_load` vs `rjson_loads`)

```bash
uv run python benchmark/python.py
uv run python benchmark/python.py --size-mb 10 --runs 7
uv run python benchmark/python.py --input path/to/data.json
```

## Notes

- Default fixture (~5 MiB) is generated under `benchmark/fixtures/` when `--input` is omitted.
- Prefer within-suite comparisons (`rjq/filter` vs `jq/filter`, or load vs loads).
