# Iter

ソースコードの関連性 (callers / callees / references) を AST 解析ベースで
グラフ表示するエディタ。Tauri 2 + React + Monaco + (Phase 2) clangd LSP。

## アーキテクチャ

```
+---------------------+      +---------------------+
| Control Panel       |      | File Window (n)     |
| - project picker    |      | - Monaco editor     |
| - file tree         |─────▶| - 検索 (常時 + Ctrl+F) |
| - relation toggles  |      | - 保存 (Ctrl+S)      |
+---------------------+      +---------------------+
        │ invoke()                  ▲
        ▼                           │
+----------------------------------------+
| Tauri 2 backend (Rust)                 |
| - project::detect_project              |
| - window::open_file_window             |
| - (Phase 2) lsp_client → clangd        |
+----------------------------------------+
```

## 開発

```bash
npm install
npm run tauri dev
```

> **Note**: 初回 `npm run tauri dev` は Rust 依存をビルドするため数分かかる。

## MVP (Phase 1) で動くこと

- [x] CMakeLists.txt / *.vcxproj 検知
- [x] ファイルツリー表示 (Control Panel)
- [x] ファイルクリックで個別ウインドウに Monaco Editor で開く
- [x] 常時表示の検索バー (Ctrl+F でフォーカス + Monaco 上にハイライト)
- [x] 編集・Ctrl+S で保存
- [ ] (Phase 2) `compile_commands.json` 生成 + clangd spawn
- [ ] (Phase 2) callHierarchy で caller/callee を React Flow ノードで周囲に表示
- [ ] (Phase 2) Control Panel チェックボックスで表示種別切替

## ファイル配置

| パス | 役割 |
|---|---|
| `src/ControlPanel.tsx` | プロジェクト選択 + ファイルツリー + 関連表示トグル |
| `src/FileWindow.tsx` | Monaco エディタ + 検索 + 保存 |
| `src-tauri/src/project.rs` | CMakeLists/vcxproj 検知 + ファイル走査 |
| `src-tauri/src/window.rs` | 個別ファイルウインドウの生成 |
| `src-tauri/capabilities/default.json` | Tauri 2 permissions (MVP は緩め、Phase 2 で project root に絞る) |

## ライセンス

- Iter 本体: MIT (詳細は [LICENSE](LICENSE))
- 依存ライブラリの帰属表示: [NOTICE](NOTICE)
- 第三者ライセンス目録: [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md)
