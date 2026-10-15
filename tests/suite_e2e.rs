//! E2E correctness suite for the `rjq` binary (`--parse eager` and `--parse lazy`).
//!
//! Fixtures: [nst/JSONTestSuite](https://github.com/nst/JSONTestSuite) under
//! `tests/json/suite/` (`y_*` accept · `n_*` reject · `i_*` implementation-defined).
//!
//! System `jq` is the evaluator: its compact stdout is ground truth whenever it
//! accepts. `rjq` must match that value (after normalizing `rjq` stdout through
//! `jq -c`). Reject / impl-defined cases follow the suite prefixes.
//!
//! Lazy parse may stop after the first complete value, so trailing junk that
//! makes a fixture `n_*` for eager can still succeed under `--parse lazy`.
//!
//! ```bash
//! cargo test --features cli --test suite_e2e
//! ```
//!
//! Skipped deep-nesting stress files:
//! `i_structure_500_nested_arrays`, `n_structure_100000_opening_arrays`,
//! `n_structure_open_array_object`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

const SKIP: &[&str] = &[
    "i_structure_500_nested_arrays.json",
    "n_structure_100000_opening_arrays.json",
    "n_structure_open_array_object.json",
];

fn suite_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/json/suite")
}

fn rjq_bin() -> &'static Path {
    static BIN: OnceLock<PathBuf> = OnceLock::new();
    BIN.get_or_init(|| PathBuf::from(env!("CARGO_BIN_EXE_rjq")))
        .as_path()
}

fn require_jq() {
    let status = Command::new("jq")
        .arg("--version")
        .output()
        .unwrap_or_else(|err| panic!("jq is required for suite_e2e but could not be run: {err}"));
    assert!(
        status.status.success(),
        "jq --version failed: {}",
        String::from_utf8_lossy(&status.stderr)
    );
}

fn run_jq(path: &Path) -> Output {
    Command::new("jq")
        .args(["-c", "."])
        .arg(path)
        .output()
        .unwrap_or_else(|err| panic!("failed to spawn jq on {}: {err}", path.display()))
}

fn run_rjq(path: &Path, parse_mode: &str) -> Output {
    Command::new(rjq_bin())
        .args(["--no-color", "--parse", parse_mode, "-i"])
        .arg(path)
        .args(["-s", "."])
        .env("NO_COLOR", "1")
        .output()
        .unwrap_or_else(|err| {
            panic!(
                "failed to spawn rjq --parse {parse_mode} on {}: {err}",
                path.display()
            )
        })
}

/// Compact-encode `stdout` with `jq -c .` so pretty-print / number spelling match.
fn jq_canonicalize(stdout: &[u8]) -> Result<Vec<u8>, String> {
    let out = Command::new("jq")
        .args(["-c", "."])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(stdout)?;
            }
            child.wait_with_output()
        })
        .map_err(|err| err.to_string())?;

    if !out.status.success() {
        return Err(format!(
            "jq -c failed while normalizing rjq stdout: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(out.stdout)
}

fn fixtures() -> Vec<PathBuf> {
    let mut paths: Vec<_> = std::fs::read_dir(suite_dir())
        .expect("read tests/json/suite")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "json")
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|name| !SKIP.contains(&name))
        })
        .collect();
    paths.sort();
    paths
}

enum Kind {
    Accept,
    Reject,
    Impl,
}

fn kind_of(name: &str) -> Kind {
    if name.starts_with("y_") {
        Kind::Accept
    } else if name.starts_with("n_") {
        Kind::Reject
    } else if name.starts_with("i_") {
        Kind::Impl
    } else {
        panic!("unexpected fixture name: {name}");
    }
}

fn assert_outputs_match(jq: &Output, rjq: &Output) -> Result<(), String> {
    if !rjq.status.success() {
        return Err(format!(
            "rjq rejected (exit {:?}): {}",
            rjq.status.code(),
            String::from_utf8_lossy(&rjq.stderr)
        ));
    }
    let expected = &jq.stdout;
    let actual = jq_canonicalize(&rjq.stdout)?;
    if expected != &actual {
        return Err(format!(
            "output mismatch\n  jq:  {}\n  rjq: {}",
            String::from_utf8_lossy(expected).trim_end(),
            String::from_utf8_lossy(&actual).trim_end()
        ));
    }
    Ok(())
}

fn check_reject(path: &Path, parse_mode: &str, rjq: &Output) -> Result<(), String> {
    if !rjq.status.success() {
        return Ok(());
    }

    // Lazy may accept a complete value followed by junk (eager still rejects).
    if parse_mode == "lazy" {
        let eager = run_rjq(path, "eager");
        if !eager.status.success() && jq_canonicalize(&rjq.stdout).is_ok() {
            return Ok(());
        }
    }

    Err(format!(
        "rjq --parse {parse_mode} accepted n_ fixture (suite requires reject)"
    ))
}

fn check_fixture(path: &Path, name: &str, parse_mode: &str, jq: &Output) -> Result<(), String> {
    let rjq = run_rjq(path, parse_mode);
    match kind_of(name) {
        Kind::Accept => {
            if !jq.status.success() {
                Err(format!(
                    "jq unexpectedly rejected y_ fixture: {}",
                    String::from_utf8_lossy(&jq.stderr)
                ))
            } else {
                assert_outputs_match(jq, &rjq)
            }
        }
        Kind::Reject => check_reject(path, parse_mode, &rjq),
        Kind::Impl => match (jq.status.success(), rjq.status.success()) {
            (true, true) => assert_outputs_match(jq, &rjq),
            _ => Ok(()),
        },
    }
}

fn run_suite(parse_mode: &str) {
    require_jq();

    let mut failures: Vec<String> = Vec::new();
    let mut checked = 0usize;

    for path in fixtures() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let jq = run_jq(&path);
        checked += 1;
        if let Err(err) = check_fixture(&path, &name, parse_mode, &jq) {
            failures.push(format!("{name}: {err}"));
        }
    }

    assert!(
        failures.is_empty(),
        "suite_e2e --parse {parse_mode}: {}/{} fixtures failed:\n{}",
        failures.len(),
        checked,
        failures.join("\n")
    );
}

#[test]
fn json_suite_rjq_matches_jq_eager() {
    run_suite("eager");
}

#[test]
fn json_suite_rjq_matches_jq_lazy() {
    run_suite("lazy");
}
