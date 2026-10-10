use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};

/// Deeply nested + wide object, mix of strings (no escapes), numbers, bools.
fn gen_twitter_like(records: usize) -> String {
    let mut s = String::from("[");
    for i in 0..records {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            r#"{{"id":{i},"id_str":"{i}","name":"user number {i}","screen_name":"handle_{i}","verified":{ver},"followers_count":{fc},"friends_count":{fr},"location":"somewhere in the world","description":"a fairly ordinary profile description without escapes","lang":"en","coordinates":[{lon:.5},{lat:.5}],"active":true,"score":{score:.4}}}"#,
            ver = if i % 3 == 0 { "true" } else { "false" },
            fc = i * 37,
            fr = i * 11,
            lon = (i as f64) * 0.013,
            lat = (i as f64) * 0.007,
            score = (i as f64) * 1.25,
        ));
    }
    s.push(']');
    s
}

/// Array of mostly numbers (number-heavy workload).
fn gen_numbers(n: usize) -> String {
    let mut s = String::from("[");
    for i in 0..n {
        if i > 0 {
            s.push(',');
        }
        if i % 4 == 0 {
            s.push_str(&format!("{}.{}e{}", i, i % 97, i % 10));
        } else {
            s.push_str(&i.to_string());
        }
    }
    s.push(']');
    s
}

/// Array of strings, some with escapes.
fn gen_strings(n: usize) -> String {
    let mut s = String::from("[");
    for i in 0..n {
        if i > 0 {
            s.push(',');
        }
        if i % 5 == 0 {
            s.push_str(&format!(r#""line {i}\ttab and \"quote\" and \n newline""#));
        } else {
            s.push_str(&format!(r#""plain ascii string value number {i} here""#));
        }
    }
    s.push(']');
    s
}

fn bench_dataset(c: &mut Criterion, name: &str, data: &str) {
    let bytes = data.as_bytes();
    let mut group = c.benchmark_group(name);
    group.throughput(Throughput::Bytes(bytes.len() as u64));

    group.bench_with_input(BenchmarkId::new("rs_jq", "slice"), &bytes, |b, &bytes| {
        b.iter(|| {
            let v = rs_jq::json::json_load_slice(black_box(bytes)).unwrap();
            black_box(v);
        });
    });

    group.bench_with_input(BenchmarkId::new("serde_json", "value"), &data, |b, &data| {
        b.iter(|| {
            let v: serde_json::Value = serde_json::from_str(black_box(data)).unwrap();
            black_box(v);
        });
    });

    group.finish();
}

fn benches(c: &mut Criterion) {
    let twitter = gen_twitter_like(2_000);
    let numbers = gen_numbers(50_000);
    let strings = gen_strings(20_000);

    eprintln!(
        "sizes: twitter={} KiB, numbers={} KiB, strings={} KiB",
        twitter.len() / 1024,
        numbers.len() / 1024,
        strings.len() / 1024
    );

    bench_dataset(c, "twitter_like", &twitter);
    bench_dataset(c, "numbers", &numbers);
    bench_dataset(c, "strings", &strings);
}

criterion_group!(g, benches);
criterion_main!(g);
