# Iter — REVIEW_IMPLEMENTATION (2026-05-13)

## 総合評価: A-

Rust 側は LSP JSON-RPC framing、 stderr ring、 wait task の死活監視と pending drain、 LocationLink 正規化など実用域。 TypeScript 側も debounce + abort flag、 cross-window event 制御が手堅い。 単体テストも各モジュールに添えられている。 細部に短期的な regression リスクがあるので個別に指摘する。

## 良い点

- **JSON-RPC framing 自前実装が正攻法** (lsp.rs:400-440): Content-Length ヘッダ → body 読み込み、 EOF を Ok(None) で素直に返す。 prev/prev2/prev3 byte で `\r\n\r\n` 検出も妥当。
- **`textDocument/definition` のレスポンス 3 形態を 1 関数で正規化** (lsp.rs:307-351): Location / Location[] / LocationLink[] を `from_link` 補助で `Vec<Location>` に統一。 targetSelectionRange を優先する選択も LSP spec に即している。
- **cache signature が path-mtime hash + sort** (cache.rs:126-137): root mtime の罠 (README 追加で invalidate) を回避し、 relevant 拡張子のみで安定。 tests (cache.rs:264-327) で「子 dir 追加」「irrelevant 無視」「blocklist skip」「source modify」をカバー済。
- **stack trace parser のフォーマットカバレッジ** (stack_trace.rs:51-128): GCC sanitizer / GDB / V8 / Python / Rust の混在 input をテスト (stack_trace.rs:262-271) しており実用的。
- **window 重複オープンを label hash で防止** (window.rs:56-71): 既存ウィンドウなら focus + open-at event を投げるだけで再利用。

## 改善点

### I1: `read_snippet` が path traversal 無防備 (snippet.rs:18-33)

`#[tauri::command] read_snippet(path, line, context)` がそのまま `std::fs::read_to_string(&path)` に渡る。 capabilities の fs:scope で守られているとは言え、 Tauri 2 の plugin-fs scope と `std::fs` 経由は別系統 (scope ガードは `plugin-fs` の crate 側でのみ効く)。 Rust 側の自前 command でファイル読みする場合は scope check が効かないため、 project_root 配下 + symlink 拒否のガードを足したい。

### I2: stderr ring が Self に保持されない (lsp.rs:167-168)

`drop(stderr_ring)` で明示的に手放しており、 wait task と reader task の Arc clone 2 件のみが ring を保持する。 reader task が EOF で終了すると参照は wait task の 1 件、 wait 完了で 0 になる。 結果として「死亡通知後に再度 stderr を取りたい」フローが追加されると ring が消えている。 現状は Phase 2 MVP として問題ないが、 client が retrievable に stderr を持つ設計に寄せる方が将来安全。

### I3: `Hash` が DefaultHasher のままで衝突耐性が弱い (cache.rs:82-86 / window.rs:102-106)

`DefaultHasher::new()` (SipHash) は cryptographic ではないので 64-bit truncate で project root path / file path のハッシュ衝突は理論上ある。 cache は誤ヒットすると project_root を取り違える危険、 window label は別 path に同じ label が当たると open_at 不能。 path 文字列を SHA-256 → hex 先頭 16 にする方が確実。

### I4: `lsp.rs:118` で notification (id 無し) を黙って捨てる

`publishDiagnostics` などの sever-initiated notification を全て無視。 phase 2 MVP では diagnostics 表示が無いので妥当だが、 README の Phase 2 完了条件 (callHierarchy グラフ表示) には影響なし。 将来追加時の TODO コメントとして残るとよい。

### I5: `compile_db.rs:121` で cmake 失敗時の stderr を string lossy で握りつぶす

`Err(format!("cmake 失敗 (exit {}): {}", output.status, stderr))` まではいいが、 ensure_virtual_compile_commands:91-95 で `if let Ok(cc) = ...` と silently ignore して直書きにフォールバックする。 ユーザに「cmake は失敗したのでフォールバック実行中」と伝わらない。 emit 経由でフロントに warn を流したい。

### I6: `parse_path_line_col` の Windows drive 判定が drive letter 1 文字限定 (stack_trace.rs:285-300)

ドライブレターは ASCII 1 文字を想定するが、 `D:\foo\bar.cpp:42:5` のように 3 分割される時に `parts[0]` 長さ 1 + ASCII で判別している。 これは想定通り動くが、 仮に UNC path `//server/share/foo.cpp:42:5` が来ると 4 分割以上になり None を返す。 UNC 対応は仕様外でも、 コメントとして残すと良い。

### I7: FileWindow.tsx:188-211 の debounce 用 ref が cleanup されない

`queryDebounceRef.current = window.setTimeout(...)` 後、 `useEffect cleanup` で `clearTimeout` が無く、 unmount 時に古い timer が走る。 path 変更直後の component unmount で setRelationData が unmount 後に呼ばれて React 警告になり得る。 単純な追加で済むので AUTOFIX 候補。

## まとめ

実装の骨格は強い。 I1 (path traversal) と I3 (hash 衝突) は将来のセキュリティ regression を防ぐ意味で優先度高。 残りは Phase 3 で潰せばよい。
