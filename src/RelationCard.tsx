/**
 * 関連グラフのカード本体 (変種を許容する base)。
 *
 * - Monaco を使わず、軽量な `<pre>` で前後 N 行のコードを表示
 * - クリックで in-project なら `open_at`、外なら no-op
 * - variant prop で badge 色とラベルを差し替え (caller / callee / reference / frame / custom)
 *
 * 設計方針:
 *   - DOM は header (1 行) + body (snippet) のシンプル 2 行構成
 *   - スタイルは inline + class で、テーマや variant 追加は data drivien に拡張可能
 *   - LSP 結果はフロント側で fetch → snippet を data に詰めてから React Flow に渡す
 *     ので、このコンポーネントは「データを描画するだけ」の責務に固定する
 */
import { memo } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";
import { win } from "./lsp";

export type RelationVariant =
  | "caller"
  | "callee"
  | "reference"
  | "frame"
  | "origin"
  | "custom";

export interface RelationCardData {
  /** カード種別。色とラベル選び用 */
  variant: RelationVariant;
  /** ヘッダ左に出す関数名 / シンボル名。なければ '<anon>' */
  symbol?: string | null;
  /** 表示するファイルパス */
  path: string;
  /** 0-based の対象行 */
  line: number;
  /** 取得済みのスニペット行 (前後 ±context) */
  snippet: string[];
  /** snippet の最初の行が file 中の何行目か (0-based) */
  snippetStart: number;
  /** プロジェクト外なら disabled (クリック不可、文字色グレー) */
  inProject: boolean;
  /** 1 行 badge (caller/callee/refs/#0 等)。variant の自動値を上書きしたい時に使う */
  badge?: string;
  [key: string]: unknown;
}

const ACCENT_BY_VARIANT: Record<RelationVariant, string> = {
  caller: "#5eb2ff",
  callee: "#7ddba2",
  reference: "#efc56a",
  frame: "#9a8cff",
  origin: "#4a7afe",
  custom: "#a0a9b8",
};

const LABEL_BY_VARIANT: Record<RelationVariant, string> = {
  caller: "caller",
  callee: "callee",
  reference: "ref",
  frame: "frame",
  origin: "origin",
  custom: "",
};

export const RelationCard = memo(function RelationCard(
  props: NodeProps,
) {
  const data = props.data as unknown as RelationCardData;
  const accent = ACCENT_BY_VARIANT[data.variant] ?? ACCENT_BY_VARIANT.custom;
  const label = data.badge ?? LABEL_BY_VARIANT[data.variant];
  const fname = data.path.split(/[\\/]/).pop() ?? data.path;
  const fileTitle = `${fname}:${data.line + 1}`;

  const handleClick = () => {
    if (!data.inProject) return;
    void win.openAt(data.path, data.line, 0, false);
  };

  return (
    <div
      onClick={handleClick}
      className={data.inProject ? "iter-card iter-clickable" : "iter-card iter-disabled"}
      style={{
        ["--card-accent" as string]: accent,
      }}
      title={data.path}
    >
      <Handle type="target" position={Position.Left} style={{ opacity: 0 }} />
      <div className="iter-card-head">
        {label && <span className="iter-card-badge">{label}</span>}
        <span className="iter-card-symbol">{data.symbol ?? "<anon>"}</span>
        <span className="iter-card-loc">{fileTitle}</span>
      </div>
      <pre className="iter-card-body">
        {data.snippet.map((s, i) => {
          const ln = data.snippetStart + i;
          const isTarget = ln === data.line;
          return (
            <div
              key={i}
              className={isTarget ? "iter-card-line target" : "iter-card-line"}
            >
              <span className="iter-card-lineno">{ln + 1}</span>
              <span className="iter-card-linetext">{s || " "}</span>
            </div>
          );
        })}
      </pre>
      <Handle type="source" position={Position.Right} style={{ opacity: 0 }} />
    </div>
  );
});

/** 全 RelationCard の共通スタイルを 1 度だけ document に流し込む。 */
export function ensureCardStyles() {
  const id = "iter-card-style";
  if (document.getElementById(id)) return;
  const s = document.createElement("style");
  s.id = id;
  s.textContent = `
    .iter-card {
      width: 320px;
      background: #11141a;
      border: 1px solid var(--card-accent, #4a7afe);
      border-radius: 4px;
      font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
      font-size: 11px;
      color: #c9d3e0;
      overflow: hidden;
      box-shadow: 0 1px 2px rgba(0,0,0,0.4);
    }
    .iter-clickable { cursor: pointer; }
    .iter-clickable:hover { background: #14181f; }
    .iter-disabled { cursor: not-allowed; opacity: 0.5; }
    .iter-card-head {
      display: flex;
      align-items: center;
      gap: 0.4rem;
      padding: 4px 8px;
      background: rgba(255,255,255,0.03);
      border-bottom: 1px solid rgba(255,255,255,0.06);
      font-size: 11px;
    }
    .iter-card-badge {
      background: var(--card-accent, #4a7afe);
      color: #0e0f12;
      font-weight: 700;
      font-size: 9px;
      padding: 1px 4px;
      border-radius: 2px;
      letter-spacing: 0.04em;
      text-transform: uppercase;
      flex-shrink: 0;
    }
    .iter-card-symbol {
      color: var(--card-accent, #4a7afe);
      font-weight: 600;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
      flex: 1;
    }
    .iter-card-loc {
      color: #6b7383;
      font-size: 10px;
      flex-shrink: 0;
    }
    .iter-card-body {
      margin: 0;
      padding: 4px 0;
      max-height: 130px;
      overflow: hidden;
      background: #0e0f12;
    }
    .iter-card-line {
      display: flex;
      gap: 0.5rem;
      padding: 0 8px;
      line-height: 1.45;
      white-space: pre;
    }
    .iter-card-line.target {
      background: rgba(74, 122, 254, 0.15);
    }
    .iter-card-lineno {
      color: #4a4f5a;
      min-width: 2.5em;
      text-align: right;
      flex-shrink: 0;
    }
    .iter-card-linetext {
      color: #c9d3e0;
      flex: 1;
      min-width: 0;
      overflow: hidden;
      text-overflow: ellipsis;
    }
    .react-flow__attribution { display: none; }
  `;
  document.head.appendChild(s);
}
