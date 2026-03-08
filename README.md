# Iter — Relay Graph Editor

コンパイラエラーをリレーショナルグラフとして可視化するインタラクティブなエディタです。
エラーに関連するコードエンティティ（関数、型、変数、インクルード）の関係性をグラフで視覚的に把握できます。

## 機能概要

- **コンパイラエラー解析** — GCC/Clang、MSVC、Unity C# のエラー形式に対応
- **AST 解析** — libclang を利用してエラー箇所のシンボルと依存関係を抽出
- **グラフ可視化** — ノードの展開/折りたたみ、ズーム、パンに対応したインタラクティブな表示
- **レイアウトアルゴリズム** — Sugiyama 階層レイアウト（DAG）およびフォースディレクテッドレイアウト（循環グラフ）を自動選択
- **シンタックスハイライト** — syntect によるソースコードのハイライト表示
- **キャッシュ** — SQLite による AST・アセンブリ解析結果のキャッシュ

## 必要な環境

- **Rust** 1.70 以上（Edition 2021）
- **システムライブラリ**:
  - X11 開発ライブラリ（`libx11-dev` 等）
  - Cairo 開発ライブラリ（`libcairo2-dev` 等）
  - SQLite3（rusqlite にバンドル済み）
- **オプション**:
  - libclang 18.0 以上（AST 解析に使用、ランタイムロード）
  - llvm-objdump（アセンブリ解析に使用）

### Ubuntu/Debian でのインストール例

```bash
sudo apt install libx11-dev libcairo2-dev libclang-18-dev llvm-18
```

## ビルド方法

```bash
# リポジトリのクローン
git clone https://github.com/LUDIARS/Iter.git
cd Iter

# デバッグビルド
cargo build

# リリースビルド（最適化あり）
cargo build --release

# ビルドして実行
cargo run
```

ビルド成果物は `target/debug/relay-editor`（デバッグ）または `target/release/relay-editor`（リリース）に生成されます。

## 使い方

```bash
# エラー文字列を直接指定
relay-editor --error "<エラーメッセージ>" [--build-dir <ディレクトリ>] [--show-asm]

# コンパイラ出力をパイプで渡す
gcc main.c 2>&1 | relay-editor --pipe [--build-dir <ディレクトリ>]
```

### 操作方法

| 操作 | 説明 |
|------|------|
| 中ボタンドラッグ | グラフのパン（移動） |
| スクロール | ズームイン/アウト（0.1x〜5.0x） |
| ノードをクリック | ノードの展開/折りたたみ |
| Ctrl+Q | 終了 |

## プロジェクト構成

```
src/
├── main.rs              # エントリーポイント
├── core/                # コアモジュール
│   ├── types.rs         # 型定義（ノード、エッジ、グラフ等）
│   ├── config.rs        # 表示設定・定数
│   ├── error_parser.rs  # コンパイラエラー解析
│   └── orchestrator.rs  # グラフ構築のオーケストレーション
├── graph/               # グラフ表示モジュール
│   ├── graph_view.rs    # メインビュー（カメラ・ズーム）
│   ├── graph_node.rs    # ノード描画・ヒットテスト
│   ├── graph_edge.rs    # エッジ描画（ベジェ曲線）
│   ├── graph_layout.rs  # レイアウトアルゴリズム
│   └── animation.rs     # アニメーションユーティリティ
├── editor/              # エディタモジュール
│   └── editor_view.rs   # シンタックスハイライト付きコード表示
├── analysis/            # 解析モジュール
│   ├── ast_analyzer.rs  # libclang による AST 解析
│   └── assembly_analyzer.rs  # llvm-objdump によるアセンブリ解析
├── platform/            # プラットフォーム抽象化
│   ├── renderer.rs      # レンダラーインターフェース
│   ├── renderer_cairo.rs # Cairo レンダラー実装
│   └── window_x11.rs    # X11 ウィンドウ管理
└── cache/               # キャッシュモジュール
    └── cache_manager.rs # SQLite キャッシュ管理
```

## 依存ライブラリ

| ライブラリ | バージョン | 用途 |
|-----------|-----------|------|
| cairo-rs | 0.20 | 2D グラフィックス描画 |
| x11 | 2.21 | X11 ウィンドウ管理 |
| regex | 1 | エラーパターンマッチング |
| rusqlite | 0.32 | SQLite キャッシュ |
| syntect | 5 | シンタックスハイライト |
| clang-sys | 1 | libclang FFI バインディング |
| log / env_logger | 0.4 / 0.11 | ログ出力 |

## ライセンス

MIT License
