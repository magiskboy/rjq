//! One-shot wall-time bench for JQL query cases (parse once, query many).
//!
//! Cases (keep scripts in sync with `benchmark/common.py` `CASES`):
//! - `access` — deep nested path
//! - `filter` — compound filter on nested fields
//! - `map` — object projection from nested fields
//! - `filter_map` — filter then map pipeline
//!
//! ```text
//! cargo run --release --bin bench-jql -- \
//!   --input benchmark/fixtures/fixture_records_v2_5mb.json --warmup 1 --runs 5
//! ```
//!
//! Prints one JSON object per line (machine-readable for `benchmark/run.py`).

use std::env;
use std::fs;
use std::process;
use std::time::Instant;

use rs_jq::jql::jql_execute;
use rs_jq::json::json_load_slice;

const CASES: &[(&str, &str)] = &[
    ("access", ".[0].profile.prefs.theme"),
    (
        "filter",
        "filter(.active == true && .meta.level > 5 && .score > 50)",
    ),
    (
        "map",
        "map({ id: .id, city: .meta.city, roles: .profile.roles, lat: .meta.geo.lat })",
    ),
    (
        "filter_map",
        "filter(.active == true) | map({ name: .name, city: .meta.city, sku: .orders[0].sku })",
    ),
];

fn median(mut vals: Vec<f64>) -> f64 {
    if vals.is_empty() {
        return f64::NAN;
    }
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = vals.len();
    if n % 2 == 1 {
        vals[n / 2]
    } else {
        (vals[n / 2 - 1] + vals[n / 2]) / 2.0
    }
}

fn mean(vals: &[f64]) -> f64 {
    if vals.is_empty() {
        return f64::NAN;
    }
    vals.iter().sum::<f64>() / vals.len() as f64
}

fn stdev(vals: &[f64]) -> f64 {
    if vals.len() < 2 {
        return 0.0;
    }
    let m = mean(vals);
    let var = vals.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / (vals.len() - 1) as f64;
    var.sqrt()
}

fn time_runs<F>(warmup: usize, runs: usize, mut f: F) -> Result<Vec<f64>, String>
where
    F: FnMut() -> Result<(), String>,
{
    for i in 0..warmup {
        let t0 = Instant::now();
        f()?;
        eprintln!(
            "    warmup {}/{}: {:.2} ms",
            i + 1,
            warmup,
            t0.elapsed().as_secs_f64() * 1000.0
        );
    }
    let mut walls = Vec::with_capacity(runs);
    for i in 0..runs {
        let t0 = Instant::now();
        f()?;
        let dt = t0.elapsed().as_secs_f64();
        walls.push(dt);
        eprintln!("    run {}/{}: {:.2} ms [ok]", i + 1, runs, dt * 1000.0);
    }
    Ok(walls)
}

fn emit(case: &str, query: &str, input_bytes: usize, walls: &[f64]) {
    let med = median(walls.to_vec());
    let mb_s = if med > 0.0 {
        (input_bytes as f64 / (1024.0 * 1024.0)) / med
    } else {
        f64::NAN
    };
    let walls_json: String = walls
        .iter()
        .map(|v| format!("{v:.9}"))
        .collect::<Vec<_>>()
        .join(",");
    let query_esc = query.replace('\\', "\\\\").replace('"', "\\\"");
    println!(
        "{{\"name\":\"{case}\",\"suite\":\"rust\",\"query\":\"{query_esc}\",\
\"input_bytes\":{input_bytes},\"median_s\":{med:.9},\"mean_s\":{mean:.9},\
\"stdev_s\":{stdev:.9},\"mb_per_s\":{mb_s:.6},\"runs\":{runs},\"errors\":0,\
\"walls\":[{walls_json}]}}",
        mean = mean(walls),
        stdev = stdev(walls),
        runs = walls.len(),
    );
}

fn emit_error(case: &str, query: &str, input_bytes: usize) {
    let query_esc = query.replace('\\', "\\\\").replace('"', "\\\"");
    println!(
        "{{\"name\":\"{case}\",\"suite\":\"rust\",\"query\":\"{query_esc}\",\
\"input_bytes\":{input_bytes},\"median_s\":null,\"mean_s\":null,\"stdev_s\":null,\
\"mb_per_s\":null,\"runs\":0,\"errors\":1,\"walls\":[]}}"
    );
}

fn usage() -> ! {
    eprintln!("Usage: bench-jql --input <path> [--warmup N] [--runs N]");
    process::exit(2);
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut input: Option<String> = None;
    let mut warmup = 1usize;
    let mut runs = 5usize;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--input" => {
                i += 1;
                input = args.get(i).cloned();
            }
            "--warmup" => {
                i += 1;
                warmup = args
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_else(|| usage());
            }
            "--runs" => {
                i += 1;
                runs = args
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_else(|| usage());
            }
            "-h" | "--help" => usage(),
            other => {
                eprintln!("unknown arg: {other}");
                usage();
            }
        }
        i += 1;
    }

    let input = match input {
        Some(p) => p,
        None => usage(),
    };
    if runs < 1 {
        eprintln!("--runs must be >= 1");
        process::exit(2);
    }

    let data = match fs::read(&input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("failed to read {input}: {e}");
            process::exit(1);
        }
    };
    let input_bytes = data.len();

    eprintln!("input: {input} ({input_bytes} bytes)");
    eprintln!("parsing JSON once…");
    let value = match json_load_slice(&data) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("JSON parse failed: {e}");
            process::exit(1);
        }
    };

    for (case, query) in CASES {
        eprintln!("\n==> rust / {case}");
        eprintln!("    query: {query}");
        let walls = time_runs(warmup, runs, || {
            let out = jql_execute(&value, query).map_err(|e| e.to_string())?;
            std::hint::black_box(out);
            Ok(())
        });
        match walls {
            Ok(w) => emit(case, query, input_bytes, &w),
            Err(e) => {
                eprintln!("FAIL: {e}");
                emit_error(case, query, input_bytes);
            }
        }
    }
}
