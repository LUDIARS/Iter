import { useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

/** Rust 側 `detect_project` が返す形。`tauri/src/project.rs` と同期 */
interface ProjectInfo {
  root: string;
  build_system: "cmake" | "vcxproj" | "unknown";
  files: FileNode[];
}

interface FileNode {
  path: string;
  rel: string;
  name: string;
  is_dir: boolean;
  children: FileNode[];
}

/** Phase 2 で graph に表示する関連の種類 (UI だけ先に置く) */
const RELATION_KINDS = [
  { key: "callers", label: "呼び出し元 (callers)" },
  { key: "callees", label: "呼び出し先 (callees)" },
  { key: "references", label: "参照 (references)" },
  { key: "definitions", label: "定義先 (definitions)" },
] as const;

export function ControlPanel() {
  const [project, setProject] = useState<ProjectInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [enabledRelations, setEnabledRelations] = useState<Set<string>>(
    () => new Set(["callers", "callees"]),
  );

  const pickProject = useCallback(async () => {
    const picked = await open({ directory: true, multiple: false });
    if (!picked || typeof picked !== "string") return;
    setLoading(true);
    setError(null);
    try {
      const info = await invoke<ProjectInfo>("detect_project", { root: picked });
      setProject(info);
    } catch (e) {
      setError(String(e));
      setProject(null);
    } finally {
      setLoading(false);
    }
  }, []);

  const openFile = useCallback(async (path: string) => {
    try {
      await invoke("open_file_window", { path });
    } catch (e) {
      setError(`ファイルを開けませんでした: ${String(e)}`);
    }
  }, []);

  const toggleRelation = (key: string) => {
    setEnabledRelations((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

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
            <div>
              <strong>{project.build_system.toUpperCase()}</strong> プロジェクト検知
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
            ※ Phase 2 で clangd の callHierarchy / references を有効化します
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

