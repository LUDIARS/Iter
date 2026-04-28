/**
 * スタックトレースを 1 つの React Flow グラフとして表示。
 * - frame 1 つ = `RelationCard` (variant="frame")
 * - 上から下へ #0 → 最深 frame
 * - in-project はクリック → `open_at`
 * - 100 frame を超えたら超過分を捨て、コンテキスト行も短縮
 */
import { useEffect, useState } from "react";
import { ReactFlow, Background, Controls, type Node, type Edge } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { fs, type StackFrame } from "./lsp";
import { RelationCard, ensureCardStyles, type RelationCardData } from "./RelationCard";

interface Props {
  frames: StackFrame[];
}

const MAX_FRAMES = 100;
const DEFAULT_SNIPPET_CONTEXT = 5;
const REDUCED_SNIPPET_CONTEXT = 2;
const FRAME_HEIGHT = 200;

const nodeTypes = { card: RelationCard };

export function StackTraceGraph({ frames }: Props) {
  const [graph, setGraph] = useState<{ nodes: Node[]; edges: Edge[]; trimmed: number }>({
    nodes: [],
    edges: [],
    trimmed: 0,
  });

  useEffect(() => {
    ensureCardStyles();
  }, []);

  useEffect(() => {
    if (!frames.length) {
      setGraph({ nodes: [], edges: [], trimmed: 0 });
      return;
    }
    let cancelled = false;
    const trimmed = Math.max(0, frames.length - MAX_FRAMES);
    const used = trimmed > 0 ? frames.slice(0, MAX_FRAMES) : frames;
    const ctx = used.length > 60 ? REDUCED_SNIPPET_CONTEXT : DEFAULT_SNIPPET_CONTEXT;

    (async () => {
      const snippets = await Promise.all(
        used.map((f) =>
          // stack frame の line は 1-based、Rust 側は 0-based 想定なので -1
          fs.readSnippet(f.path, Math.max(0, f.line - 1), ctx).catch(() => null),
        ),
      );
      if (cancelled) return;

      const nodes: Node[] = [];
      const edges: Edge[] = [];

      used.forEach((f, i) => {
        const id = `frame-${i}`;
        const snip = snippets[i];
        const data: RelationCardData = {
          variant: "frame",
          symbol: f.function ?? "<anon>",
          path: f.path,
          line: Math.max(0, f.line - 1),
          snippet: snip?.lines ?? [],
          snippetStart: snip?.start_line ?? Math.max(0, f.line - 1),
          inProject: f.in_project,
          badge: `#${f.index}`,
        };
        nodes.push({
          id,
          type: "card",
          data,
          position: { x: 0, y: i * FRAME_HEIGHT },
          draggable: true,
        });
        if (i > 0) {
          edges.push({
            id: `e-${i}`,
            source: `frame-${i - 1}`,
            target: id,
            style: { stroke: "#9a8cff" },
          });
        }
      });

      setGraph({ nodes, edges, trimmed });
    })();

    return () => {
      cancelled = true;
    };
  }, [frames]);

  if (!frames.length) {
    return (
      <div style={{ padding: "1rem", color: "#6b7383", fontSize: "0.85rem" }}>
        スタックトレースを貼り付けてグラフ化すると frame カードが出ます
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
          表示上限 {MAX_FRAMES} ({graph.trimmed} frame 省略)
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
