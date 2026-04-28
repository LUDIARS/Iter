//! ファイルウィンドウの open / 既存ウィンドウへのフォーカス。
//!
//! 1 ファイル = 1 OS ウインドウ。`label` を `file-<hash>` で安定化させて、
//! 既に開いていれば前面に出すだけ。新規なら `WebviewWindowBuilder` で生成。
//!
//! `open_at` は `path + line + col` を受け取って、対応ウィンドウを (新規 or 再利用)
//! 開きつつ、フロントへ位置情報を eve で投げる。フロント側は監聴し、
//! Monaco を該当行へ scroll + (オプションで) 宣言追跡をかける。

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

#[derive(Debug, Clone, Serialize)]
struct OpenAtPayload {
    path: String,
    line: u32,
    column: u32,
    follow_definition: bool,
}

#[tauri::command]
pub async fn open_file_window(app: AppHandle, path: String) -> Result<(), String> {
    open_internal(&app, &path, None, None, false).await
}

/// `path + line + (column?)` で開く。`follow_definition=true` なら、
/// フロントが Monaco mount 後に LSP の textDocument/definition を呼んで、
/// 結果が現在ファイル外を指していたらさらにそちらを `open_at` する。
#[tauri::command]
pub async fn open_at(
    app: AppHandle,
    path: String,
    line: u32,
    column: Option<u32>,
    follow_definition: Option<bool>,
) -> Result<(), String> {
    open_internal(
        &app,
        &path,
        Some(line),
        column,
        follow_definition.unwrap_or(false),
    )
    .await
}

async fn open_internal(
    app: &AppHandle,
    path: &str,
    line: Option<u32>,
    column: Option<u32>,
    follow_definition: bool,
) -> Result<(), String> {
    let label = label_for(path);

    // 既開ウィンドウなら前面 + イベント送信のみ
    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.set_focus();
        if let Some(l) = line {
            let payload = OpenAtPayload {
                path: path.to_string(),
                line: l,
                column: column.unwrap_or(0),
                follow_definition,
            };
            let _ = existing.emit("iter://open-at", &payload);
        }
        return Ok(());
    }

    // 新規ウィンドウ。クエリ経由で path/line/col/follow を投げる。
    let mut query = format!("path={}", encode_query(path));
    if let Some(l) = line {
        query.push_str(&format!("&line={}", l));
    }
    if let Some(c) = column {
        query.push_str(&format!("&col={}", c));
    }
    if follow_definition {
        query.push_str("&follow=1");
    }
    let webview_url = WebviewUrl::App(format!("file-window.html?{}", query).into());

    let title = std::path::Path::new(path)
        .file_name()
        .map(|s| format!("Iter — {}", s.to_string_lossy()))
        .unwrap_or_else(|| format!("Iter — {}", path));

    WebviewWindowBuilder::new(app, label, webview_url)
        .title(title)
        .inner_size(1100.0, 780.0)
        .min_inner_size(640.0, 400.0)
        .resizable(true)
        .build()
        .map_err(|e| e.to_string())?;

    Ok(())
}

fn label_for(path: &str) -> String {
    let mut h = DefaultHasher::new();
    path.hash(&mut h);
    format!("file-{:x}", h.finish())
}

fn encode_query(s: &str) -> String {
    // `URLSearchParams` 互換のシンプルな %-encode (空白は %20)
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
