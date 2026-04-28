//! プロジェクト検知 + ファイルツリー列挙。
//!
//! MVP では指定ディレクトリ直下〜数階層を走査し、CMakeLists.txt または
//! `*.vcxproj` の有無で build_system を判定する。compile_commands.json の
//! 自動生成は Phase 2 (clangd 統合と一緒に入れる)。

use serde::Serialize;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
pub enum ProjectError {
    #[error("path not found: {0}")]
    NotFound(String),
    #[error("path is not a directory: {0}")]
    NotADirectory(String),
    #[error("io: {0}")]
    Io(String),
}

impl From<std::io::Error> for ProjectError {
    fn from(e: std::io::Error) -> Self {
        ProjectError::Io(e.to_string())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildSystem {
    Cmake,
    Vcxproj,
    Unknown,
}

#[derive(Debug, Serialize)]
pub struct ProjectInfo {
    pub root: String,
    pub build_system: BuildSystem,
    pub files: Vec<FileNode>,
}

#[derive(Debug, Serialize)]
pub struct FileNode {
    pub path: String,
    pub rel: String,
    pub name: String,
    pub is_dir: bool,
    pub children: Vec<FileNode>,
}

/// MVP の走査ルール:
/// - 隠しディレクトリ (`.git` 等) は除外
/// - `node_modules`, `target`, `build`, `dist`, `.cache` 等のビルド成果物も除外
/// - 深さ制限 8 (現実的な C++ プロジェクトを覆える)
const SKIP_DIR_NAMES: &[&str] = &[
    "node_modules",
    "target",
    "build",
    "out",
    "dist",
    ".cache",
    ".git",
    ".vs",
    ".vscode",
    ".idea",
];
const MAX_DEPTH: usize = 8;

#[tauri::command]
pub fn detect_project(root: String) -> Result<ProjectInfo, ProjectError> {
    let root_path = PathBuf::from(&root);
    if !root_path.exists() {
        return Err(ProjectError::NotFound(root));
    }
    if !root_path.is_dir() {
        return Err(ProjectError::NotADirectory(root));
    }

    let build_system = detect_build_system(&root_path);
    let files = walk(&root_path, &root_path, 0)?;

    Ok(ProjectInfo {
        root: root_path.to_string_lossy().into_owned(),
        build_system,
        files,
    })
}

fn detect_build_system(root: &Path) -> BuildSystem {
    if root.join("CMakeLists.txt").exists() {
        return BuildSystem::Cmake;
    }
    // *.vcxproj が直下にあるか軽く見る (再帰探索はしない、速度優先)
    if let Ok(entries) = std::fs::read_dir(root) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_file()
                && p.extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s.eq_ignore_ascii_case("vcxproj"))
                    .unwrap_or(false)
            {
                return BuildSystem::Vcxproj;
            }
        }
    }
    BuildSystem::Unknown
}

fn walk(base: &Path, dir: &Path, depth: usize) -> Result<Vec<FileNode>, ProjectError> {
    if depth > MAX_DEPTH {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .flatten()
        .filter(|e| {
            let name = e.file_name();
            let s = name.to_string_lossy();
            if s.starts_with('.') {
                return false;
            }
            !SKIP_DIR_NAMES.iter().any(|skip| s.eq_ignore_ascii_case(skip))
        })
        .collect();
    // ディレクトリ → ファイル の順、それぞれ名前 ASCII でソート
    entries.sort_by_key(|e| {
        (
            !e.file_type().map(|t| t.is_dir()).unwrap_or(false),
            e.file_name().to_string_lossy().to_lowercase(),
        )
    });

    for e in entries {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        let rel = p
            .strip_prefix(base)
            .map(|r| r.to_string_lossy().into_owned())
            .unwrap_or_else(|_| name.clone());
        let is_dir = p.is_dir();
        let children = if is_dir {
            walk(base, &p, depth + 1)?
        } else {
            Vec::new()
        };
        out.push(FileNode {
            path: p.to_string_lossy().into_owned(),
            rel,
            name,
            is_dir,
            children,
        });
    }
    Ok(out)
}
