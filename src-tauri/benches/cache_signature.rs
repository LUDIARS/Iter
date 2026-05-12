//! `cache::project_signature` のベンチ。 100 / 500 / 2000 ファイル規模の
//! プロジェクトでハッシュ計算がどれくらいで終わるかを確認する。
//!
//! signature は detect_project / try_load の頻繁に呼ぶホットパスなので、
//! 大規模 repo (Linux kernel など) でも数百ミリ秒で済むことを担保したい。

use std::fs;
use std::path::Path;

use criterion::{Criterion, criterion_group, criterion_main};
use tempfile::TempDir;

/// 同名関数を lib crate から再 export していないので、 私たちは bench に必要な
/// 同等処理をここに再実装する代わりに、 ライブラリの公開 API を直接呼ぶ形にする
/// (現状 `project_signature` は pub(crate) なので、 `cache::try_load` 経由で
/// 内部的に signature を計算するパスをベンチ対象にする)。
use iter_lib::cache;

fn populate(root: &Path, n_files: usize) {
    fs::write(root.join("CMakeLists.txt"), "project(bench)\n").unwrap();
    // n_files を 50 dirs × M files に分散して collect_relevant の再帰負荷を作る
    let per_dir = (n_files / 50).max(1);
    for d in 0..50 {
        let dir = root.join(format!("src/group_{:03}", d));
        fs::create_dir_all(&dir).unwrap();
        for f in 0..per_dir {
            let path = dir.join(format!("file_{:04}.cpp", f));
            fs::write(&path, "int x = 0;\n").unwrap();
        }
    }
}

fn bench_signature(c: &mut Criterion) {
    for &n in &[100usize, 500, 2000] {
        let tmp = TempDir::new().unwrap();
        populate(tmp.path(), n);
        let name = format!("project_signature/{}_files", n);
        c.bench_function(&name, |b| {
            b.iter(|| {
                // signature が変わったかどうかだけ反映される 軽量パス: try_load は
                // None を返すが、 その内部で project_signature を呼んでいる。
                // 純粋な signature 計測のため、 cache 不在状態 (try_load が None) で測る。
                let _: Option<serde_json::Value> = cache::try_load(tmp.path());
            });
        });
    }
}

criterion_group!(benches, bench_signature);
criterion_main!(benches);
