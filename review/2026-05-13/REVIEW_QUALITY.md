# Iter — REVIEW_QUALITY (2026-05-13)

## 総合評価: A-

各モジュールに `//!` doc comment + 設計意図コメントが付き、 cargo test と vitest が両方走る。 CI は ubuntu/windows/macos の 3 OS matrix で rust+frontend を実行。 LICENSE/NOTICE/THIRD_PARTY_LICENSES の OSS 法務、 criterion benchmark の準備、 ADR の運用も整っている。 v0.2 規模としては高水準。

## 良い点

- **モジュール先頭の `//!` ドキュメント**: cache.rs:1-15、 lsp.rs:1-12、 compile_db.rs:1-15、 stack_trace.rs:1-12 など全ての Rust 主要モジュールに目的・入出力・スコープが書かれている。 1 ファイル開けば「何をやる」が分かる。
- **テスト網羅**: project.rs:214-269 / compile_db.rs:310-427 / stack_trace.rs:130-271 / cache.rs:229-327 / lsp.test.ts:1-31 と、 ロジックが密な箇所には必ず unit test がある。 Windows mtime resolution に合わせた `wait_mtime_tick` (cache.rs:240-242) のような細やかな対応も◎。
- **CI matrix の構成**: .github/workflows/ci.yml の 3 OS × (cargo check + cargo test + tsc + vitest + vite build) で main マージ前の信頼度が高い。 least-privilege の `permissions: contents: read` (#17 で導入) も妥当。
- **OSS 法務の整備**: LICENSE / NOTICE / THIRD_PARTY_LICENSES.md が揃い、 README にも明示。
- **criterion bench**: src-tauri/benches/cache_signature.rs / snippet_read.rs で hot path 候補を予め計測可能化。 baseline 比較は今後の TODO として明示済。

## 改善点

### Q1: Rust の `unwrap()` が散在 (lsp.rs:212, 219, 237, 252, 264, 273, 296, 320)

`serde_json::to_value(params).unwrap()` が多用される。 serde は通常 fail しないが、 panic-on-RPC は debug 性が悪い。 `?` で `LspError::Rpc(e.to_string())` に統一する方が安全 + clean。 AUTOFIX 候補。

### Q2: `expect("stdin captured")` 等の expect (lsp.rs:83-85)

子プロセス spawn 直後の stdin/stdout/stderr unwrap は実質 infallible だが、 contract コメントとして「`Stdio::piped()` を指定したので必ず Some」と書いておくと良い。

### Q3: TypeScript の `any` / `!` 非使用は ◎ だが、 `?? []` で空配列フォールバックが暗黙

FileWindow.tsx:204 の `h?.items[0]?.name ?? ""` は妥当だが、 graph node 0 件のときに「カーソル位置はシンボルじゃない」状態と「LSP がまだ起動していない」状態を UI で区別できない。 RelationData に `lspState` を持たせると良い (実装変更を伴うので AUTOFIX 対象外)。

### Q4: コメント量が多すぎて diff レビュー時のノイズになる箇所

cache.rs:1-15、 compile_db.rs:1-15 などは適切だが、 lsp.rs:301-306 の definitions 解説などは長すぎる場合がある。 過剰な日本語コメントは多言語混在 git diff のときに読みにくい。 質より量に偏った箇所は要トリミング。

### Q5: `eprintln!` で warn を出している箇所がある (lsp_commands.rs:62-66)

Tauri app では `tracing` / `log` crate を使い、 frontend に emit する方が user-facing。 eprintln は stderr に流れるだけで dev mode 以外で見えない。

### Q6: vite/vitest config が確認できないが、 frontend テストが lsp.test.ts 1 本のみ

uriToPath だけで、 ControlPanel / FileWindow / utils.ts / RelationGraph / StackTraceGraph の挙動テストがない。 isInProject (utils.ts:16-20) は分岐があるので低コストでテスト追加可能。

### Q7: rust-version = "1.77" は十分新しいが (Cargo.toml:7)、 MSRV 動作確認の CI job がない

stable のみ。 1.77 specific feature を踏んだ場合の検知ができないので MSRV job を 1 つ足すか、 README で動作確認 toolchain を明記。

### Q8: コミット履歴は丁寧で issue link が紐付いている (`#11`, `#7,#8`, `#5` 等)

v0.2 までで 16 commits、 PR ベース運用が定着。 1 PR = 1 機能のサイズ感も適正。

## 評価まとめ

- ドキュメント: A
- テスト: A-
- CI: A
- コード品質: B+ (`unwrap` の多用が惜しい)
- 法務: A
