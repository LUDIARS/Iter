//! ファイルウィンドウの open / 既存ウィンドウへのフォーカス。
//!
//! 1 ファイル = 1 OS ウインドウ。`label` を `file-<hash>` で安定化させて、
//! 既に開いていれば前面に出すだけ。新規なら `WebviewWindowBuilder` で生成。

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

#[tauri::command]
pub async fn open_file_window(app: AppHandle, path: String) -> Result<(), String> {
    let label = label_for(&path);

    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.set_focus();
        return Ok(());
    }

    // path はクエリで file-window.html に渡す。WebviewUrl::App は frontendDist
    // 配下からの相対パス (+ クエリ) を受け取る。
    let webview_url = WebviewUrl::App(
        format!("file-window.html?path={}", encode_query(&path)).into(),
    );

    let title = std::path::Path::new(&path)
        .file_name()
        .map(|s| format!("Iter — {}", s.to_string_lossy()))
        .unwrap_or_else(|| format!("Iter — {}", path));

    WebviewWindowBuilder::new(&app, label, webview_url)
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
