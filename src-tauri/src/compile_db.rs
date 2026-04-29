//! `compile_commands.json` の検知 / 自動生成。
//!
//! 検知ルール (優先順):
//!   1. `<root>/compile_commands.json`
//!   2. `<root>/build*/compile_commands.json` (浅く探索、`out/`, `cmake-build-debug/` 等)
//!   3. CMakeLists.txt 有り + `cmake` コマンド有り → `<root>/build/` に生成
//!   4. CMakeLists.txt 無し → 仮想 CMakeLists を `<root>/.iter/CMakeLists.txt` に
//!      生成して `<root>/.iter/build/compile_commands.json` まで作る:
//!         - `cmake` が PATH にある → cmake 経由で生成
//!         - `cmake` が無い → 直接 `compile_commands.json` を書き出す (フォールバック)
//!
//! 仮想生成では root 配下の `.cpp/.cc/.cxx/.c` を source、`.h/.hpp/.hxx` の親
//! ディレクトリを include path として収集する。`.git`、`node_modules`、`build`、
//! `out`、`target`、`.iter` などは除外。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const VIRTUAL_DIR: &str = ".iter";
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
    if root.join("CMakeLists.txt").exists() {
        return generate_via_cmake(root, root, &root.join("build"));
    }
    // CMakeLists 不在 → 仮想生成
    ensure_virtual_compile_commands(root)
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

/// `<root>/.iter/CMakeLists.txt` を生成し、可能なら cmake で
/// compile_commands.json まで作る。cmake が無ければ直接生成する。
pub fn ensure_virtual_compile_commands(root: &Path) -> Result<PathBuf, String> {
    let virtual_dir = root.join(VIRTUAL_DIR);
    std::fs::create_dir_all(&virtual_dir).map_err(|e| format!(".iter dir 作成失敗: {e}"))?;
    write_iter_gitignore(&virtual_dir);

    let scan = scan_sources(root)?;
    if scan.sources.is_empty() {
        return Err(format!(
            "C/C++ ソースファイルが root 配下に見つかりません ({} を探索)",
            root.display()
        ));
    }

    // 1) 仮想 CMakeLists.txt を生成 — cmake が無くても残しておけばユーザが手動で
    //    使える + clangd の `--compile-commands-dir` で参照される可能性
    write_virtual_cmakelists(&virtual_dir, root, &scan)?;

    // 2) cmake があれば呼び出して compile_commands.json を作る
    let virtual_build = virtual_dir.join("build");
    if which::which("cmake").is_ok() {
        std::fs::create_dir_all(&virtual_build)
            .map_err(|e| format!(".iter/build dir 作成失敗: {e}"))?;
        if let Ok(cc) = generate_via_cmake(root, &virtual_dir, &virtual_build) {
            return Ok(cc);
        }
        // cmake 失敗時は静かに直接生成にフォールバック (cmake のエラーは将来 emit したい)
    }

    // 3) cmake が無い or 失敗 → 直接 compile_commands.json を書き出す
    write_direct_compile_commands(&virtual_build, root, &scan)
}

fn generate_via_cmake(
    source_dir: &Path,
    cmakelists_dir: &Path,
    build_dir: &Path,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(build_dir).map_err(|e| format!("build dir 作成失敗: {e}"))?;

    let cmake = which::which("cmake").map_err(|_| {
        "cmake コマンドが PATH に見つかりません (CMake をインストールするか、既存の compile_commands.json を root か build/ に置いてください)".to_string()
    })?;

    let output = Command::new(cmake)
        .arg("-B")
        .arg(build_dir)
        .arg("-S")
        .arg(cmakelists_dir)
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
            "cmake は完了したが {} が生成されませんでした (source_dir={})",
            cc.display(),
            source_dir.display()
        ));
    }
    Ok(cc)
}

/// 走査結果。source ファイルの absolute path と include 候補ディレクトリ。
pub struct ScanResult {
    pub sources: Vec<PathBuf>,
    pub includes: BTreeSet<PathBuf>,
}

pub fn scan_sources(root: &Path) -> Result<ScanResult, String> {
    let mut sources = Vec::new();
    let mut includes = BTreeSet::new();
    walk(root, root, 0, &mut sources, &mut includes)
        .map_err(|e| format!("ソース走査失敗: {e}"))?;
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

fn write_iter_gitignore(virtual_dir: &Path) {
    let gi = virtual_dir.join(".gitignore");
    if !gi.exists() {
        let _ = std::fs::write(&gi, "# Iter が生成したファイル\n*\n");
    }
}

fn write_virtual_cmakelists(
    virtual_dir: &Path,
    root: &Path,
    scan: &ScanResult,
) -> Result<(), String> {
    let mut s = String::new();
    s.push_str("# Auto-generated by Iter — do not edit by hand.\n");
    s.push_str("# Regenerated whenever the user opens a project without CMakeLists.txt.\n");
    s.push_str("cmake_minimum_required(VERSION 3.10)\n");
    s.push_str("project(IterVirtualProject LANGUAGES C CXX)\n");
    s.push_str("set(CMAKE_EXPORT_COMPILE_COMMANDS ON)\n");
    s.push_str("set(CMAKE_CXX_STANDARD 17)\n");
    s.push_str("set(CMAKE_CXX_STANDARD_REQUIRED ON)\n");
    s.push_str("set(CMAKE_C_STANDARD 11)\n\n");

    s.push_str("add_library(iter_virtual STATIC\n");
    for src in &scan.sources {
        let rel = src.strip_prefix(root).unwrap_or(src);
        s.push_str("  \"");
        s.push_str(&cmake_path(rel));
        s.push_str("\"\n");
    }
    s.push_str(")\n\n");

    if !scan.includes.is_empty() {
        s.push_str("target_include_directories(iter_virtual PRIVATE\n");
        for inc in &scan.includes {
            let rel = inc.strip_prefix(root).unwrap_or(inc);
            // root 自身は \"\" に潰れるので . で表記
            let rel_s = cmake_path(rel);
            let rel_s = if rel_s.is_empty() {
                ".".to_string()
            } else {
                rel_s
            };
            s.push_str("  \"");
            s.push_str(&rel_s);
            s.push_str("\"\n");
        }
        s.push_str(")\n");
    }

    let target = virtual_dir.join("CMakeLists.txt");
    std::fs::write(&target, s).map_err(|e| format!("仮想 CMakeLists 書き込み失敗: {e}"))?;
    Ok(())
}

/// CMakeLists 内のパス区切りはバックスラッシュを使うとエスケープ問題が出るため
/// 強制的にスラッシュへ正規化する。
fn cmake_path(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

fn write_direct_compile_commands(
    build_dir: &Path,
    _root: &Path,
    scan: &ScanResult,
) -> Result<PathBuf, String> {
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
    fn virtual_cmakelists_lists_all_sources() {
        let d = tempdir().unwrap();
        let root = d.path();
        write(&root.join("a.cpp"), "");
        write(&root.join("sub/b.cc"), "");
        write(&root.join("inc/h.h"), "");

        let virtual_dir = root.join(VIRTUAL_DIR);
        fs::create_dir_all(&virtual_dir).unwrap();
        let scan = scan_sources(root).unwrap();
        write_virtual_cmakelists(&virtual_dir, root, &scan).unwrap();

        let body = fs::read_to_string(virtual_dir.join("CMakeLists.txt")).unwrap();
        assert!(body.contains("project(IterVirtualProject"));
        assert!(body.contains("CMAKE_EXPORT_COMPILE_COMMANDS ON"));
        assert!(body.contains("a.cpp"));
        assert!(body.contains("sub/b.cc"));
        // include path に inc / sub / (root 自体)
        assert!(body.contains("\"inc\""));
        assert!(body.contains("\"sub\""));
    }

    #[test]
    fn direct_compile_commands_has_entry_per_source() {
        let d = tempdir().unwrap();
        let root = d.path();
        write(&root.join("a.cpp"), "");
        write(&root.join("b.c"), "");
        write(&root.join("inc/h.h"), "");

        let scan = scan_sources(root).unwrap();
        let build_dir = root.join(VIRTUAL_DIR).join("build");
        let cc = write_direct_compile_commands(&build_dir, root, &scan).unwrap();
        let body = fs::read_to_string(&cc).unwrap();

        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        let arr = v.as_array().expect("array");
        assert_eq!(arr.len(), 2);
        // a.cpp → clang++、b.c → clang
        let cmds: Vec<&str> = arr
            .iter()
            .map(|e| e.get("command").unwrap().as_str().unwrap())
            .collect();
        assert!(cmds.iter().any(|c| c.starts_with("clang++") && c.contains("a.cpp")));
        assert!(cmds.iter().any(|c| c.starts_with("clang ") && c.contains("b.c")));
        // include path が反映
        assert!(cmds.iter().all(|c| c.contains("-I")));
    }

    #[test]
    fn ensure_virtual_creates_files_when_no_cmakelists() {
        let d = tempdir().unwrap();
        let root = d.path();
        write(&root.join("main.cpp"), "");

        let cc = ensure_virtual_compile_commands(root).unwrap();
        // CMakeLists が .iter/ 配下に出来る (cmake が無くても直書きで build/ は出来る)
        assert!(root.join(".iter/CMakeLists.txt").exists());
        // cmake が無くても build/compile_commands.json は出来る
        assert!(cc.exists());
        let body = std::fs::read_to_string(&cc).unwrap();
        assert!(body.contains("main.cpp"));
    }

    #[test]
    fn ensure_virtual_errors_when_no_sources() {
        let d = tempdir().unwrap();
        let err = ensure_virtual_compile_commands(d.path()).unwrap_err();
        assert!(err.contains("ソースファイル"));
    }
}
