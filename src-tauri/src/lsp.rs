//! clangd subprocess + LSP JSON-RPC ブリッジ。
//!
//! - stdin/stdout で JSON-RPC、Content-Length ヘッダフレーミング
//! - 1 プロジェクト = 1 clangd プロセス、`AppState` 配下に Arc<Mutex<Option<Client>>> で保持
//! - フロントが叩く Tauri コマンドからは `Client::*` を await で呼ぶ
//! - clangd プロセスを wait task で監視し、死亡時に `iter://lsp-down` を emit。
//!   pending request は即時 error で drain し、`is_dead` を立てて以降の request も拒否
//! - stderr は piped で受け、直近 STDERR_RING_CAP 行を保持。死亡通知に同梱
//!
//! Phase 2 の MVP では call hierarchy / references / definition の 3 系統だけ。
//! semanticTokens / inlayHint / formatter 等は今回スコープ外。

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

use lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyItem, CallHierarchyOutgoingCall, ClientCapabilities,
    DidOpenTextDocumentParams, InitializeParams, InitializedParams, Location, Position,
    ReferenceContext, ReferenceParams, TextDocumentIdentifier, TextDocumentItem,
    TextDocumentPositionParams, Uri, WorkDoneProgressParams, WorkspaceFolder,
};
use serde_json::{Value, json};
use std::str::FromStr;
use tauri::Emitter;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, Command};
use tokio::sync::{Mutex, oneshot};

#[derive(Debug, thiserror::Error)]
pub enum LspError {
    #[error("clangd not found in PATH")]
    ClangdNotFound,
    #[error("clangd exited unexpectedly: {0}")]
    Dead(String),
    #[error("io: {0}")]
    Io(String),
    #[error("rpc: {0}")]
    Rpc(String),
}

impl From<std::io::Error> for LspError {
    fn from(e: std::io::Error) -> Self {
        LspError::Io(e.to_string())
    }
}

type Pending = Arc<Mutex<HashMap<i64, oneshot::Sender<Result<Value, String>>>>>;

const STDERR_RING_CAP: usize = 100;

pub struct ClangdClient {
    stdin: Mutex<ChildStdin>,
    next_id: AtomicI64,
    pending: Pending,
    is_dead: Arc<AtomicBool>,
}

impl ClangdClient {
    /// clangd を起動し、initialize / initialized を完了するまでブロックする。
    /// `app` は wait task が `iter://lsp-down` イベントを emit するために使う。
    pub async fn spawn(
        project_root: &Path,
        compile_commands_dir: &Path,
        app: tauri::AppHandle,
    ) -> Result<Self, LspError> {
        let clangd = which::which("clangd").map_err(|_| LspError::ClangdNotFound)?;

        let mut child = Command::new(clangd)
            .arg(format!(
                "--compile-commands-dir={}",
                compile_commands_dir.display()
            ))
            .arg("--background-index")
            .arg("--clang-tidy=false")
            .arg("--header-insertion=never")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let stdin = child.stdin.take().expect("stdin captured");
        let stdout = child.stdout.take().expect("stdout captured");
        let stderr = child.stderr.take().expect("stderr captured");

        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let stderr_ring: Arc<Mutex<VecDeque<String>>> =
            Arc::new(Mutex::new(VecDeque::with_capacity(STDERR_RING_CAP)));
        let is_dead = Arc::new(AtomicBool::new(false));

        // stderr reader: 改行ごとにリングへ追加 (上限超過時は前から drop)
        let stderr_ring_for_reader = stderr_ring.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let mut b = stderr_ring_for_reader.lock().await;
                if b.len() >= STDERR_RING_CAP {
                    b.pop_front();
                }
                b.push_back(line);
            }
        });

        // stdout reader: 受信メッセージを pending の oneshot へ流す
        let pending_for_reader = pending.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            loop {
                match read_message(&mut reader).await {
                    Ok(Some(value)) => {
                        if let Some(id) = value.get("id").and_then(|v| v.as_i64()) {
                            let mut p = pending_for_reader.lock().await;
                            if let Some(tx) = p.remove(&id) {
                                if let Some(err) = value.get("error") {
                                    let _ = tx.send(Err(err.to_string()));
                                } else {
                                    let result =
                                        value.get("result").cloned().unwrap_or(Value::Null);
                                    let _ = tx.send(Ok(result));
                                }
                            }
                        }
                        // notification (id 無し) は今回無視
                    }
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
        });

        // wait task: child の終了を待ち、死亡時に状態を更新 + フロントへ通知
        let pending_for_wait = pending.clone();
        let stderr_ring_for_wait = stderr_ring.clone();
        let is_dead_for_wait = is_dead.clone();
        let app_for_wait = app.clone();
        tokio::spawn(async move {
            let mut child = child;
            let exit = child.wait().await;
            is_dead_for_wait.store(true, Ordering::SeqCst);

            let exit_desc = match &exit {
                Ok(s) => match s.code() {
                    Some(c) => format!("exit {c}"),
                    None => "killed by signal".to_string(),
                },
                Err(e) => format!("wait failed: {e}"),
            };
            let recent: Vec<String> = stderr_ring_for_wait.lock().await.iter().cloned().collect();

            // フロントに通知。emit 失敗 (window 全閉等) は致命ではない
            let _ = app_for_wait.emit(
                "iter://lsp-down",
                json!({
                    "reason": exit_desc,
                    "stderr": recent,
                }),
            );

            // pending を全部 error で drain (フロント側 hang 防止)
            let mut p = pending_for_wait.lock().await;
            for (_id, tx) in p.drain() {
                let _ = tx.send(Err(format!("clangd exited: {exit_desc}")));
            }
        });

        // stderr_ring は reader task と wait task が Arc で持ち合うので Self には保持しない
        drop(stderr_ring);

        let client = Self {
            stdin: Mutex::new(stdin),
            next_id: AtomicI64::new(1),
            pending,
            is_dead,
        };

        client.initialize(project_root).await?;
        client.initialized().await?;
        Ok(client)
    }

    /// clangd が既に死んでいるか。死亡監視 task が wait().await から返ったら true。
    pub fn is_dead(&self) -> bool {
        self.is_dead.load(Ordering::SeqCst)
    }

    async fn initialize(&self, project_root: &Path) -> Result<(), LspError> {
        let url = url::Url::from_directory_path(project_root)
            .map_err(|_| LspError::Rpc("invalid project_root path".to_string()))?;
        let root_uri = Uri::from_str(url.as_str())
            .map_err(|e| LspError::Rpc(format!("uri parse: {e}")))?;
        #[allow(deprecated)]
        let params = InitializeParams {
            process_id: Some(std::process::id()),
            root_path: None,
            root_uri: Some(root_uri.clone()),
            initialization_options: None,
            capabilities: ClientCapabilities::default(),
            trace: None,
            workspace_folders: Some(vec![WorkspaceFolder {
                uri: root_uri,
                name: project_root
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "root".to_string()),
            }]),
            client_info: None,
            locale: None,
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        let _ = self
            .request("initialize", serde_json::to_value(params).unwrap())
            .await?;
        Ok(())
    }

    async fn initialized(&self) -> Result<(), LspError> {
        let params = InitializedParams {};
        self.notify("initialized", serde_json::to_value(params).unwrap())
            .await
    }

    pub async fn did_open(
        &self,
        uri: Uri,
        language_id: &str,
        text: String,
    ) -> Result<(), LspError> {
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: language_id.to_string(),
                version: 1,
                text,
            },
        };
        self.notify("textDocument/didOpen", serde_json::to_value(params).unwrap())
            .await
    }

    pub async fn prepare_call_hierarchy(
        &self,
        uri: Uri,
        position: Position,
    ) -> Result<Vec<CallHierarchyItem>, LspError> {
        let params = TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position,
        };
        let v = self
            .request(
                "textDocument/prepareCallHierarchy",
                serde_json::to_value(params).unwrap(),
            )
            .await?;
        Ok(serde_json::from_value(v).unwrap_or_default())
    }

    pub async fn incoming_calls(
        &self,
        item: CallHierarchyItem,
    ) -> Result<Vec<CallHierarchyIncomingCall>, LspError> {
        let v = self
            .request("callHierarchy/incomingCalls", json!({ "item": item }))
            .await?;
        Ok(serde_json::from_value(v).unwrap_or_default())
    }

    pub async fn outgoing_calls(
        &self,
        item: CallHierarchyItem,
    ) -> Result<Vec<CallHierarchyOutgoingCall>, LspError> {
        let v = self
            .request("callHierarchy/outgoingCalls", json!({ "item": item }))
            .await?;
        Ok(serde_json::from_value(v).unwrap_or_default())
    }

    pub async fn references(
        &self,
        uri: Uri,
        position: Position,
    ) -> Result<Vec<Location>, LspError> {
        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: Default::default(),
            context: ReferenceContext {
                include_declaration: false,
            },
        };
        let v = self
            .request("textDocument/references", serde_json::to_value(params).unwrap())
            .await?;
        Ok(serde_json::from_value(v).unwrap_or_default())
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value, LspError> {
        if self.is_dead() {
            return Err(LspError::Dead("clangd is not running".to_string()));
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);

        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.send_raw(&msg).await?;

        match rx.await {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(LspError::Rpc(e)),
            Err(_) => Err(LspError::Rpc("response channel closed".to_string())),
        }
    }

    async fn notify(&self, method: &str, params: Value) -> Result<(), LspError> {
        if self.is_dead() {
            return Err(LspError::Dead("clangd is not running".to_string()));
        }
        let msg = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.send_raw(&msg).await
    }

    async fn send_raw(&self, msg: &Value) -> Result<(), LspError> {
        let body = serde_json::to_vec(msg).map_err(|e| LspError::Rpc(e.to_string()))?;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(header.as_bytes()).await?;
        stdin.write_all(&body).await?;
        stdin.flush().await?;
        Ok(())
    }
}

/// Content-Length フレーム 1 件を読む。EOF なら Ok(None)。
async fn read_message<R: AsyncReadExt + Unpin>(
    reader: &mut R,
) -> Result<Option<Value>, std::io::Error> {
    // 1) ヘッダ部分を読む。Content-Length: <n>\r\n のあと \r\n で本体に入る。
    let mut header = Vec::new();
    let mut prev = 0u8;
    let mut prev2 = 0u8;
    let mut prev3 = 0u8;
    loop {
        let mut byte = [0u8; 1];
        let n = reader.read(&mut byte).await?;
        if n == 0 {
            return Ok(None);
        }
        header.push(byte[0]);
        // \r\n\r\n を検出
        if prev3 == b'\r' && prev2 == b'\n' && prev == b'\r' && byte[0] == b'\n' {
            break;
        }
        prev3 = prev2;
        prev2 = prev;
        prev = byte[0];
    }

    let header_str = String::from_utf8_lossy(&header);
    let mut content_length: usize = 0;
    for line in header_str.split("\r\n") {
        if let Some(rest) = line.strip_prefix("Content-Length:") {
            content_length = rest.trim().parse().unwrap_or(0);
        }
    }
    if content_length == 0 {
        return Ok(None);
    }

    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body).await?;
    let v = serde_json::from_slice::<Value>(&body)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(Some(v))
}
