# ADR-003: グラフ描画に React Flow (`@xyflow/react`) を採用

**Status**: Accepted (2026-04, PR #5)

## Context

「caller を上、 callee を下、 references を右」 のような関連を **ノードカードで** 表示したい。 ノードの中身は自前の `RelationCard` (パス / 行番号 / コードスニペット表示)。 候補:

| 候補 | 長所 | 短所 |
|---|---|---|
| **D3 直書き** | 完全自由、 軽量 | React コンポーネントをノードに使えない、 layout 自分で実装 |
| **Cytoscape.js** | 機能豊富、 layout 多数 | カスタムノードに自前 React を埋めるのが面倒、 styling は CSS-in-Cytoscape |
| **React Flow (`@xyflow/react`)** | React コンポーネントをノードに使える、 自動 layout 不要なケースで超軽量、 v12 系で xyflow に rebrand | layout アルゴリズム (dagre 等) は追加が要る、 数千ノードで重くなる |
| **Mermaid** | 設計図的描画にはベスト | 動的更新 / クリック interact が弱い |

## Decision

`@xyflow/react` (v12.3+) を採用。 `RelationCard` を `nodeTypes` に登録、 caller/callee は固定 layout (caller 群を上、 callee 群を下、 references を右) で配置。 layout アルゴリズム (dagre / ELK) は **入れない**。 トリム上限 `MAX_TOTAL_CARDS = 100` で大規模を回避。

## Consequences

- **+ React 統合** — `RelationCard.tsx` で素直に React/CSS を書ける。 hover / click / keyboard が普通の React イベント。
- **+ 軽量初期構成** — 自動 layout 無しなので初期 bundle が小さい (`@xyflow/react` core のみ)。
- **- 大規模 graph 不向き** — caller が 100 を超える場合は trim + 「+N more」 表示で済ます。 LSP / clangd 自体が遅くなる方が先にボトルネック。
- **- layout は手動** — 関連の種類 (caller / callee / ref) を増やすときレイアウトロジックを足す必要。 これは関連が拡大したらタイミングを見て layout エンジン入れる候補。
