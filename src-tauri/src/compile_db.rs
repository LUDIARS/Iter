//! `compile_commands.json` の検知 / 安全な自動生成。
//!
//! 検知ルール (優先順):
//!   1. `<root>/compile_commands.json`
//!   2. `<root>/build*/compile_commands.json` (浅く探索、`out/`, `cmake-build-debug/` 等)
//!   3. `<root>/.iter/build/compile_commands.json` を source 走査から直接生成
//!
//! CMakeLists.txt は対象ワークスペースが所有する入力なので、プロジェクトを開く際には
//! 一切読み込まず、解釈も実行もしない。生成時は root 配下の `.cpp/.cc/.cxx/.c` を
//! source、`.h/.hpp/.hxx` の親ディレクトリを include path として収集する。`.git`、
//! `node_modules`、`build`、`out`、`target`、`.iter` などは除外。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const ITER_DIR: &str = ".iter";
const SKIP_DIRS: &[&str] = &[
    ".git",
    ".iter",
    ".cache",
    ".vs",
    ".vscode",
    ".idea",
    "node_modules",
    "target",
    "build",
    "out",
    "dist",
];
const SOURCE_EXTS: &[&str] = &["cpp", "cc", "cxx", "c"];
const HEADER_EXTS: &[&str] = &["h", "hpp", "hxx", "hh"];

/// compile_commands.json の場所を確定する。なければ生成する。
pub fn ensure_compile_commands(root: &Path) -> Result<PathBuf, String> {
    if let Some(p) = find_existing(root) {
        return Ok(p);
    }
    ensure_scanned_compile_commands(root)
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

/// source 走査から `<root>/.iter/build/compile_commands.json` を直接生成する。
///
/// この関数は CMakeLists.txt を開かない。プロジェクトを選ぶだけで対象
/// ワークスペースの CMake コードが実行されないようにするためである。
fn ensure_scanned_compile_commands(root: &Path) -> Result<PathBuf, String> {
    let iter_dir = root.join(ITER_DIR);
    std::fs::create_dir_all(&iter_dir).map_err(|e| format!(".iter dir 作成失敗: {e}"))?;
    write_iter_gitignore(&iter_dir);

    let scan = scan_sources(root)?;
    if scan.sources.is_empty() {
        return Err(format!(
            "C/C++ ソースファイルが root 配下に見つかりません ({} を探索)",
            root.display()
        ));
    }

    write_direct_compile_commands(&iter_dir.join("build"), &scan)
}

/// 走査結果。source ファイルの absolute path と include 候補ディレクトリ。
pub struct ScanResult {
    pub sources: Vec<PathBuf>,
    pub includes: BTreeSet<PathBuf>,
}

pub fn scan_sources(root: &Path) -> Result<ScanResult, String> {
    let mut sources = Vec::new();
    let mut includes = BTreeSet::new();
    walk(root, root, 0, &mut sources, &mut includes).map_err(|e| format!("ソース走査失敗: {e}"))?;
    sources.sort();
    Ok(ScanResult { sources, includes })
}

fn walk(
    base: &Path,
    dir: &Path,
    depth: usize,
    sources: &mut Vec<PathBuf>,
    includes: &mut BTreeSet<PathBuf>,
) -> std::io::Result<()> {
    if depth > 16 {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_s = name.to_string_lossy();
        if name_s.starts_with('.') && name_s != "." {
            // ドットディレクトリは原則スキップ。`.iter` を含むので明示
            continue;
        }
        if path.is_dir() {
            if SKIP_DIRS.iter().any(|s| name_s.eq_ignore_ascii_case(s)) {
                continue;
            }
            walk(base, &path, depth + 1, sources, includes)?;
            continue;
        }
        let ext_lower = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase());
        let ext = match ext_lower.as_deref() {
            Some(e) => e,
            None => continue,
        };
        if SOURCE_EXTS.iter().any(|s| *s == ext) {
            sources.push(path.clone());
            // ソースの親も include 候補に (相対 #include 用)
            if let Some(parent) = path.parent() {
                includes.insert(parent.to_path_buf());
            }
        } else if HEADER_EXTS.iter().any(|s| *s == ext) {
            if let Some(parent) = path.parent() {
                includes.insert(parent.to_path_buf());
            }
        }
    }
    Ok(())
}

fn write_iter_gitignore(iter_dir: &Path) {
    let gi = iter_dir.join(".gitignore");
    if !gi.exists() {
        let _ = std::fs::write(&gi, "# Iter が生成したファイル\n*\n");
    }
}

fn write_direct_compile_commands(build_dir: &Path, scan: &ScanResult) -> Result<PathBuf, String> {
    std::fs::create_dir_all(build_dir).map_err(|e| format!("build dir 作成失敗: {e}"))?;

    let mut entries = Vec::with_capacity(scan.sources.len());
    let directory = build_dir.to_string_lossy().replace('\\', "/");
    let include_args: Vec<String> = scan
        .includes
        .iter()
        .map(|p| format!("-I{}", p.to_string_lossy().replace('\\', "/")))
        .collect();

    for src in &scan.sources {
        let file = src.to_string_lossy().replace('\\', "/");
        let is_c = src
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("c"))
            .unwrap_or(false);
        let compiler = if is_c { "clang" } else { "clang++" };
        let std_arg = if is_c { "-std=c11" } else { "-std=c++17" };
        let mut command = String::new();
        command.push_str(compiler);
        command.push(' ');
        command.push_str(std_arg);
        command.push_str(" -c");
        for inc in &include_args {
            command.push(' ');
            command.push_str(inc);
        }
        command.push(' ');
        command.push_str(&file);

        entries.push(serde_json::json!({
            "directory": directory,
            "command": command,
            "file": file,
        }));
    }

    let cc = build_dir.join("compile_commands.json");
    let body = serde_json::to_vec_pretty(&entries)
        .map_err(|e| format!("compile_commands.json 直書き失敗: {e}"))?;
    std::fs::write(&cc, body).map_err(|e| format!("compile_commands.json 書き込み失敗: {e}"))?;
    Ok(cc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, body).unwrap();
    }

    #[test]
    fn scan_collects_sources_and_includes() {
        let d = tempdir().unwrap();
        let root = d.path();
        write(&root.join("src/main.cpp"), "");
        write(&root.join("src/util.cc"), "");
        write(&root.join("src/util.h"), "");
        write(&root.join("include/api.hpp"), "");
        write(&root.join("build/garbage.cpp"), ""); // SKIP_DIRS で除外
        write(&root.join(".iter/skip.cpp"), ""); // 同上
        write(&root.join("README.md"), ""); // 拡張子 not source

        let scan = scan_sources(root).unwrap();
        let names: Vec<String> = scan
            .sources
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"main.cpp".to_string()));
        assert!(names.contains(&"util.cc".to_string()));
        assert!(!names.contains(&"garbage.cpp".to_string()));
        assert!(!names.contains(&"skip.cpp".to_string()));

        // include に src/ と include/ の両方
        let inc_names: Vec<String> = scan
            .includes
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert!(inc_names.contains(&"src".to_string()));
        assert!(inc_names.contains(&"include".to_string()));
    }

    #[test]
    fn uses_existing_compile_database_before_generating_one() {
        let d = tempdir().unwrap();
        let root = d.path();
        let existing = root.join("compile_commands.json");
        write(&existing, "[]");

        assert_eq!(ensure_compile_commands(root).unwrap(), existing);
        assert!(!root.join(ITER_DIR).exists());
    }

    #[test]
    fn direct_compile_commands_has_entry_per_source() {
        let d = tempdir().unwrap();
        let root = d.path();
        write(&root.join("a.cpp"), "");
        write(&root.join("b.c"), "");
        write(&root.join("inc/h.h"), "");

        let scan = scan_sources(root).unwrap();
        let build_dir = root.join(ITER_DIR).join("build");
        let cc = write_direct_compile_commands(&build_dir, &scan).unwrap();
        let body = fs::read_to_string(&cc).unwrap();

        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        let arr = v.as_array().expect("array");
        assert_eq!(arr.len(), 2);
        // a.cpp → clang++、b.c → clang
        let cmds: Vec<&str> = arr
            .iter()
            .map(|e| e.get("command").unwrap().as_str().unwrap())
            .collect();
        assert!(cmds
            .iter()
            .any(|c| c.starts_with("clang++") && c.contains("a.cpp")));
        assert!(cmds
            .iter()
            .any(|c| c.starts_with("clang ") && c.contains("b.c")));
        // include path が反映
        assert!(cmds.iter().all(|c| c.contains("-I")));
    }

    #[test]
    fn ensure_never_executes_target_cmakelists() {
        let d = tempdir().unwrap();
        let root = d.path();
        write(&root.join("main.cpp"), "");
        let sentinel = root.join("cmake-executed");
        write(
            &root.join("CMakeLists.txt"),
            r#"execute_process(
  COMMAND "${CMAKE_COMMAND}" -E touch "${CMAKE_CURRENT_LIST_DIR}/cmake-executed"
)"#,
        );

        let cc = ensure_compile_commands(root).unwrap();
        assert_eq!(cc, root.join(".iter/build/compile_commands.json"));
        assert!(cc.exists());
        assert!(!sentinel.exists());
        let body = std::fs::read_to_string(&cc).unwrap();
        assert!(body.contains("main.cpp"));
    }

    #[test]
    fn ensure_errors_when_no_sources() {
        let d = tempdir().unwrap();
        let err = ensure_compile_commands(d.path()).unwrap_err();
        assert!(err.contains("ソースファイル"));
    }
}
