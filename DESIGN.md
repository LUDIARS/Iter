# Iter — DESIGN

ソースコードの関連性 (callers / callees / references) を AST 解析ベースで
グラフ表示するデスクトップエディタ。 本書は v0.2.x 時点のアーキテクチャと、
個別の設計決定 (ADR) の入口。

## 一枚図

```
                       +-------------------------+
                       | Control Panel (1 only)  |
                       | - project picker        |
                       | - file tree             |
                       | - relation toggles      |
                       | - stack-trace input     |
                       +-----------+-------------+
                                   | invoke()
                                   v
+----------------------------------------------------------+
| Tauri 2 backend (Rust)                                   |
|                                                          |
|   project::detect_project    --> cache::try_load/save    |
|   compile_db::ensure_compile_commands --> .iter/         |
|   lsp::ClangdClient (1 per project, async JSON-RPC)      |
|   lsp_commands::*                                        |
|   snippet::read_snippet                                  |
|   stack_trace::parse_stack_trace                         |
|   window::{open_file_window, open_at, close_others}      |
|                                                          |
|   shared state: LspState { client, project_root }        |
+----------------------------------------------------------+
                                   ^
                                   | invoke()
                                   |
                  +----------------+----------------+
                  |                                 |
                  v                                 v
       +---------------------+         +-----------------------+
       | File Window (N 個)  |         | File Window (N 個)    |
       | - Monaco editor     | <-----> | - RelationGraph (xy)  |
       | - search / save     |  open_  | - RelationCard nodes  |
       | - follow_definition |  at()   |   (read_snippet 経由) |
       +---------------------+         +-----------------------+
```

## レイヤ責務

| レイヤ | 主な仕事 |
|---|---|
| **Control Panel** | プロジェクト選択 + ファイル一覧 + relation 表示 toggle + stack trace 入力 |
| **File Window (multi)** | Monaco でファイル編集、検索、保存。 カーソル位置の relation 取得 → RelationGraph 描画 |
| **Tauri commands** | OS 層 (file IO / 子プロセス) を frontend に公開する 1 単位 = 1 関数 |
| **ClangdClient** | clangd プロセスとの JSON-RPC 双方向通信 + 死活監視 + stderr ring buffer |
| **cache** | `<config_dir>/iter/projects/<hash>.json` で `detect_project` 結果を保持 |
| **compile_db** | `compile_commands.json` を CMake で生成 or `.iter/` に仮想生成 |

## 設計判断のインデックス (ADR)

| # | テーマ | 要約 |
|---|---|---|
| [ADR-001](docs/adr/0001-tauri-2.md) | Tauri 2 を採用 | Electron 比で起動 / メモリ / セキュリティ全てで優位、 Rust IPC との親和性 |
| [ADR-002](docs/adr/0002-clangd-lsp.md) | C/C++ 解析は clangd LSP | tree-sitter のみだと意味解析 (caller/callee 解決) が不足、 clangd の callHierarchy で済む |
| [ADR-003](docs/adr/0003-react-flow.md) | グラフ描画は React Flow (@xyflow/react) | D3 直書きや Cytoscape 比で React コンポーネントとの統合が楽、 ノード = 自前 RelationCard を組める |
| [ADR-004](docs/adr/0004-cache-signature.md) | キャッシュ妥当性は relevant-file mtime hash | root mtime だけだと sub-dir 変更を見逃す。 source 拡張子に限定して走査負荷を抑える |
| [ADR-005](docs/adr/0005-multi-window.md) | File Window は **OS の子ウインドウ** (タブではなく) | Monaco + 関連グラフを並べて見るには独立ウインドウが効率的。 cross-window state は Rust 共有 + Tauri events |

## ホットパス と コスト感覚 (criterion bench で計測)

`cargo bench` で次の 2 系列を計測 (`src-tauri/benches/`):
- `cache_signature` — 100 / 500 / 2000 ファイル規模
- `snippet_read` — 1k / 100k 行 ファイル

regression が出たら CI で検出する (ベースライン比較は今後 #11 の M7 follow-up で wire-in)。

## 開発

```bash
npm install
npm run tauri dev   # 初回は Rust 依存ビルドで数分
npm test            # vitest (frontend)
cargo test --manifest-path src-tauri/Cargo.toml  # Rust
cargo bench --manifest-path src-tauri/Cargo.toml # criterion
```
