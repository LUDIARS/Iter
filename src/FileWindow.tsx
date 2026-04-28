import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Editor, { type OnMount } from "@monaco-editor/react";
import { readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";
import type * as Monaco from "monaco-editor";

interface Props {
  path: string;
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

export function FileWindow({ path }: Props) {
  const [contents, setContents] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [searchTerm, setSearchTerm] = useState("");
  const [matchCount, setMatchCount] = useState<number | null>(null);
  const editorRef = useRef<Monaco.editor.IStandaloneCodeEditor | null>(null);
  const monacoRef = useRef<typeof Monaco | null>(null);
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const decorationsRef = useRef<string[]>([]);

  const language = useMemo(() => languageOf(path), [path]);

  // 初期ロード
  useEffect(() => {
    let aborted = false;
    (async () => {
      try {
        const text = await readTextFile(path);
        if (!aborted) setContents(text);
      } catch (e) {
        if (!aborted) setError(String(e));
      }
    })();
    return () => {
      aborted = true;
    };
  }, [path]);

  // タイトル更新
  useEffect(() => {
    if (path) document.title = `Iter — ${path.split(/[\\/]/).pop() ?? path}`;
  }, [path]);

  // Ctrl+F は **常時表示の検索バー** にフォーカスするだけ。Monaco 標準の
  // Find ウィジェットも残っているのでそちら経由でも検索できる。
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

  // 検索文字列が変わるたびに editor 上のマッチをハイライト + 件数を出す
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

    const matches = model.findMatches(
      searchTerm,
      true, // searchOnlyEditableRange
      false, // isRegex
      false, // matchCase
      null, // wordSeparators
      false, // captureMatches
    );
    setMatchCount(matches.length);

    const newDecorations = matches.map((m) => ({
      range: m.range,
      options: { inlineClassName: "iter-search-hit" },
    }));
    decorationsRef.current = ed.deltaDecorations(
      decorationsRef.current,
      newDecorations,
    );

    if (matches.length > 0) {
      ed.revealRangeInCenter(matches[0].range);
    }
  }, [searchTerm]);

  const handleMount: OnMount = (editor, monaco) => {
    editorRef.current = editor;
    monacoRef.current = monaco;
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

  // Ctrl+S で保存
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

  if (!path) {
    return <div className="fw-error">path クエリが指定されていません</div>;
  }

  return (
    <div className="fw-shell">
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
          <button onClick={save} title="Ctrl+S">
            保存
          </button>
        </div>
      </header>

      <div className="fw-editor">
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
            }}
          />
        )}
      </div>
      <style>{`.iter-search-hit { background: rgba(74, 122, 254, 0.35); border-radius: 2px; }`}</style>
    </div>
  );
}
