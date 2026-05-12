# ADR-004: キャッシュ妥当性は relevant-file mtime ハッシュ

**Status**: Accepted (2026-05, PR #18, #11 M3)

## Context

`detect_project` は plot しないと数千ファイルの走査で 1-3 秒かかる。
キャッシュを `<config_dir>/iter/projects/<hash>.json` に置いて 2 回目以降を
高速化したい。 問題は **キャッシュ無効化のキー** を何にするか:

| 候補 | 長所 | 短所 |
|---|---|---|
| **root dir mtime のみ** | 計算 1 回、 ほぼ無料 | 子ディレクトリでファイル追加されても root mtime が変わらず stale を返す (実観測) |
| **全ファイル mtime hash** | 完全正確 | node_modules / target を走査すると重い、 巨大 repo で seconds 単位 |
| **relevant-file mtime hash** | 妥当性と速度のバランス | "relevant" の定義を維持するコストがある |
| **inotify / fsevents 常時 watch** | 完全リアルタイム | 別途 watcher 常駐、 Tauri ライフサイクル管理が複雑 |

## Decision

**relevant-file mtime hash**。 `src-tauri/src/cache.rs:project_signature`:

- 走査対象: `*.c / *.cc / *.cpp / *.cxx / *.h / *.hpp / *.hxx / *.cmake / *.sln / *.vcxproj / *.csproj / CMakeLists.txt`
- skip dir: `node_modules / target / .git / .svn / .hg / dist / build / out / .iter / .cache / .vs / .idea`
- 深さ上限 8 (現実的なリポは 5-6 階層)
- symlink は cycle 防止で skip
- root の mtime は **意図的に含めない** (README 追加で signature が変わってしまうのを避ける)
- (relative-path, mtime) を path で sort してから hash → 順序非依存

cache schema は v1 → v2 に bump。 古い v1 cache (`root_mtime_secs` キー) は version mismatch で破棄され、 fresh scan が走る。

## Consequences

- **+ 子ディレクトリ変更を検出** — 主たる pain point が解消。
- **+ 無関係ファイルで無効化されない** — README / package.json / *.md の編集では cache が活きる。
- **+ benchmark** — `src-tauri/benches/cache_signature.rs` で 100/500/2000 ファイル相当のコストを計測 (`cargo bench`)。
- **- "relevant" 定義の保守** — 新言語サポート (Rust, Go 等) を入れる場合 `is_relevant_file` に追記が要る。
- **- mtime resolution 依存** — Windows は 1 秒粒度、 1 秒以内の連続編集を取り逃す可能性。 実用上は問題なし (人間操作タイムスケールでは十分)。
