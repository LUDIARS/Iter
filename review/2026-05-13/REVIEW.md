# Iter — REVIEW Summary (2026-05-13)

## 総合評価: A- (weighted_score 82)

LUDIARS/Iter v0.2 は Tauri 2 + React + Monaco + clangd LSP のソースコード関連性ビジュアライザ。 README / DESIGN / ADR 5 本が整い、 Rust 8 モジュール + TS 7 ファイル + CI 3 OS matrix + criterion bench までスケルトン完成度が高い。 Phase 2 (callHierarchy / cache signature / multi-window) もほぼ実装到達済み。

## 各レビュー結果

| カテゴリ | 評価 | 主要ファイル |
|---|---|---|
| DESIGN     | B+ | DESIGN.md / docs/adr/0001-0005 |
| VULN       | B  | capabilities/default.json / lsp_commands.rs / snippet.rs |
| IMPL       | A- | lsp.rs / cache.rs / stack_trace.rs |
| MISSING    | B  | README.md / FileWindow.tsx (didChange) |
| QUALITY    | A- | CI matrix / unwrap 散在 / tests |

## 主要所見 (Top 5)

1. **path traversal リスク (Medium)**: `read_snippet` (snippet.rs:18-33) が plugin-fs scope を経由せず `std::fs::read_to_string` を直接呼ぶ。 project_root 配下 + symlink 拒否ガードが必要。
2. **clangd 編集同期未実装 (Medium)**: FileWindow.tsx:261-269 の save 後に `textDocument/didChange/didSave` を飛ばさないため relation 結果が stale。
3. **README が現状を反映していない (Medium)**: README.md:44-46 の Phase 2 `[ ]` 3 件は実装済。
4. **fs:scope 動的拡張の deny 順序が文書化されてない (Medium)**: expand_fs_scope (lsp_commands.rs:59-68) が ~/.ssh 等を含む root を選んだ場合の挙動が ADR に無い。
5. **`unwrap()` の多用 (Low-Medium)**: lsp.rs に 8 箇所、 panic より `LspError::Rpc` 返却に統一したい。

## 件数

- 検出 issue 合計: 26 件 (Design 4 + Vuln 5 + Impl 7 + Missing 8 + Quality 8、 概算)
- High: 0、 Medium: 6、 Low: 18、 Info: 2
- autofix_count: 0 (ソースコード修正禁止)

## 推奨次手

- README 同期 (15 分)
- `read_snippet` の path ガード追加 (30 分)
- `expand_fs_scope` の sensitive directory チェック (30 分)
- `lsp.rs` の `unwrap` を `?` 化 (1 時間)
- `didChange/didSave` 実装 (1-2 時間)
