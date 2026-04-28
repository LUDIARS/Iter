// Tauri 2 では実装は lib.rs に置き、main.rs はそこに委譲するのがイディオム。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    iter_lib::run();
}
