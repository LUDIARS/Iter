//! Tauri が呼び出す `#[command]` エンドポイント群。
//!
//! state として `Mutex<Option<ClangdClient>>` を 1 つだけ持ち、`detect_project`
//! 直後に `lsp_open_project` を呼ぶ想定。フロントは `lsp_call_hierarchy` /
//! `lsp_definitions` / `lsp_references` で 1 ヶ所のシンボル情報を取れる。
//!
//! Phase 2 MVP では同時に 1 プロジェクトしか開けない (シングルトン)。

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyItem, CallHierarchyOutgoingCall, Location, Uri,
};
use serde::Serialize;
use tokio::sync::Mutex;

use crate::compile_db;
use crate::lsp::ClangdClient;

#[derive(Default)]
pub struct LspState {
    pub client: Mutex<Option<Arc<ClangdClient>>>,
    pub project_root: Mutex<Option<PathBuf>>,
}

/// プロジェクトを開いて clangd を起動する。compile_commands.json を必要なら生成。
#[tauri::command]
pub async fn lsp_open_project(
    state: tauri::State<'_, LspState>,
    root: String,
) -> Result<(), String> {
    let root_path = PathBuf::from(&root);
    let cc = compile_db::ensure_compile_commands(&root_path)?;
    let cc_dir = cc.parent().map(PathBuf::from).unwrap_or(root_path.clone());

    let client = ClangdClient::spawn(&root_path, &cc_dir)
        .await
        .map_err(|e| e.to_string())?;
    let arc = Arc::new(client);

    *state.client.lock().await = Some(arc);
    *state.project_root.lock().await = Some(root_path);
    Ok(())
}

#[tauri::command]
pub async fn lsp_open_file(
    state: tauri::State<'_, LspState>,
    path: String,
    text: String,
) -> Result<(), String> {
    let client = require_client(&state).await?;
    let uri = path_to_uri(&path)?;
    let language_id = guess_language_id(&path);
    client
        .did_open(uri, &language_id, text)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct CallHierarchyResult {
    pub items: Vec<CallHierarchyItem>,
    pub incoming: Vec<CallHierarchyIncomingCall>,
    pub outgoing: Vec<CallHierarchyOutgoingCall>,
}

/// 指定位置で prepareCallHierarchy → incoming/outgoing を 1 まとめで取得。
#[tauri::command]
pub async fn lsp_call_hierarchy(
    state: tauri::State<'_, LspState>,
    path: String,
    line: u32,
    character: u32,
) -> Result<CallHierarchyResult, String> {
    let client = require_client(&state).await?;
    let uri = path_to_uri(&path)?;
    let pos = lsp_types::Position { line, character };

    let items = client
        .prepare_call_hierarchy(uri, pos)
        .await
        .map_err(|e| e.to_string())?;
    let mut incoming = Vec::new();
    let mut outgoing = Vec::new();
    for it in &items {
        if let Ok(mut v) = client.incoming_calls(it.clone()).await {
            incoming.append(&mut v);
        }
        if let Ok(mut v) = client.outgoing_calls(it.clone()).await {
            outgoing.append(&mut v);
        }
    }
    Ok(CallHierarchyResult {
        items,
        incoming,
        outgoing,
    })
}

#[tauri::command]
pub async fn lsp_references(
    state: tauri::State<'_, LspState>,
    path: String,
    line: u32,
    character: u32,
) -> Result<Vec<Location>, String> {
    let client = require_client(&state).await?;
    let uri = path_to_uri(&path)?;
    client
        .references(uri, lsp_types::Position { line, character })
        .await
        .map_err(|e| e.to_string())
}

fn path_to_uri(path: &str) -> Result<Uri, String> {
    let url = url::Url::from_file_path(path).map_err(|_| format!("invalid path: {path}"))?;
    Uri::from_str(url.as_str()).map_err(|e| format!("uri parse: {e}"))
}

async fn require_client(
    state: &tauri::State<'_, LspState>,
) -> Result<Arc<ClangdClient>, String> {
    let guard = state.client.lock().await;
    guard
        .clone()
        .ok_or_else(|| "LSP not initialized — call lsp_open_project first".to_string())
}

fn guess_language_id(path: &str) -> String {
    let lower = path.to_lowercase();
    if lower.ends_with(".c") || lower.ends_with(".h") {
        "c".to_string()
    } else if lower.ends_with(".cpp")
        || lower.ends_with(".cc")
        || lower.ends_with(".cxx")
        || lower.ends_with(".hpp")
        || lower.ends_with(".hxx")
    {
        "cpp".to_string()
    } else {
        "plaintext".to_string()
    }
}
