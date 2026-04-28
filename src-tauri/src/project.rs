//! プロジェクト検知 + ファイルツリー列挙。
//!
//! - 指定ディレクトリ直下〜数階層を走査して CMakeLists.txt / `*.vcxproj` の
//!   有無で build_system を判定する。
//! - 結果はディスクキャッシュに保存し、次回は root mtime 一致で即返す
//!   (`refresh_project` で明示破棄可能)。
//! - compile_commands.json の自動生成は `lsp_open_project` 側で行う。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::cache;

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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildSystem {
    Cmake,
    Vcxproj,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub root: String,
    pub build_system: BuildSystem,
    pub files: Vec<FileNode>,
    /// このプロジェクトの結果がキャッシュからきたかどうか (フロント表示用)
    #[serde(default)]
    pub from_cache: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileNode {
    pub path: String,
    pub rel: String,
    pub name: String,
    pub is_dir: bool,
    pub children: Vec<FileNode>,
}

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

    // キャッシュヒット (root mtime 一致) なら即返す
    if let Some(mut cached) = cache::try_load::<ProjectInfo>(&root_path) {
        cached.from_cache = true;
        return Ok(cached);
    }

    let info = walk_root(&root_path)?;
    // キャッシュ保存に失敗しても致命ではない (権限不足など) — 続行
    let _ = cache::save(&root_path, &info);
    Ok(info)
}

/// 明示再走査 (キャッシュを破棄してから走査)。
#[tauri::command]
pub fn refresh_project(root: String) -> Result<ProjectInfo, ProjectError> {
    let root_path = PathBuf::from(&root);
    if !root_path.exists() {
        return Err(ProjectError::NotFound(root));
    }
    cache::invalidate(&root_path);
    let info = walk_root(&root_path)?;
    let _ = cache::save(&root_path, &info);
    Ok(info)
}

fn walk_root(root_path: &Path) -> Result<ProjectInfo, ProjectError> {
    let build_system = detect_build_system(root_path);
    let files = walk(root_path, root_path, 0)?;
    Ok(ProjectInfo {
        root: root_path.to_string_lossy().into_owned(),
        build_system,
        files,
        from_cache: false,
    })
}

fn detect_build_system(root: &Path) -> BuildSystem {
    if root.join("CMakeLists.txt").exists() {
        return BuildSystem::Cmake;
    }
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
