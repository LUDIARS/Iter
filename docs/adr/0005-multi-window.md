# ADR-005: ファイルは「タブ」 ではなく **OS の独立 WebView ウインドウ**

**Status**: Accepted (2026-04, PR #4)

## Context

VS Code / IntelliJ はファイルをタブで開く。 Iter のユースケースでは
「caller を開いたまま callee の中身を見たい」 「関数本体と RelationGraph を
横並びで見たい」 のような並べ替えが多い。 候補:

| 候補 | 長所 | 短所 |
|---|---|---|
| **タブ式 (単一 window)** | 慣れた UX | 並列表示できない、 dual monitor 活用が難しい |
| **分割ペイン** | 単一ウインドウ内で並列 | ペイン管理が複雑、 grid lock-in |
| **独立ウインドウ (1 file = 1 window)** | OS のウインドウ管理に任せられる、 dual monitor 自然 | cross-window state が必要、 window 数管理 |

## Decision

**1 ファイル = 1 WebView ウインドウ**。 `window::open_file_window(path)` で開く。
キャップは設けず、 `Ctrl+Shift+W` で「自分以外を全部閉じる」を提供 (`close_other_windows`)。

cross-window で共有する state:
- `project_root` → Rust 側 `LspState.project_root` (PR #18, #11 M6)
- それ以外 (search state 等) は各 window 独立で OK

## Consequences

- **+ dual monitor 親和性** — caller を片方の画面、 callee をもう片方で見る運用が自然。
- **+ Ctrl+Shift+W の単純さ** — 「整理したい」 を 1 ショートカットで実現できる。
- **- ウインドウ間の同期** — `project_root` 1 つだけは Rust 共有 state にまとめる必要があった (#11 M6)。 future では「ファイル保存通知」 「LSP イベント」 も Tauri events 経由で全ウインドウに broadcast する予定。
- **- 起動コスト** — 各ウインドウが WebView を立ち上げるので、 数十枚開くとメモリが嵩む。 ただし v0.2 段階では 10 枚程度の運用を想定。
