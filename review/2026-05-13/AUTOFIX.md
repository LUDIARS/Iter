# Iter — AUTOFIX 候補一覧 (2026-05-13)

**autofix_count: 0** (今回はレビューのみ、 ソースコード修正禁止指示につき列挙のみ)

## 安全に自動修正可能な候補 (将来実行用、 今回は実行しない)

### A1: README.md の Phase 2 チェックボックスを更新

- ファイル: README.md:44-46
- 内容: 既に実装済の項目を `[x]` に変更
  - `[x] (Phase 2) compile_commands.json 生成 + clangd spawn` (compile_db.rs:38-47 で実装済)
  - `[x] (Phase 2) callHierarchy で caller/callee を React Flow ノードで周囲に表示` (lsp_commands.rs:94-123)
  - `[x] (Phase 2) Control Panel チェックボックスで表示種別切替` (ControlPanel.tsx:25-29 + 90-99)

### A2: lsp.rs の `serde_json::to_value(params).unwrap()` を `?` 化

- ファイル: src-tauri/src/lsp.rs:212, 219, 237, 252, 264, 273, 296, 320
- 内容: `.unwrap()` → `.map_err(|e| LspError::Rpc(e.to_string()))?`

### A3: utils.ts の isInProject に unit test 追加

- ファイル: src/utils.test.ts (新規)
- 内容: null root / 大文字小文字 / バックスラッシュの 3 分岐をカバー

### A4: FileWindow.tsx の debounce timer cleanup

- ファイル: src/FileWindow.tsx:188-211
- 内容: useEffect cleanup で `clearTimeout(queryDebounceRef.current)` を呼ぶ

### A5: parse_stack_trace の入力サイズ上限

- ファイル: src-tauri/src/stack_trace.rs:27
- 内容: `if text.len() > 1_000_000 { return Vec::new(); }` を先頭に追加

### A6: capabilities/default.json の動的 scope 拡張順序を doc コメント化

- ファイル: src-tauri/capabilities/default.json:4 (description)
- 内容: 「ランタイム allow_directory は static deny より優先されない」ことを明示

### A7: stack_trace.rs の UNC path 非対応を doc コメント

- ファイル: src-tauri/src/stack_trace.rs:285 付近
- 内容: 「`//server/share` 形式の UNC は対象外」と明示

## 実行は次回以降の指示時

ユーザの「AUTOFIX 適用」指示があるまで、 上記いずれも実装しない。
