/**
 * スタックトレースを 1 つの React Flow グラフとして表示する。
 * - 上から下へ frame 順 (#0 が最上、最深 frame が最下)
 * - クリックで `open_at` (in-project のみ。out-of-project は disabled)
 *
 * 入力はテキストエリア → `parse_stack_trace` (Rust) で frames を取り、
 * このコンポーネントへ渡す。
 */
import { useMemo, useEffect } from "react";
import { ReactFlow, Background, Controls, type Node, type Edge } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { win, type StackFrame } from "./lsp";

interface Props {
  frames: StackFrame[];
}

export function StackTraceGraph({ frames }: Props) {
  const { nodes, edges } = useMemo<{ nodes: Node[]; edges: Edge[] }>(() => {
    const nodes: Node[] = [];
    const edges: Edge[] = [];

    frames.forEach((f, i) => {
      const id = `f-${i}`;
      const fname = f.path.split(/[\\/]/).pop() ?? f.path;
      const label = `#${f.index} ${f.function ?? "<anon>"}\n${fname}:${f.line}`;
      nodes.push({
        id,
        data: { label, path: f.path, line: f.line, inProject: f.in_project },
        position: { x: 0, y: i * 80 },
        style: frameStyle(f.in_project),
        className: f.in_project ? "iter-clickable" : "iter-disabled",
      });
      if (i > 0) {
        edges.push({
          id: `e-${i}`,
          source: `f-${i - 1}`,
          target: id,
          animated: false,
          style: { stroke: "#4a7afe" },
        });
      }
    });

    return { nodes, edges };
  }, [frames]);

  useEffect(() => {
    const id = "iter-graph-style";
    if (document.getElementById(id)) return;
    const s = document.createElement("style");
    s.id = id;
    s.textContent = `
      .iter-clickable { cursor: pointer; }
      .iter-disabled  { cursor: not-allowed; opacity: 0.55; }
      .react-flow__attribution { display: none; }
    `;
    document.head.appendChild(s);
  }, []);

  const onNodeClick = (_: unknown, node: Node) => {
    const d = node.data as { path?: string; line?: number; inProject?: boolean };
    if (!d.inProject || !d.path || d.line === undefined) return;
    // line は 1-based 想定。Monaco は 1-based、LSP は 0-based。
    // stack-trace は 1-based なので、Monaco 互換のため -1 して渡す
    void win.openAt(d.path, Math.max(0, d.line - 1), 0, false);
  };

  if (!frames.length) {
    return (
      <div style={{ padding: "1rem", color: "#6b7383", fontSize: "0.85rem" }}>
        スタックトレースをパースすると frame グラフが出ます
      </div>
    );
  }

  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      onNodeClick={onNodeClick}
      fitView
      proOptions={{ hideAttribution: true }}
      colorMode="dark"
    >
      <Background color="#222831" gap={16} />
      <Controls showInteractive={false} />
    </ReactFlow>
  );
}

function frameStyle(inProject: boolean) {
  return {
    background: "#11141a",
    color: inProject ? "#e6e8ee" : "#6b7383",
    border: `1px solid ${inProject ? "#4a7afe" : "#3a3f4a"}`,
    borderRadius: 4,
    padding: "6px 10px",
    fontSize: 11,
    fontFamily: "ui-monospace, Consolas, monospace",
    whiteSpace: "pre-line" as const,
    minWidth: 220,
  };
}
