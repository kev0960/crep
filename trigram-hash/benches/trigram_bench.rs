use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::Throughput;
use criterion::criterion_group;
use criterion::criterion_main;
use trigram_hash::trigram_hash::split_lines_to_tokens;
use trigram_hash::trigram_hash_v0::split_lines_to_tokens_v0;

const URLS: &[(&str, &str)] = &[
    ("ts", "https://unpkg.com/typescript@5.9.3/lib/typescript.js"),
    (
        "sqlite",
        "https://raw.githubusercontent.com/azadkuh/sqlite-amalgamation/refs/heads/master/sqlite3.c",
    ),
];

#[derive(Clone)]
struct Fixture {
    contents: Vec<(&'static str, Vec<String>)>,
}

static FIXTURE: OnceLock<Fixture> = OnceLock::new();

fn run(cwd: &Path, args: &[&str]) {
    std::process::Command::new(args[0])
        .args(&args[1..])
        .current_dir(cwd)
        .output()
        .expect("spawn ok");
}

fn fetch_file_to_test(url: &str) -> String {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();

    run(root, &["curl", "-L", "-o", "output", url]);

    let content = std::fs::read(root.join("output")).unwrap();
    String::from_utf8(content).unwrap()
}

fn get_fixture() -> &'static Fixture {
    FIXTURE.get_or_init(|| {
        let mut contents = vec![];
        for (label, url) in URLS {
            let content = fetch_file_to_test(url);
            contents.push((
                *label,
                content
                    .lines()
                    .map(|s| s.to_owned())
                    .collect::<Vec<String>>(),
            ));
        }

        Fixture { contents }
    })
}

pub fn bench(c: &mut Criterion) {
    let fixture_set = get_fixture();

    let mut g = c.benchmark_group("trigrams");
    g.sample_size(10).measurement_time(Duration::from_secs(10));

    for (label, fx) in &fixture_set.contents {
        g.throughput(Throughput::Bytes(fx.len() as u64));

        g.bench_with_input(BenchmarkId::new("v0", *label), &fx, |b, &input| {
            b.iter(|| split_lines_to_tokens_v0(input, 0));
        });

        g.bench_with_input(BenchmarkId::new("v1", *label), &fx, |b, &input| {
            b.iter(|| split_lines_to_tokens(input, 0));
        });
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
