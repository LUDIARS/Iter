import { useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { emit } from "@tauri-apps/api/event";
import { lsp, win, type StackFrame } from "./lsp";
import { StackTraceGraph } from "./StackTraceGraph";

interface ProjectInfo {
  root: string;
  build_system: "cmake" | "vcxproj" | "unknown";
  files: FileNode[];
  from_cache?: boolean;
}

const PROJECT_ROOT_KEY = "iter:project_root";

interface FileNode {
  path: string;
  rel: string;
  name: string;
  is_dir: boolean;
  children: FileNode[];
}

const RELATION_KINDS = [
  { key: "callers", label: "呼び出し元 (callers)" },
  { key: "callees", label: "呼び出し先 (callees)" },
  { key: "references", label: "参照 (references)" },
] as const;

export function ControlPanel() {
  const [project, setProject] = useState<ProjectInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [lspState, setLspState] = useState<"idle" | "starting" | "ready" | "failed">(
    "idle",
  );
  const [error, setError] = useState<string | null>(null);
  const [enabledRelations, setEnabledRelations] = useState<Set<string>>(
    () => new Set(["callers", "callees"]),
  );

  // スタックトレース入力 + frame
  const [stackInput, setStackInput] = useState("");
  const [frames, setFrames] = useState<StackFrame[]>([]);

  const pickProject = useCallback(async () => {
    const picked = await open({ directory: true, multiple: false });
    if (!picked || typeof picked !== "string") return;
    setLoading(true);
    setError(null);
    try {
      const info = await invoke<ProjectInfo>("detect_project", { root: picked });
      setProject(info);
      // file window から in-project 判定で読みたいので localStorage に root を共有
      try {
        localStorage.setItem(PROJECT_ROOT_KEY, info.root);
      } catch {
        // sandboxed iframe 等で失敗するなら無視
      }

      // CMake プロジェクトなら clangd を起動 (失敗しても閲覧/編集は可能)
      if (info.build_system === "cmake") {
        setLspState("starting");
        try {
          await lsp.openProject(info.root);
          setLspState("ready");
        } catch (e) {
          setLspState("failed");
          setError(`LSP 起動失敗: ${String(e)}`);
        }
      } else {
        setLspState("idle");
      }
    } catch (e) {
      setError(String(e));
      setProject(null);
    } finally {
      setLoading(false);
    }
  }, []);

  const openFile = useCallback(async (path: string) => {
    try {
      await win.openFileWindow(path);
    } catch (e) {
      setError(`ファイルを開けませんでした: ${String(e)}`);
    }
  }, []);

  const toggleRelation = (key: string) => {
    setEnabledRelations((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      // 全 file window へ broadcast
      void emit("iter://relations-changed", Array.from(next));
      return next;
    });
  };

  const onParseStack = async () => {
    const f = await lsp.parseStackTrace(stackInput, project?.root);
    setFrames(f);
  };

  const refreshProject = useCallback(async () => {
    if (!project) return;
    setLoading(true);
    try {
      const fresh = await invoke<ProjectInfo>("refresh_project", {
        root: project.root,
      });
      setProject(fresh);
    } catch (e) {
      setError(`再走査失敗: ${String(e)}`);
    } finally {
      setLoading(false);
    }
  }, [project]);

  return (
    <div className="cp-shell">
      <header className="cp-header">
        <h1>Iter — Source Map</h1>
        <button onClick={pickProject} disabled={loading}>
          {loading ? "解析中…" : "プロジェクトを開く"}
        </button>
      </header>

      <div className="cp-project-bar">
        {project ? (
          <>
            <div style={{ display: "flex", alignItems: "center", gap: "0.5rem", flexWrap: "wrap" }}>
              <strong>{project.build_system.toUpperCase()}</strong>
              <span style={{ color: lspBadgeColor(lspState) }}>
                LSP: {lspState}
              </span>
              {project.from_cache && (
                <span
                  title="ディスクキャッシュから即時ロード。最新化したい場合は ↻ を押す"
                  style={{
                    fontSize: "0.7rem",
                    background: "#1f232b",
                    border: "1px solid #2a2f3a",
                    padding: "1px 6px",
                    borderRadius: 3,
                    color: "#7ddba2",
                  }}
                >
                  cached
                </span>
              )}
              <button
                onClick={refreshProject}
                disabled={loading}
                style={{ marginLeft: "auto", padding: "0.2rem 0.5rem", fontSize: "0.75rem" }}
                title="プロジェクトを再走査 (キャッシュ破棄)"
              >
                ↻ 再走査
              </button>
            </div>
            <div className="cp-project-path">{project.root}</div>
          </>
        ) : (
          <div className="cp-project-path">
            まだプロジェクトが選択されていません
          </div>
        )}
        {error && <div style={{ color: "#e66060" }}>{error}</div>}
      </div>

      <div style={{ overflowY: "auto" }}>
        <section className="cp-section">
          <h2>関連性表示</h2>
          <div className="cp-checks">
            {RELATION_KINDS.map((k) => (
              <label key={k.key}>
                <input
                  type="checkbox"
                  checked={enabledRelations.has(k.key)}
                  onChange={() => toggleRelation(k.key)}
                />
                {k.label}
              </label>
            ))}
          </div>
          <div style={{ marginTop: "0.4rem", fontSize: "0.75rem", color: "#6b7383" }}>
            File Window のカーソル位置で clangd の callHierarchy/references を実行
          </div>
        </section>

        <section className="cp-section">
          <h2>スタックトレース → グラフ</h2>
          <textarea
            value={stackInput}
            onChange={(e) => setStackInput(e.target.value)}
            placeholder="スタックトレースを貼り付け (gdb / lldb / sanitizer / V8 / Python / Rust 対応)"
            style={{
              width: "100%",
              minHeight: "100px",
              fontFamily: "ui-monospace, Consolas, monospace",
              fontSize: "0.78rem",
              background: "#14171d",
              color: "inherit",
              border: "1px solid #2a2f3a",
              borderRadius: 4,
              padding: "0.4rem",
              boxSizing: "border-box",
              resize: "vertical",
            }}
          />
          <button onClick={onParseStack} disabled={!stackInput.trim()}>
            グラフ化 ({frames.length} frames)
          </button>
          <div style={{ height: 300, marginTop: "0.5rem", border: "1px solid #1c1f27", borderRadius: 4 }}>
            <StackTraceGraph frames={frames} />
          </div>
          <div style={{ fontSize: "0.7rem", color: "#6b7383", marginTop: "0.3rem" }}>
            プロジェクト外の frame はグレー (クリック不可)。in-project はクリックで該当行を開く。
          </div>
        </section>

        <section className="cp-section">
          <h2>ファイルツリー</h2>
        </section>
        <div className="cp-tree">
          {project ? (
            <FileTree nodes={project.files} onOpen={openFile} />
          ) : (
            <div className="cp-tree-empty">
              「プロジェクトを開く」から CMakeLists.txt があるディレクトリを選択
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function lspBadgeColor(state: string): string {
  switch (state) {
    case "ready":
      return "#7ddba2";
    case "starting":
      return "#efc56a";
    case "failed":
      return "#e66060";
    default:
      return "#8993a4";
  }
}

function FileTree({
  nodes,
  onOpen,
  depth = 0,
}: {
  nodes: FileNode[];
  onOpen: (path: string) => void;
  depth?: number;
}) {
  return (
    <>
      {nodes.map((n) => (
        <FileTreeNode key={n.path} node={n} onOpen={onOpen} depth={depth} />
      ))}
    </>
  );
}

function FileTreeNode({
  node,
  onOpen,
  depth,
}: {
  node: FileNode;
  onOpen: (path: string) => void;
  depth: number;
}) {
  const [expanded, setExpanded] = useState(depth < 1);
  const indent = { paddingLeft: `${depth * 0.9 + 0.3}rem` };

  if (node.is_dir) {
    return (
      <>
        <div
          className="cp-tree-node dir"
          style={indent}
          onClick={() => setExpanded((v) => !v)}
        >
          <span className="icon">{expanded ? "▾" : "▸"}</span>
          <span>{node.name}</span>
        </div>
        {expanded && (
          <FileTree nodes={node.children} onOpen={onOpen} depth={depth + 1} />
        )}
      </>
    );
  }

  return (
    <div
      className="cp-tree-node file"
      style={indent}
      onClick={() => onOpen(node.path)}
      title={node.path}
    >
      <span className="icon">·</span>
      <span>{node.name}</span>
    </div>
  );
}
