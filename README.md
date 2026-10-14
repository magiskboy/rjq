# rjq

Learning project: JSON parser + jq-like query language, written in Rust.

Median wall time on a ~10 MiB JSON.

![CLI performance: rjq vs jq](benchmark/plot.png)

| Case | rjq | jq | Relative |
|---|---:|---:|---|
| access | 55.5 ms | 93.0 ms | rjq **1.68×** faster |
| filter | 70.7 ms | 129.3 ms | rjq **1.83×** faster |
| map | 77.6 ms | 151.3 ms | rjq **1.95×** faster |
| filter_map | 75.9 ms | 125.0 ms | rjq **1.65×** faster |

**Optimization methods**

- **`Cow` for strings (and numbers)** — JSON strings without escapes and number spellings borrow the input buffer (`Cow::Borrowed`); only unescape / `into_owned` allocate. Avoids copying every scalar into a new `String` during lex/parse.
- **JQL `Proxy` lazy materialize** — engine intermediates are `Borrowed` / `Shared(Rc)` / `Owned` instead of deep-cloning between ops. Access and element iteration stay zero-copy on the input document; fan-out (`binary` / `object`) shares owned trees; new arrays/objects allocate only when the op builds a new shape; `into_owned` stays at CLI / `'static` boundaries.
- **Builder straight to the target type** — `JsonBuilder` separates syntax (parser) from representation. Native path builds `JsonValue`; Python path (`PyBuilder`) emits `PyDict` / `PyList` / scalars directly — no intermediate `JsonValue` tree on the `rjson` hot path.
- **Lazy number parse** — lexer keeps the raw digit span (`JsonNumber` + `IntegerLike` / `FloatLike`). Parse to `i64` / `f64` (or Python `int` / `float` / big `int`) only when a consumer needs a numeric value; `json_dump` can rewrite the original spelling.
- **SIMD + NEON** — vectorized scans for string specials and non-whitespace: AVX2 on `x86_64`, NEON on Apple Silicon / `aarch64`, with scalar fallback elsewhere.

## Build

```bash
cargo build
```

CLI:

```bash
cargo build --release --features cli --bin rjq
```

## Notable JQL queries

Queries below run against the fixture `src/jql/sample.json`.

**Access**

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

**Pipe**

```text
.matrix[0] | .[1]                              # 2
.company | .teams[1] | .name                   # "data"
.jobs | .[0].title                             # "Dev"
.matrix[1] | .[0,2]                            # [4, 6]
.count | . > 40                                # true
.jobs[0].title | . == "Dev"                    # true
```

**Filter**

```text
.jobs | filter(.active == true)
.jobs | filter(.id > 2)
.people | filter(.vip == true && .score >= 70)
.candidates | filter(.id > 10 && .age < 20 && (.money > 10 || .gold >= 1))
.jobs | filter((.level == "senior" || .level == "mid") && .money >= 50)
```

**Map**

```text
.jobs | map(.title)
.jobs | filter(.active == true) | map(.id)
.matrix | map(.[0])
.jobs | filter(.id > 2) | map({ name: .title, pay: .money })
.company.teams | map({ name: .name, size: .size })
```

**Compose**

```text
.jobs[]{title,id} | filter(.id > 2) | .[0].title
.company.teams | filter(.size > 5) | .[0].name
.jobs[0,3] | filter(.active == true) | .[1].title
.people | filter(.vip == true) | .[0].name
.matrix | filter(.[0] > 3) | .[0].[1]
.jobs | filter(.active == true) | map(.title)
```

## Colored output

Syntax highlighting adapts to the terminal (`NO_COLOR`, `FORCE_COLOR`, `COLORTERM`, `TERM`). Disable with `--no-color`.

Override colors via a one-line `RJQ_COLORS` scheme (`role=#RRGGBB`, comma-separated). Roles: `key`, `string`, `number`, `boolean`, `null`, `punct`. Omit a role to keep the default. Unset / invalid → built-in default (VS Code Dark+).

```bash
export RJQ_COLORS='key=#569CD6,string=#CE9178,number=#B5CEA8,boolean=#569CD6,null=#808080,punct=#D4D4D4'
```

Popular schemes (copy into `~/.bashrc` / `~/.zshrc` / direnv):

| Theme | `RJQ_COLORS` |
|---|---|
| **VS Code Dark+** (default) | `key=#569CD6,string=#CE9178,number=#B5CEA8,boolean=#569CD6,null=#808080,punct=#D4D4D4` |
| **Dracula** | `key=#8BE9FD,string=#F1FA8C,number=#BD93F9,boolean=#FF79C6,null=#6272A4,punct=#F8F8F2` |
| **Nord** | `key=#81A1C1,string=#A3BE8C,number=#B48EAD,boolean=#EBCB8B,null=#4C566A,punct=#D8DEE9` |
| **Solarized Dark** | `key=#268BD2,string=#2AA198,number=#D33682,boolean=#B58900,null=#586E75,punct=#839496` |
| **Monokai** | `key=#F92672,string=#E6DB74,number=#AE81FF,boolean=#AE81FF,null=#75715E,punct=#F8F8F2` |
| **Gruvbox Dark** | `key=#83A598,string=#B8BB26,number=#D3869B,boolean=#FABD2F,null=#928374,punct=#EBDBB2` |
| **Catppuccin Mocha** | `key=#89B4FA,string=#A6E3A1,number=#FAB387,boolean=#CBA6F7,null=#6C7086,punct=#CDD6F4` |
| **One Dark** | `key=#E06C75,string=#98C379,number=#D19A66,boolean=#56B6C2,null=#5C6370,punct=#ABB2BF` |
| **Tokyo Night** | `key=#7AA2F7,string=#9ECE6A,number=#FF9E64,boolean=#BB9AF7,null=#565F89,punct=#A9B1D6` |

```bash
# example: Nord for this shell
export RJQ_COLORS='key=#81A1C1,string=#A3BE8C,number=#B48EAD,boolean=#EBCB8B,null=#4C566A,punct=#D8DEE9'
rjq -i data.json -s '.jobs[]'
rjq --no-color -i data.json   # plain output
```
