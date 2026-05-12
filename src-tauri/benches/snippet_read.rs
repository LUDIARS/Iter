//! `snippet::read_snippet` のベンチ。 中規模 (1000 行) と大規模 (10 万行) の
//! ファイルから 50 行コンテキストを切り出すコストを測る。 graph node の
//! プレビュー描画で連続コールされるホットパス。

use std::fs;
use std::io::Write;
use std::path::Path;

use criterion::{Criterion, criterion_group, criterion_main};
use tempfile::TempDir;

use iter_lib::snippet;

fn make_file(path: &Path, lines: usize) {
    let mut f = fs::File::create(path).unwrap();
    for i in 0..lines {
        writeln!(f, "// line {:06} — some content here for testing", i + 1).unwrap();
    }
}

fn bench_snippet(c: &mut Criterion) {
    let tmp = TempDir::new().unwrap();
    let small = tmp.path().join("small.cpp");
    make_file(&small, 1_000);
    let big = tmp.path().join("big.cpp");
    make_file(&big, 100_000);

    c.bench_function("read_snippet/1k_lines_at_500_ctx5", |b| {
        b.iter(|| {
            let _ = snippet::read_snippet(small.to_string_lossy().into_owned(), 500, 5);
        });
    });
    c.bench_function("read_snippet/100k_lines_at_50000_ctx5", |b| {
        b.iter(|| {
            let _ = snippet::read_snippet(big.to_string_lossy().into_owned(), 50_000, 5);
        });
    });
    c.bench_function("read_snippet/100k_lines_at_99000_ctx10", |b| {
        b.iter(|| {
            let _ = snippet::read_snippet(big.to_string_lossy().into_owned(), 99_000, 10);
        });
    });
}

criterion_group!(benches, bench_snippet);
criterion_main!(benches);
