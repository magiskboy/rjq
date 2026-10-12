# Benchmarks

JQL query benchmarks for the two core cases:

| Case | rjq script | jq script |
|---|---|---|
| **access** | `.[0].meta.city` | `.[0].meta.city` |
| **filter** | `filter(.active == true)` | `map(select(.active == true))` |

| Suite | What is measured |
|---|---|
| **rust** | In-process `jql_execute` (JSON parsed once, outside the timed loop) |
| **cli** | `rjq` vs stock `jq` process wall time (+ peak RSS) |

Fixtures live under `benchmark/fixtures/` (array of objects with `id`, `name`, `active`, `score`, `meta.city`, …).

## Setup

```bash
# Rust binaries (CLI + in-process rust suite)
cargo build --release --features cli --bin rjq --bin bench-jql

# Stock jq on PATH (cli suite)
# brew install jq   # macOS
```

## Run

```bash
# Default: rust + cli, both cases, ~5 MiB fixture
python benchmark/run.py

# One suite
python benchmark/run.py --suite rust
python benchmark/run.py --suite cli --runs 7
python benchmark/run.py --only rjq

# Larger fixture / custom input
python benchmark/run.py --size-mb 10
python benchmark/run.py --input path/to/data.json
```

`run.py` auto-builds release binaries unless `--no-build` is passed.

## Layout

```text
benchmark/
  run.py              # entrypoint
  common.py           # fixtures + report helpers
  rust_jql.py         # wraps bench-jql
  cli_jql.py          # rjq / jq
  fixtures/           # generated JSON (gitignored)
  rust/
    jql_query.rs      # bin: bench-jql
```

## Notes

- The **rust** suite isolates query cost (parse once). The **cli** suite is end-to-end.
- Prefer within-suite comparisons (e.g. `rjq/filter` vs `jq/filter`).
- Large fixtures are gitignored; regenerate with `python benchmark/run.py --suite fixture --size-mb 5`.
