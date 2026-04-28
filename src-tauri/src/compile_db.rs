//! `compile_commands.json` の検知 / 自動生成。
//!
//! 検知ルール (優先順):
//!   1. `<root>/compile_commands.json`
//!   2. `<root>/build/compile_commands.json`
//!   3. `<root>/build*/compile_commands.json` のいずれか (`out/`, `cmake-build-debug/` 等)
//!
//! どこにもない場合は **CMake を呼び出して** `build/` に生成する。
//! `cmake -B build -DCMAKE_EXPORT_COMPILE_COMMANDS=ON <root>` を実行するだけ。
//! 失敗時はエラーを返してフロントに伝える。

use std::path::{Path, PathBuf};
use std::process::Command;

/// compile_commands.json の場所を確定する。なければ生成する。
pub fn ensure_compile_commands(root: &Path) -> Result<PathBuf, String> {
    if let Some(p) = find_existing(root) {
        return Ok(p);
    }
    if !root.join("CMakeLists.txt").exists() {
        return Err(
            "CMakeLists.txt が無いため compile_commands.json を自動生成できません".to_string(),
        );
    }
    generate_via_cmake(root)
}

fn find_existing(root: &Path) -> Option<PathBuf> {
    let direct = root.join("compile_commands.json");
    if direct.exists() {
        return Some(direct);
    }
    // build*/compile_commands.json を浅く探す (深さ 1)
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        let p = entry.path();
        if p.is_dir() {
            let cand = p.join("compile_commands.json");
            if cand.exists() {
                return Some(cand);
            }
        }
    }
    None
}

fn generate_via_cmake(root: &Path) -> Result<PathBuf, String> {
    let build_dir = root.join("build");
    std::fs::create_dir_all(&build_dir).map_err(|e| format!("build dir 作成失敗: {e}"))?;

    let cmake = which::which("cmake").map_err(|_| {
        "cmake コマンドが PATH に見つかりません (CMake をインストールするか、既存の compile_commands.json を root か build/ に置いてください)".to_string()
    })?;

    let output = Command::new(cmake)
        .arg("-B")
        .arg(&build_dir)
        .arg("-S")
        .arg(root)
        .arg("-DCMAKE_EXPORT_COMPILE_COMMANDS=ON")
        .output()
        .map_err(|e| format!("cmake 起動失敗: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("cmake 失敗 (exit {}): {}", output.status, stderr));
    }

    let cc = build_dir.join("compile_commands.json");
    if !cc.exists() {
        return Err(format!(
            "cmake は完了したが {} が生成されませんでした",
            cc.display()
        ));
    }
    Ok(cc)
}
