/**
 * 該当関数の callers / callees / references を React Flow で
 * 「コード外に小さい四角ノード + エッジ」として描画する。
 *
 * - 中央の青ノード = カーソル位置のシンボル (origin)
 * - 上に積む四角 = caller (incoming)
 * - 下に積む四角 = callee (outgoing)
 * - 右に積む四角 = reference
 *
 * ノードクリックで該当ファイルを `open_at` する。プロジェクト外
 * (uri が project_root の外) のノードは disabled クラスを当てて
 * クリックを抑制する。
 */
import { useEffect, useMemo } from "react";
import { ReactFlow, Background, Controls, type Node, type Edge } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import {
  win,
  uriToPath,
  type CallHierarchyResult,
  type LspLocation,
} from "./lsp";

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

export function RelationGraph({ data, show, projectRoot }: Props) {
  const { nodes, edges } = useMemo<{ nodes: Node[]; edges: Edge[] }>(() => {
    if (!data) return { nodes: [], edges: [] };

    const nodes: Node[] = [];
    const edges: Edge[] = [];

    nodes.push({
      id: "origin",
      type: "default",
      data: { label: data.origin.name || "(cursor)" },
      position: { x: 0, y: 0 },
      style: originStyle,
      selectable: false,
      draggable: false,
    });

    if (show.callers && data.callHierarchy) {
      data.callHierarchy.incoming.forEach((c, i) => {
        const id = `caller-${i}`;
        const path = uriToPath(c.from.uri);
        const inProject = isInProject(path, projectRoot);
        nodes.push({
          id,
          data: {
            label: c.from.name,
            path,
            line: c.from.range.start.line,
            inProject,
          },
          position: { x: -260, y: -60 + i * 50 },
          style: nodeStyle(inProject, "#5eb2ff"),
          className: inProject ? "iter-clickable" : "iter-disabled",
        });
        edges.push({
          id: `e-caller-${i}`,
          source: id,
          target: "origin",
          animated: false,
          style: { stroke: "#5eb2ff" },
          label: "calls",
        });
      });
    }

    if (show.callees && data.callHierarchy) {
      data.callHierarchy.outgoing.forEach((c, i) => {
        const id = `callee-${i}`;
        const path = uriToPath(c.to.uri);
        const inProject = isInProject(path, projectRoot);
        nodes.push({
          id,
          data: {
            label: c.to.name,
            path,
            line: c.to.range.start.line,
            inProject,
          },
          position: { x: 260, y: -60 + i * 50 },
          style: nodeStyle(inProject, "#7ddba2"),
          className: inProject ? "iter-clickable" : "iter-disabled",
        });
        edges.push({
          id: `e-callee-${i}`,
          source: "origin",
          target: id,
          animated: false,
          style: { stroke: "#7ddba2" },
          label: "calls",
        });
      });
    }

    if (show.references && data.references) {
      data.references.forEach((loc, i) => {
        const id = `ref-${i}`;
        const path = uriToPath(loc.uri);
        const inProject = isInProject(path, projectRoot);
        const fname = path.split(/[\\/]/).pop() ?? path;
        nodes.push({
          id,
          data: {
            label: `${fname}:${loc.range.start.line + 1}`,
            path,
            line: loc.range.start.line,
            inProject,
          },
          position: { x: 0, y: 110 + i * 40 },
          style: nodeStyle(inProject, "#efc56a"),
          className: inProject ? "iter-clickable" : "iter-disabled",
        });
        edges.push({
          id: `e-ref-${i}`,
          source: "origin",
          target: id,
          animated: false,
          style: { stroke: "#efc56a", strokeDasharray: "4 2" },
        });
      });
    }

    return { nodes, edges };
  }, [data, show, projectRoot]);

  // body スタイル注入 (Tailwind 等は使っていないので一時的にここで)
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
    void win.openAt(d.path, d.line, 0, false);
  };

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

function isInProject(path: string, root: string | null): boolean {
  if (!root) return true; // root 不明時は全部 clickable 扱い
  const norm = (s: string) => s.replace(/\\/g, "/").toLowerCase();
  return norm(path).startsWith(norm(root));
}

const originStyle = {
  background: "#1c2233",
  color: "#e6e8ee",
  border: "2px solid #4a7afe",
  borderRadius: 6,
  padding: "6px 12px",
  fontSize: 12,
  fontFamily: "ui-monospace, Consolas, monospace",
};

function nodeStyle(inProject: boolean, accent: string) {
  return {
    background: "#11141a",
    color: inProject ? "#e6e8ee" : "#6b7383",
    border: `1px solid ${accent}`,
    borderRadius: 4,
    padding: "4px 8px",
    fontSize: 11,
    fontFamily: "ui-monospace, Consolas, monospace",
  };
}
