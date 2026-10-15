# Tests

## Layout

| Path | What |
|------|------|
| `json/` | Unit tests for the JSON lexer / parser / value (pulled into `src/` via `#[path]`) |
| `jql/` | Unit tests for the query lexer / parser / engine (same) |
| `json/suite/` | [JSONTestSuite](https://github.com/nst/JSONTestSuite) fixtures (`y_*` / `n_*` / `i_*`) |
| `suite_e2e.rs` | E2E: `rjq` binary vs system `jq` on those fixtures (`eager` + `lazy`) |

## Run

Library unit tests:

```bash
cargo test
```

CLI + e2e suite (needs `jq` on `PATH`):

```bash
cargo test --features cli
```

E2e only (two cases: `eager` / `lazy`):

```bash
cargo test --features cli --test suite_e2e
cargo test --features cli --test suite_e2e eager
cargo test --features cli --test suite_e2e lazy
```
