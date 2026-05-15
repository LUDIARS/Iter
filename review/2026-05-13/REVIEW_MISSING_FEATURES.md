# Iter — REVIEW_MISSING_FEATURES (2026-05-13)

## 総合評価: B

README.md:34-46 の MVP チェックリストで [x] が並んでおり MVP は通過、 Phase 2 の [ ] が 3 件 (`compile_commands.json` 自動生成・callHierarchy 周辺ノード描画・Control Panel チェックボックス切替) と言及されているが、 実コードでは Phase 2 系の relation graph / `lsp_call_hierarchy` 等は既に実装済。 README が現状にやや追い付いていない。

## MVP / Phase 2 のギャップ

### M1 (High): README が Phase 2 完了状況を反映していない

README.md:44-46 の `[ ]` 3 件は実コード上 `lsp_call_hierarchy` (lsp_commands.rs:94-123)、 `compile_db::ensure_compile_commands` (compile_db.rs:38-47)、 `RELATION_KINDS` トグル (ControlPanel.tsx:25-29) で実現されている。 README を更新するか、 DESIGN.md の到達点に揃える。

### M2 (Medium): csproj 系の体験が薄い

project.rs:42 で `BuildSystem::Csproj` を検知するが、 clangd 起動条件は `if (info.build_system === "cmake")` (ControlPanel.tsx:62) のみ。 csproj/sln 単体を選んだ場合に「LSP は idle のまま、 ファイル閲覧のみ可」となるが、 UI 上の説明がなく、 ユーザは LSP failed と混同しがち。 build_system に応じて「この build system では LSP 解析は無効」とバッジ説明が欲しい。

### M3 (Medium): 編集保存後の clangd 同期がない

FileWindow.tsx:261-269 の `save` は writeTextFile するだけで、 clangd への `textDocument/didChange` / `didSave` が飛ばない。 結果として保存後の relation 結果は stale な解析を返す。 LSP 同期 (full text didChange or didSave) の実装が未着手。

### M4 (Medium): Phase 2 の "Control Panel チェックボックスで表示種別切替" は半分のみ

`emit("iter://relations-changed", ...)` で broadcast はしているが (ControlPanel.tsx:96)、 callees/callers/references の各 toggle 単独で graph node の表示が変わる動作は RelationGraph 側の実装次第。 README には「toggle で切替」だけで仕様詳細が不明。

### M5 (Low): clangd 死亡後の自動 restart が無い

ADR-002 と lsp.rs:130-165 で死活監視 + `iter://lsp-down` emit は実装済だが、 frontend で listen して再起動を促す UI 動線が ControlPanel に無い。 「LSP failed」表示後、 手動で再度プロジェクトを開き直す必要がある。

### M6 (Low): スタックトレース起点のグラフが project 内外で違う扱いになるが、 ボタンが「グラフ化」のみで再ロードしないと反映されない

ControlPanel.tsx:101-104 の onParseStack が textarea 変更で auto re-parse しない (debounce すべき)。 UX 上の細かい話。

### M7 (Low): bench の regression baseline が CI に未接続

DESIGN.md:67-73 で「regression が出たら CI で検出する (ベースライン比較は今後 #11 の M7 follow-up)」と既知。

### M8 (Low): undo/redo / autosave / dirty indicator がない

Monaco には built-in undo があるが、 ファイル変更後の dirty indicator (タイトルの `*`) や離脱前 confirm が無く、 ユーザが閉じる前に書き忘れ得る。

## 推奨優先順位

1. M3 (didChange/didSave): relation graph の正しさに直結 — high
2. M2 (csproj UI 表示): MVP の説明責任 — medium
3. M1 (README 同期): 30 分タスク — medium
4. M5/M8: UX 改善 — low
