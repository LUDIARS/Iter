/**
 * 関連グラフ。React Flow 上に **専用の RelationCard** をノードとして描画する。
 * Monaco は使わず、`read_snippet` で取得した前後 5 行 (既定) のスニペットを
 * 軽量に表示するだけ。クリックで `open_at` → 該当行を Monaco で開く。
 *
 * 100 件超の対策:
 *   - 合計表示数を 100 上限にトリム (`MAX_TOTAL_CARDS`)
 *   - スニペット取得時のコンテキスト行 (`SNIPPET_CONTEXT`) も負荷に応じて自動で
 *     2 行まで減らす (探索範囲短縮)
 *   - 切り捨て分の件数は subtitle で表示
 */
import { useEffect, useMemo, useState } from "react";
import { ReactFlow, Background, Controls, type Node, type Edge } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { fs, type CallHierarchyResult, type LspLocation, uriToPath } from "./lsp";
import {
  RelationCard,
  ensureCardStyles,
  type RelationCardData,
  type RelationVariant,
} from "./RelationCard";

export interface RelationData {
  origin: { name: string; path: string; line: number };
  callHierarchy: CallHierarchyResult | null;
  references: LspLocation[];
}

interface Props {
  data: RelationData | null;
  show: { callers: boolean; callees: boolean; references: boolean };
  projectRoot: string | null;
}

const MAX_TOTAL_CARDS = 100;
const DEFAULT_SNIPPET_CONTEXT = 5;
const REDUCED_SNIPPET_CONTEXT = 2;

const CARD_WIDTH = 340;
const CARD_HEIGHT = 180;
const COL_GAP = 80;
const ROW_GAP = 30;

const nodeTypes = { card: RelationCard };

interface RawTarget {
  variant: RelationVariant;
  symbol: string | null;
  path: string;
  line: number;
  inProject: boolean;
}

export function RelationGraph({ data, show, projectRoot }: Props) {
  const [graph, setGraph] = useState<{ nodes: Node[]; edges: Edge[]; trimmed: number }>({
    nodes: [],
    edges: [],
    trimmed: 0,
  });

  useEffect(() => {
    ensureCardStyles();
  }, []);

  // raw target 抽出 (snippets 取得前)
  const targets = useMemo<RawTarget[]>(() => {
    if (!data) return [];
    const arr: RawTarget[] = [];
    if (show.callers && data.callHierarchy) {
      for (const c of data.callHierarchy.incoming) {
        const p = uriToPath(c.from.uri);
        arr.push({
          variant: "caller",
          symbol: c.from.name,
          path: p,
          line: c.from.range.start.line,
          inProject: isInProject(p, projectRoot),
        });
      }
    }
    if (show.callees && data.callHierarchy) {
      for (const c of data.callHierarchy.outgoing) {
        const p = uriToPath(c.to.uri);
        arr.push({
          variant: "callee",
          symbol: c.to.name,
          path: p,
          line: c.to.range.start.line,
          inProject: isInProject(p, projectRoot),
        });
      }
    }
    if (show.references) {
      for (const r of data.references) {
        const p = uriToPath(r.uri);
        arr.push({
          variant: "reference",
          symbol: null,
          path: p,
          line: r.range.start.line,
          inProject: isInProject(p, projectRoot),
        });
      }
    }
    return arr;
  }, [data, show, projectRoot]);

  // snippet 取得 + node/edge 組み立て
  useEffect(() => {
    if (!data) {
      setGraph({ nodes: [], edges: [], trimmed: 0 });
      return;
    }
    let cancelled = false;
    const requestedTotal = targets.length + 1; // +1 = origin
    const trimmed = Math.max(0, requestedTotal - MAX_TOTAL_CARDS);
    const cap = Math.max(0, MAX_TOTAL_CARDS - 1); // origin 抜きの cap
    const trimmedTargets = trimmed > 0 ? targets.slice(0, cap) : targets;
    const ctx =
      trimmedTargets.length > 60 ? REDUCED_SNIPPET_CONTEXT : DEFAULT_SNIPPET_CONTEXT;

    (async () => {
      // origin スニペット
      const originSnippetP = fs
        .readSnippet(data.origin.path, data.origin.line, ctx)
        .catch(() => null);
      // 他カードのスニペットを並列に取得
      const targetSnippetsP = Promise.all(
        trimmedTargets.map((t) =>
          fs.readSnippet(t.path, t.line, ctx).catch(() => null),
        ),
      );

      const [originSnippet, snippets] = await Promise.all([
        originSnippetP,
        targetSnippetsP,
      ]);
      if (cancelled) return;

      const nodes: Node[] = [];
      const edges: Edge[] = [];

      const originData: RelationCardData = {
        variant: "origin",
        symbol: data.origin.name || "(cursor)",
        path: data.origin.path,
        line: data.origin.line,
        snippet: originSnippet?.lines ?? [],
        snippetStart: originSnippet?.start_line ?? data.origin.line,
        inProject: true,
        badge: "origin",
      };
      nodes.push({
        id: "origin",
        type: "card",
        data: originData,
        position: { x: 0, y: 0 },
        draggable: true,
      });

      // 列ごとにレイアウト
      const buckets: Record<RelationVariant, RawTarget[]> = {
        caller: [],
        callee: [],
        reference: [],
        frame: [],
        origin: [],
        custom: [],
      };
      const snippetByIndex = snippets;
      trimmedTargets.forEach((t) => buckets[t.variant].push(t));

      const callerSlot = -CARD_WIDTH - COL_GAP;
      const calleeSlot = CARD_WIDTH + COL_GAP;
      const refSlot = 0;

      buckets.caller.forEach((t, i) => {
        const idx = trimmedTargets.indexOf(t);
        const snippet = snippetByIndex[idx];
        const id = `caller-${i}`;
        nodes.push({
          id,
          type: "card",
          data: makeCard(t, snippet),
          position: {
            x: callerSlot,
            y: -((buckets.caller.length - 1) * (CARD_HEIGHT + ROW_GAP)) / 2 +
              i * (CARD_HEIGHT + ROW_GAP),
          },
          draggable: true,
        });
        edges.push(makeEdge(`e-${id}`, id, "origin", "#5eb2ff", "calls"));
      });

      buckets.callee.forEach((t, i) => {
        const idx = trimmedTargets.indexOf(t);
        const snippet = snippetByIndex[idx];
        const id = `callee-${i}`;
        nodes.push({
          id,
          type: "card",
          data: makeCard(t, snippet),
          position: {
            x: calleeSlot,
            y: -((buckets.callee.length - 1) * (CARD_HEIGHT + ROW_GAP)) / 2 +
              i * (CARD_HEIGHT + ROW_GAP),
          },
          draggable: true,
        });
        edges.push(makeEdge(`e-${id}`, "origin", id, "#7ddba2", "calls"));
      });

      buckets.reference.forEach((t, i) => {
        const idx = trimmedTargets.indexOf(t);
        const snippet = snippetByIndex[idx];
        const id = `ref-${i}`;
        nodes.push({
          id,
          type: "card",
          data: makeCard(t, snippet),
          position: {
            x: refSlot,
            y: CARD_HEIGHT + 80 + i * (CARD_HEIGHT + ROW_GAP),
          },
          draggable: true,
        });
        edges.push(
          makeEdge(`e-${id}`, "origin", id, "#efc56a", undefined, "4 2"),
        );
      });

      setGraph({ nodes, edges, trimmed });
    })();

    return () => {
      cancelled = true;
    };
  }, [data, targets]);

  if (!data) {
    return (
      <div style={{ padding: "1rem", color: "#6b7383", fontSize: "0.85rem" }}>
        ファイル中の関数や記号にカーソルを置くと caller / callee / 参照が表示されます
      </div>
    );
  }

  return (
    <div style={{ position: "relative", height: "100%" }}>
      {graph.trimmed > 0 && (
        <div
          style={{
            position: "absolute",
            top: 6,
            left: 6,
            zIndex: 10,
            background: "rgba(239, 197, 106, 0.15)",
            border: "1px solid #efc56a",
            color: "#efc56a",
            padding: "2px 8px",
            borderRadius: 3,
            fontSize: 11,
            fontFamily: "ui-monospace, Consolas, monospace",
          }}
        >
          表示上限 100 件 ({graph.trimmed} 件は省略)
        </div>
      )}
      <ReactFlow
        nodes={graph.nodes}
        edges={graph.edges}
        nodeTypes={nodeTypes}
        fitView
        proOptions={{ hideAttribution: true }}
        colorMode="dark"
        minZoom={0.2}
        maxZoom={1.5}
      >
        <Background color="#222831" gap={16} />
        <Controls showInteractive={false} />
      </ReactFlow>
    </div>
  );
}

function makeCard(
  t: RawTarget,
  snippet: { start_line: number; lines: string[] } | null,
): RelationCardData {
  return {
    variant: t.variant,
    symbol: t.symbol,
    path: t.path,
    line: t.line,
    snippet: snippet?.lines ?? [],
    snippetStart: snippet?.start_line ?? t.line,
    inProject: t.inProject,
  };
}

function makeEdge(
  id: string,
  source: string,
  target: string,
  color: string,
  label?: string,
  dash?: string,
): Edge {
  return {
    id,
    source,
    target,
    label,
    style: dash ? { stroke: color, strokeDasharray: dash } : { stroke: color },
    labelStyle: { fill: color, fontSize: 10 },
    labelBgStyle: { fill: "#11141a", fillOpacity: 0.8 },
  };
}

function isInProject(path: string, root: string | null): boolean {
  if (!root) return true;
  const norm = (s: string) => s.replace(/\\/g, "/").toLowerCase();
  return norm(path).startsWith(norm(root));
}
