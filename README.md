# rs-jq

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
cargo build --features cli
```

## Test

```bash
cargo test
```

## Benchmarks

Fixture ~5 MiB, Darwin arm64, Python 3.13. Re-run:

```bash
cargo build --release --features cli
# Python binding: maturin develop --release
python scripts/bench_rjson_py.py --size-mb 5
python scripts/bench_parse.py --size-mb 5
```

### Python parse (`rjson` vs others)

Full parse into a Python object tree (`dict` / `list` / …).

| Parser | median | MB/s | vs `rjson` (wall) |
|---|---:|---:|---:|
| `rjson.rjson_loads` | 85 ms | 59 | 1.00× (baseline) |
| `rjson.rjson_load` | 83 ms | 61 | 0.98× |
| `json.loads` | 38 ms | 132 | **0.45×** (faster than rjson) |
| `json.loads(bytes)` | 35 ms | 144 | **0.41×** |
| `ujson.loads` | 32 ms | 154 | **0.38×** |
| `ujson.loads(bytes)` | 30 ms | 165 | **0.36×** |
| `orjson.loads` | 23 ms | 217 | **0.27×** |
| `orjson.loads(bytes)` | 21 ms | 238 | **0.25×** |

### CLI parse (`rs-jq` vs stock `jq`)

| Tool | median | MB/s | tokens/s | peak RSS | vs `rs-jq` (wall) |
|---|---:|---:|---:|---:|---:|
| `rs-jq` (`target/release/jq`) | 194 ms | 26 | 7.9M | ~110 MiB | 1.00× (baseline) |
| stock `jq` | 289 ms | 17 | 5.3M | ~67 MiB | **1.49×** (slower than rs-jq) |

## Optimization methods

- **`Cow` for strings (and numbers)** — JSON strings without escapes and number spellings borrow the input buffer (`Cow::Borrowed`); only unescape / `into_owned` allocate. Avoids copying every scalar into a new `String` during lex/parse.
- **Builder straight to the target type** — `JsonBuilder` separates syntax (parser) from representation. Native path builds `JsonValue`; Python path (`PyBuilder`) emits `PyDict` / `PyList` / scalars directly — no intermediate `JsonValue` tree on the `rjson` hot path.
- **Lazy number parse** — lexer keeps the raw digit span (`JsonNumber` + `IntegerLike` / `FloatLike`). Parse to `i64` / `f64` (or Python `int` / `float` / big `int`) only when a consumer needs a numeric value; `json_dump` can rewrite the original spelling.
- **SIMD + NEON** — vectorized scans for string specials and non-whitespace: AVX2 on `x86_64`, NEON on Apple Silicon / `aarch64`, with scalar fallback elsewhere.
- **Pull bytes from lexer & parser** — cursor walks the input byte slice in place (`SliceCursor` / remaining window); tokens and values borrow spans instead of accumulating a separate owned buffer of characters as the primary representation.

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

### Compose

```text
.jobs[]{title,id} | filter(.id > 2) | .[0].title
.company.teams | filter(.size > 5) | .[0].name
.jobs[0,3] | filter(.active == true) | .[1].title
.people | filter(.vip == true) | .[0].name
.matrix | filter(.[0] > 3) | .[0].[1]
```
