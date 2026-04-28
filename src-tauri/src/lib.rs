mod cache;
mod compile_db;
mod lsp;
mod lsp_commands;
mod project;
mod snippet;
mod stack_trace;
mod window;

use lsp_commands::LspState;

/// Tauri 2 entrypoint.
///
/// Plugins (`dialog`, `fs`) を有効化、LSP 用 state (clangd ハンドル) を `manage`
/// で抱え、フロントエンドが叩く `#[tauri::command]` 群を `invoke_handler` に登録。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(LspState::default())
        .invoke_handler(tauri::generate_handler![
            project::detect_project,
            project::refresh_project,
            window::open_file_window,
            window::open_at,
            window::close_other_windows,
            lsp_commands::lsp_open_project,
            lsp_commands::lsp_open_file,
            lsp_commands::lsp_call_hierarchy,
            lsp_commands::lsp_references,
            stack_trace::parse_stack_trace,
            snippet::read_snippet,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
