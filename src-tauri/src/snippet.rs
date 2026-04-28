//! ファイルの一部行 (target ± context) を返す軽量コマンド。
//!
//! 関連グラフのカードに表示するスニペットを取りに行く。
//! `read_snippet(path, line=42, context=5)` だと 37〜47 行を返す。
//! Monaco を起動せず、軽量に多数のカードを描画するための API。

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Snippet {
    /// 0-based の最初の行番号
    pub start_line: u32,
    /// クエリされた行 (0-based)
    pub target_line: u32,
    pub lines: Vec<String>,
}

#[tauri::command]
pub fn read_snippet(path: String, line: u32, context: u32) -> Result<Snippet, String> {
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read: {e}"))?;
    let all: Vec<&str> = text.split('\n').collect();
    let total = all.len();
    let target = line as usize;
    let ctx = context as usize;
    let start = target.saturating_sub(ctx);
    let end = (target + ctx + 1).min(total);
    let lines = all[start..end].iter().map(|s| s.to_string()).collect();
    Ok(Snippet {
        start_line: start as u32,
        target_line: target as u32,
        lines,
    })
}
