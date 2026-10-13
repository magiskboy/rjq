# rjq

Learning project: JSON parser + jq-like query language, written in Rust.

Still early / WIP. APIs and behavior will change.

## What's here

- `src/json` — JSON lexer & parser
- `src/jql` — query language (lexer, parser, engine)
- `src/bin/jq` — CLI binary (optional, feature `cli`)

## Build

```bash
cargo build
```

CLI:

```bash
cargo build --release --features cli --bin rjq
```

## Test

```bash
cargo test
```

## Benchmarks

See [`benchmark/`](benchmark/README.md). Two scripts:

| Script | What |
|---|---|
| `benchmark/cli.py` | `rjq` vs stock `jq` → stdout table + plot |
| `benchmark/python.py` | `rjson_load` (bytes) vs `rjson_loads` (str) → stdout |

```bash
# CLI (build binaries yourself; pass paths)
cargo build --release --features cli --bin rjq
uv run --with matplotlib python benchmark/cli.py \
  --rjq ./target/release/rjq --jq "$(which jq)"

# Python bindings
uv run python benchmark/python.py
```

### CLI: rjq vs jq

Median wall time on a ~5 MiB nested-records fixture (Darwin arm64, 5 timed runs):

![CLI performance: rjq vs jq](benchmark/plot.png)

| Case | rjq | jq | Relative |
|---|---:|---:|---|
| access | 55.5 ms | 93.0 ms | rjq **1.68×** faster |
| filter | 70.7 ms | 129.3 ms | rjq **1.83×** faster |
| map | 77.6 ms | 151.3 ms | rjq **1.95×** faster |
| filter_map | 75.9 ms | 125.0 ms | rjq **1.65×** faster |

## Optimization methods

- **`Cow` for strings (and numbers)** — JSON strings without escapes and number spellings borrow the input buffer (`Cow::Borrowed`); only unescape / `into_owned` allocate. Avoids copying every scalar into a new `String` during lex/parse.
- **JQL `Proxy` lazy materialize** — engine intermediates are `Borrowed` / `Shared(Rc)` / `Owned` instead of deep-cloning between ops. Access and element iteration stay zero-copy on the input document; fan-out (`binary` / `object`) shares owned trees; new arrays/objects allocate only when the op builds a new shape; `into_owned` stays at CLI / `'static` boundaries.
- **Builder straight to the target type** — `JsonBuilder` separates syntax (parser) from representation. Native path builds `JsonValue`; Python path (`PyBuilder`) emits `PyDict` / `PyList` / scalars directly — no intermediate `JsonValue` tree on the `rjson` hot path.
- **Lazy number parse** — lexer keeps the raw digit span (`JsonNumber` + `IntegerLike` / `FloatLike`). Parse to `i64` / `f64` (or Python `int` / `float` / big `int`) only when a consumer needs a numeric value; `json_dump` can rewrite the original spelling.
- **SIMD + NEON** — vectorized scans for string specials and non-whitespace: AVX2 on `x86_64`, NEON on Apple Silicon / `aarch64`, with scalar fallback elsewhere.

## Notable JQL queries

Queries below run against the fixture `src/jql/sample.json`.

### Access

```text
.                              # whole document
.count                         # 42
.jobs[1]                       # second job
.jobs[0,2]                     # pick indices
.jobs[1:3]                     # slice [start:end)
.jobs[]                        # all items (identity on array)
.person{name,age}              # project object keys
.jobs[]{title,description}     # project keys on each item
.company.teams[0].name         # "platform"
.matrix[0][1]                  # nested index → 2
.nested[1][1][0]               # deep nest → 3
```

### Pipe

```text
.matrix[0] | .[1]                              # 2
.company | .teams[1] | .name                   # "data"
.jobs | .[0].title                             # "Dev"
.matrix[1] | .[0,2]                            # [4, 6]
.count | . > 40                                # true
.jobs[0].title | . == "Dev"                    # true
```

### Filter

```text
.jobs | filter(.active == true)
.jobs | filter(.id > 2)
.people | filter(.vip == true && .score >= 70)
.candidates | filter(.id > 10 && .age < 20 && (.money > 10 || .gold >= 1))
.jobs | filter((.level == "senior" || .level == "mid") && .money >= 50)
```

### Map

```text
.jobs | map(.title)
.jobs | filter(.active == true) | map(.id)
.matrix | map(.[0])
.jobs | filter(.id > 2) | map({ name: .title, pay: .money })
.company.teams | map({ name: .name, size: .size })
```

### Compose

```text
.jobs[]{title,id} | filter(.id > 2) | .[0].title
.company.teams | filter(.size > 5) | .[0].name
.jobs[0,3] | filter(.active == true) | .[1].title
.people | filter(.vip == true) | .[0].name
.matrix | filter(.[0] > 3) | .[0].[1]
.jobs | filter(.active == true) | map(.title)
```
