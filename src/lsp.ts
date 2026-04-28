import { invoke } from "@tauri-apps/api/core";

/** Rust 側 `lsp_types::Uri` を文字列としてだけ扱う (ts では生 URL) */
export interface LspPosition {
  line: number;
  character: number;
}

export interface LspRange {
  start: LspPosition;
  end: LspPosition;
}

export interface LspLocation {
  /** RFC3986 URI string. file:// scheme 想定 */
  uri: string;
  range: LspRange;
}

export interface CallHierarchyItem {
  name: string;
  kind: number;
  detail?: string;
  uri: string;
  range: LspRange;
  selectionRange: LspRange;
}

export interface CallHierarchyIncomingCall {
  from: CallHierarchyItem;
  fromRanges: LspRange[];
}

export interface CallHierarchyOutgoingCall {
  to: CallHierarchyItem;
  fromRanges: LspRange[];
}

export interface CallHierarchyResult {
  items: CallHierarchyItem[];
  incoming: CallHierarchyIncomingCall[];
  outgoing: CallHierarchyOutgoingCall[];
}

export interface StackFrame {
  index: number;
  function: string | null;
  path: string;
  line: number;
  column: number | null;
  in_project: boolean;
}

export const lsp = {
  async openProject(root: string): Promise<void> {
    await invoke("lsp_open_project", { root });
  },
  async openFile(path: string, text: string): Promise<void> {
    await invoke("lsp_open_file", { path, text });
  },
  async callHierarchy(
    path: string,
    line: number,
    character: number,
  ): Promise<CallHierarchyResult> {
    return invoke<CallHierarchyResult>("lsp_call_hierarchy", {
      path,
      line,
      character,
    });
  },
  async references(
    path: string,
    line: number,
    character: number,
  ): Promise<LspLocation[]> {
    return invoke<LspLocation[]>("lsp_references", { path, line, character });
  },
  async parseStackTrace(text: string, projectRoot?: string): Promise<StackFrame[]> {
    return invoke<StackFrame[]>("parse_stack_trace", {
      text,
      projectRoot: projectRoot ?? null,
    });
  },
};

export const win = {
  async openFileWindow(path: string): Promise<void> {
    await invoke("open_file_window", { path });
  },
  async openAt(
    path: string,
    line: number,
    column?: number,
    followDefinition?: boolean,
  ): Promise<void> {
    await invoke("open_at", {
      path,
      line,
      column: column ?? null,
      followDefinition: followDefinition ?? null,
    });
  },
};

/** uri から OS パスへの変換 (`file:///c:/foo` → `c:/foo`)。Windows / Unix 両対応。 */
export function uriToPath(uri: string): string {
  if (!uri.startsWith("file://")) return uri;
  const after = decodeURIComponent(uri.slice("file://".length));
  // Windows: `/c:/foo` → `c:/foo`
  if (/^\/[A-Za-z]:/.test(after)) return after.slice(1);
  return after;
}
