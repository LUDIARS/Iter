# Iter — REVIEW_DESIGN (2026-05-13)

## 総合評価: B+

DESIGN.md と ADR 5 本で「何を」「なぜ」が明確に分かれている。一枚図でレイヤ責務 (Control Panel / File Window / Tauri commands / ClangdClient / cache / compile_db) が整理され、 ADR-001〜005 で Tauri 2 採用・clangd 採用・React Flow・cache signature・multi-window の 5 つの主要決定が記録されている (DESIGN.md:59-65)。MVP→Phase 2 のスコープ境界も README.md:34-46 で言語化されており、 v0.2 時点での到達点が分かりやすい。

## 良い点

- **ADR が決定論的に並ぶ**: 各 ADR が単一論点 (採用技術 / cache signature 形式 / window 形態) に絞られ、 後追い修正に強い (docs/adr/0001-tauri-2.md 等)。
- **責務分離が clean**: `compile_db` (compile_commands.json 生成) と `lsp` (clangd プロセス管理) と `lsp_commands` (Tauri command 層) が明示分割されており、 1 ファイル = 1 役割を維持 (src-tauri/src/lib.rs:1-12)。
- **cache signature の根拠が文書化**: ADR-004 と cache.rs:120-137 の両方で「root mtime ではなく relevant file (path, mtime) の sorted hash」とした理由が説明されている。

## 改善余地

- **C# (csproj) のスコープ未定義** (DESIGN.md:46-56 / project.rs:42): `Csproj` BuildSystem を検知するが clangd 対象外で、 frontend にどうフォールバックさせるかが DESIGN 側で言語化されていない。 ファイルツリー閲覧のみ可、 LSP 機能なし、 という UX 仕様が必要。
- **シングルトン LSP の制約**: lsp_commands.rs:23-27 で `LspState { client, project_root }` が単一プロジェクトのみ保持する設計だが、 ユーザ操作で「別 root を選び直す」遷移 (旧 client の graceful shutdown) が ADR に書かれていない。 現実装は旧 client を Arc drop に任せて新 client を spawn する暗黙挙動 (lsp_commands.rs:48-49)。
- **fs:scope の動的拡張仕様**: capabilities/default.json:18-31 の static deny (`$HOME/.ssh/**` 等) が、 runtime の `expand_fs_scope` (lsp_commands.rs:59-68) で上書きされる順序が DESIGN に未記載。 root が `~/.ssh` を含むケースの扱いが不明瞭。
- **multi-window 間の状態共有**: ADR-005 では「Rust 共有 + Tauri events」と書かれ get_project_root + iter://relations-changed で実現済だが、 編集中バッファの cross-window 衝突 (同じファイルを 2 つの window で開く) の解決方針が未記載 (window.rs は label hash で再利用するが、 race の言及なし)。

## 設計改善提案 (実装変更を伴わない、 文書追記レベル)

1. ADR-006 として「LSP 単一プロジェクト前提と切替時の挙動」を追加。
2. DESIGN.md にエラーパス一覧 (clangd not found / cmake not found / fs scope denied) と UX の対応表を追加。
3. csproj 系の「閲覧のみモード」を README.md の MVP 表に明示。
