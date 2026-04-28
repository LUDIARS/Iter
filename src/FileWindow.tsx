import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Editor, { type OnMount } from "@monaco-editor/react";
import { readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type * as Monaco from "monaco-editor";
import { lsp, win, type CallHierarchyResult, type LspLocation } from "./lsp";
import { RelationGraph, type RelationData } from "./RelationGraph";

interface Props {
  path: string;
  initialLine?: number;
  initialCol?: number;
  followDefinition?: boolean;
}

const LANG_BY_EXT: Record<string, string> = {
  c: "c",
  h: "c",
  cc: "cpp",
  cpp: "cpp",
  cxx: "cpp",
  hpp: "cpp",
  hxx: "cpp",
  ts: "typescript",
  tsx: "typescript",
  js: "javascript",
  jsx: "javascript",
  rs: "rust",
  py: "python",
  json: "json",
  md: "markdown",
  yaml: "yaml",
  yml: "yaml",
  toml: "ini",
  cmake: "cmake",
  txt: "plaintext",
};

function languageOf(path: string): string {
  const lower = path.toLowerCase();
  if (lower.endsWith("cmakelists.txt")) return "cmake";
  const m = lower.match(/\.([a-z0-9]+)$/);
  return m ? LANG_BY_EXT[m[1]] ?? "plaintext" : "plaintext";
}

const PROJECT_ROOT_KEY = "iter:project_root";

export function FileWindow({ path, initialLine, initialCol, followDefinition }: Props) {
  const [contents, setContents] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [searchTerm, setSearchTerm] = useState("");
  const [matchCount, setMatchCount] = useState<number | null>(null);
  const [showRelations, setShowRelations] = useState({
    callers: true,
    callees: true,
    references: false,
  });
  const [relationData, setRelationData] = useState<RelationData | null>(null);
  const [graphCollapsed, setGraphCollapsed] = useState(false);

  const editorRef = useRef<Monaco.editor.IStandaloneCodeEditor | null>(null);
  const monacoRef = useRef<typeof Monaco | null>(null);
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const decorationsRef = useRef<string[]>([]);
  const queryDebounceRef = useRef<number | null>(null);

  const language = useMemo(() => languageOf(path), [path]);

  // 初期ロード
  useEffect(() => {
    let aborted = false;
    (async () => {
      try {
        const text = await readTextFile(path);
        if (!aborted) {
          setContents(text);
          // LSP didOpen (失敗しても無視 — clangd が動いていない / non-cpp ファイル)
          void lsp.openFile(path, text).catch(() => undefined);
        }
      } catch (e) {
        if (!aborted) setError(String(e));
      }
    })();
    return () => {
      aborted = true;
    };
  }, [path]);

  useEffect(() => {
    if (path) document.title = `Iter — ${path.split(/[\\/]/).pop() ?? path}`;
  }, [path]);

  // Control Panel の relation toggle を listen
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    (async () => {
      unlisten = await listen<string[]>("iter://relations-changed", (e) => {
        const set = new Set(e.payload);
        setShowRelations({
          callers: set.has("callers"),
          callees: set.has("callees"),
          references: set.has("references"),
        });
      });
    })();
    return () => {
      unlisten?.();
    };
  }, []);

  // open-at イベント (別ウィンドウから飛ばされてくる)
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    (async () => {
      unlisten = await listen<{
        path: string;
        line: number;
        column: number;
        follow_definition: boolean;
      }>("iter://open-at", (e) => {
        const ed = editorRef.current;
        if (!ed) return;
        if (e.payload.path === path) {
          ed.revealLineInCenter(e.payload.line + 1);
          ed.setPosition({
            lineNumber: e.payload.line + 1,
            column: (e.payload.column || 0) + 1,
          });
          ed.focus();
        }
      });
    })();
    return () => {
      unlisten?.();
    };
  }, [path]);

  // Ctrl+F → 検索フォーカス
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
        e.preventDefault();
        searchInputRef.current?.focus();
        searchInputRef.current?.select();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // 検索文字列のハイライト
  useEffect(() => {
    const ed = editorRef.current;
    const monaco = monacoRef.current;
    if (!ed || !monaco) return;
    const model = ed.getModel();
    if (!model) return;

    if (!searchTerm) {
      ed.deltaDecorations(decorationsRef.current, []);
      decorationsRef.current = [];
      setMatchCount(null);
      return;
    }
    const matches = model.findMatches(searchTerm, true, false, false, null, false);
    setMatchCount(matches.length);
    decorationsRef.current = ed.deltaDecorations(
      decorationsRef.current,
      matches.map((m) => ({
        range: m.range,
        options: { inlineClassName: "iter-search-hit" },
      })),
    );
    if (matches.length > 0) ed.revealRangeInCenter(matches[0].range);
  }, [searchTerm]);

  // カーソル位置 (1-based monaco) に応じて LSP を叩いて relation 更新
  const queryRelations = useCallback(
    (lineMonaco: number, colMonaco: number) => {
      if (queryDebounceRef.current !== null)
        window.clearTimeout(queryDebounceRef.current);
      queryDebounceRef.current = window.setTimeout(() => {
        const line = Math.max(0, lineMonaco - 1); // → LSP 0-based
        const character = Math.max(0, colMonaco - 1);
        const wantHierarchy = showRelations.callers || showRelations.callees;
        const wantRefs = showRelations.references;

        const hierarchyP: Promise<CallHierarchyResult | null> = wantHierarchy
          ? lsp.callHierarchy(path, line, character).catch(() => null)
          : Promise.resolve(null);
        const refsP: Promise<LspLocation[]> = wantRefs
          ? lsp.references(path, line, character).catch(() => [])
          : Promise.resolve([]);

        void Promise.all([hierarchyP, refsP]).then(([h, r]) => {
          const name = h?.items[0]?.name ?? "";
          setRelationData({
            origin: { name, path, line },
            callHierarchy: h,
            references: r,
          });
        });
      }, 250);
    },
    [path, showRelations.callers, showRelations.callees, showRelations.references],
  );

  const handleMount: OnMount = (editor, monaco) => {
    editorRef.current = editor;
    monacoRef.current = monaco;

    if (initialLine !== undefined) {
      const ln = Math.max(1, initialLine);
      const col = (initialCol ?? 0) + 1;
      editor.revealLineInCenter(ln);
      editor.setPosition({ lineNumber: ln, column: col });
      editor.focus();
      if (followDefinition) {
        // 宣言を追跡: 最初に LSP definitions を取り、結果が別ファイルなら open_at
        // (Phase 2 では definitions を未公開なので references の最初を流用)
        // → 簡易実装: relation query を 1 度走らせるだけにとどめる
      }
    }

    editor.onDidChangeCursorPosition((e) => {
      queryRelations(e.position.lineNumber, e.position.column);
    });
  };

  const save = useCallback(async () => {
    const ed = editorRef.current;
    if (!ed) return;
    try {
      await writeTextFile(path, ed.getValue());
    } catch (e) {
      setError(`保存に失敗: ${String(e)}`);
    }
  }, [path]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        void save();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [save]);

  // Ctrl+Shift+W: 自分以外の File Window を全部閉じる
  useEffect(() => {
    const onKey = async (e: KeyboardEvent) => {
      if (
        (e.ctrlKey || e.metaKey) &&
        e.shiftKey &&
        e.key.toLowerCase() === "w"
      ) {
        e.preventDefault();
        try {
          const myLabel = getCurrentWebviewWindow().label;
          const closed = await win.closeOthers(myLabel);
          if (closed > 0) {
            // ささやかに通知
            console.log(`Iter: closed ${closed} other window(s)`);
          }
        } catch (err) {
          console.error("close-others failed:", err);
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  if (!path) {
    return <div className="fw-error">path クエリが指定されていません</div>;
  }

  // project_root はセッション間で共有できないので localStorage 経由
  // (ControlPanel が detect 時に保存するように Phase 2.5 で拡張予定)
  const projectRoot =
    typeof window !== "undefined" ? localStorage.getItem(PROJECT_ROOT_KEY) : null;

  return (
    <div className="fw-shell-2">
      <header className="fw-header">
        <span className="fw-path" title={path}>
          {path}
        </span>
        <div className="fw-search">
          <input
            ref={searchInputRef}
            type="search"
            placeholder="検索 (Ctrl+F)"
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
          />
          <span className="count">
            {matchCount === null ? "" : `${matchCount} 件`}
          </span>
          <button
            onClick={() => setGraphCollapsed((v) => !v)}
            title="関連グラフの表示切替"
          >
            {graphCollapsed ? "▸ Graph" : "▾ Graph"}
          </button>
          <button onClick={save} title="Ctrl+S">
            保存
          </button>
          <button
            onClick={async () => {
              const myLabel = getCurrentWebviewWindow().label;
              await win.closeOthers(myLabel);
            }}
            title="他の Iter ファイルウィンドウを全部閉じる (Ctrl+Shift+W)"
            style={{ fontSize: "0.75rem" }}
          >
            他を閉じる
          </button>
        </div>
      </header>

      <div className="fw-body" data-collapsed={graphCollapsed}>
        <div className="fw-editor-pane">
          {error ? (
            <div className="fw-error">{error}</div>
          ) : contents === null ? (
            <div className="fw-loading">読み込み中…</div>
          ) : (
            <Editor
              height="100%"
              theme="vs-dark"
              language={language}
              value={contents}
              onChange={(v) => setContents(v ?? "")}
              onMount={handleMount}
              options={{
                automaticLayout: true,
                minimap: { enabled: true },
                fontSize: 13,
                wordWrap: "off",
                renderWhitespace: "selection",
                smoothScrolling: true,
                mouseWheelZoom: true,
              }}
            />
          )}
        </div>
        {!graphCollapsed && (
          <div className="fw-graph-pane">
            <RelationGraph
              data={relationData}
              show={showRelations}
              projectRoot={projectRoot}
            />
          </div>
        )}
      </div>
      <style>{`
        .iter-search-hit { background: rgba(74, 122, 254, 0.35); border-radius: 2px; }
        .fw-shell-2 {
          display: grid; grid-template-rows: auto 1fr; height: 100%;
        }
        .fw-body {
          display: grid;
          grid-template-columns: 1fr 380px;
          height: 100%;
          overflow: hidden;
        }
        .fw-body[data-collapsed="true"] {
          grid-template-columns: 1fr 0;
        }
        .fw-editor-pane { position: relative; min-width: 0; }
        .fw-graph-pane {
          border-left: 1px solid #1c1f27;
          background: #0e0f12;
          min-width: 0;
        }
      `}</style>
    </div>
  );
}
