# ADR-002: C/C++ 関連解析は clangd LSP に委ねる

**Status**: Accepted (2026-04, PR #5 で実装)

## Context

「caller / callee / references を取って graph で見せる」 が Iter のコアバリュー。
解析手段の候補:

| 候補 | できること | できないこと |
|---|---|---|
| **正規表現 / grep** | 即座、 軽量 | 同名関数の区別、 namespace 解決、 マクロ展開後の解析 |
| **tree-sitter** | 高速、 構文木が取れる | **意味解析 (シンボル解決) を自前で書く必要**。 caller/callee 解決には型情報が必要で実装大変 |
| **libclang 直叩き** | 完全な意味解析 | C++ AST API が複雑、 incremental が難しい |
| **clangd (LSP)** | callHierarchy / references / definition が標準で揃う、 incremental 解析、 multi-file context | 別プロセス起動コスト、 compile_commands.json 必須 |

## Decision

**clangd を LSP として子プロセス起動** + `textDocument/*` を JSON-RPC で呼ぶ。

- `prepareCallHierarchy` → `incomingCalls` / `outgoingCalls`
- `references`
- `definition` (PR #18 で実装、 #11 M2)

`compile_commands.json` は `compile_db::ensure_compile_commands` が CMake (`-DCMAKE_EXPORT_COMPILE_COMMANDS=ON`) で生成、 CMakeLists 不在時は `.iter/CMakeLists.txt` を仮想生成して fallback (PR #16)。

## Consequences

- **+ 意味解析の品質** — 同名関数のオーバーロード、 namespace、 inline、 マクロ展開後の caller も clangd が正しく解いてくれる。
- **+ multi-language の入口** — 同じ LSP の上に rust-analyzer / typescript-language-server を後から差し替えできる (将来)。
- **- clangd の起動コスト** — 数百 MB のインデックスを構築するので最初の `prepareCallHierarchy` まで数秒〜10 秒。 frontend は spinner で表現。
- **- プロセス死活** — clangd が SIGSEGV する事象が稀にある。 PR #14 で watchdog + stderr piped を実装し `iter://lsp-down` イベントで UI に通知。
- **- C++ 以外** — C# (.csproj/.sln) 検知は project.rs で対応するが clangd 対象外 (#15 でファイル覧覧のみ可)。 OmniSharp 等の追加は将来。
