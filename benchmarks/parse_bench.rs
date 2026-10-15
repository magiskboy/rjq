//! Parse-only microbenchmark: JsonParser vs SIMDParser.
//!
//! File I/O is outside the timed region. No query, no render, no print of values.
//!
//!   cargo run --release --bin parse_bench -- benchmark/fixtures/fixture_records_v2_5mb.json
//!   cargo run --release --bin parse_bench -- path/to.json --iters 50 --warmup 5

use std::hint::black_box;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use rs_jq::json::{json_load_slice, SIMDParser};

fn main() {
    let args = Args::parse();
    let data = std::fs::read(&args.path).unwrap_or_else(|e| {
        eprintln!("failed to read {}: {e}", args.path.display());
        std::process::exit(1);
    });

    let bytes = data.len();
    eprintln!(
        "file={} size={:.2} MiB iters={} warmup={}",
        args.path.display(),
        bytes as f64 / (1024.0 * 1024.0),
        args.iters,
        args.warmup,
    );
    eprintln!("simd_available={}", SIMDParser::simd_available());

    let scalar = measure(args.warmup, args.iters, || {
        json_load_slice(black_box(&data)).expect("scalar parse")
    });
    let simd = measure(args.warmup, args.iters, || {
        SIMDParser::parse_slice(black_box(&data)).expect("simd parse")
    });

    report("scalar (json_load_slice)", &scalar, bytes);
    report("simd   (SIMDParser)     ", &simd, bytes);
    if scalar.median > Duration::ZERO {
        let speedup = scalar.median.as_secs_f64() / simd.median.as_secs_f64();
        eprintln!("speedup (median scalar/simd): {speedup:.2}x");
    }
}

struct Args {
    path: PathBuf,
    iters: usize,
    warmup: usize,
}

impl Args {
    fn parse() -> Self {
        let mut path = None;
        let mut iters = 30usize;
        let mut warmup = 5usize;
        let mut argv = std::env::args().skip(1);
        while let Some(arg) = argv.next() {
            match arg.as_str() {
                "--iters" => {
                    iters = argv
                        .next()
                        .expect("--iters needs a value")
                        .parse()
                        .expect("invalid --iters");
                }
                "--warmup" => {
                    warmup = argv
                        .next()
                        .expect("--warmup needs a value")
                        .parse()
                        .expect("invalid --warmup");
                }
                "-h" | "--help" => {
                    eprintln!(
                        "usage: parse_bench <json-file> [--iters N] [--warmup N]\n\
                         times json_load_slice vs SIMDParser::parse_slice only"
                    );
                    std::process::exit(0);
                }
                other if other.starts_with('-') => {
                    eprintln!("unknown flag: {other}");
                    std::process::exit(2);
                }
                other => {
                    if path.is_some() {
                        eprintln!("unexpected argument: {other}");
                        std::process::exit(2);
                    }
                    path = Some(PathBuf::from(other));
                }
            }
        }
        Self {
            path: path.unwrap_or_else(|| {
                PathBuf::from("benchmark/fixtures/fixture_records_v2_5mb.json")
            }),
            iters,
            warmup,
        }
    }
}

struct Stats {
    samples: Vec<Duration>,
    median: Duration,
    mean: Duration,
    min: Duration,
    max: Duration,
}

fn measure<T>(warmup: usize, iters: usize, mut f: impl FnMut() -> T) -> Stats {
    for _ in 0..warmup {
        black_box(f());
    }

    let mut samples = Vec::with_capacity(iters);
    for _ in 0..iters {
        let start = Instant::now();
        let value = f();
        let elapsed = start.elapsed();
        // Drop happens after the stopwatch; only parse+DOM build is timed.
        black_box(value);
        samples.push(elapsed);
    }

    samples.sort_unstable();
    let n = samples.len() as u32;
    let sum: Duration = samples.iter().copied().sum();
    Stats {
        median: samples[samples.len() / 2],
        mean: sum / n,
        min: samples[0],
        max: *samples.last().unwrap(),
        samples,
    }
}

fn report(label: &str, stats: &Stats, bytes: usize) {
    let med_ms = stats.median.as_secs_f64() * 1e3;
    let mean_ms = stats.mean.as_secs_f64() * 1e3;
    let min_ms = stats.min.as_secs_f64() * 1e3;
    let max_ms = stats.max.as_secs_f64() * 1e3;
    let gib_s = (bytes as f64 / stats.median.as_secs_f64()) / (1024.0 * 1024.0 * 1024.0);
    eprintln!(
        "{label}: median={med_ms:.2}ms mean={mean_ms:.2}ms min={min_ms:.2}ms max={max_ms:.2}ms \
         ({gib_s:.2} GiB/s @ median, n={})",
        stats.samples.len()
    );
}
