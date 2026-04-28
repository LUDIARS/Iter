//! プロジェクト走査結果のディスクキャッシュ。
//!
//! - 保存先: `<config_dir>/iter/projects/<hash>.json`
//!   - Windows: `%APPDATA%/iter/projects/`
//!   - Linux:   `$XDG_CONFIG_HOME/iter/projects/` (既定 `~/.config/iter/projects/`)
//!   - macOS:   `~/Library/Application Support/iter/projects/`
//! - キー: project root の絶対パスを SHA-like (DefaultHasher) でハッシュ
//! - 妥当性: cache 中の `root_mtime` と現在の root dir mtime を比較。一致なら fresh。
//!
//! `detect_project` 側からは `try_load` → ヒットなら即返す、無ければ通常走査して
//! `save` する流れで使う。

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Serialize, Deserialize)]
pub struct CachedProject<T: Serialize> {
    pub root: String,
    pub root_mtime_secs: u64,
    pub cached_at_secs: u64,
    pub version: u32,
    pub project: T,
}

const CACHE_VERSION: u32 = 1;

fn cache_dir() -> Option<PathBuf> {
    dirs_like_config_dir().map(|d| d.join("iter").join("projects"))
}

/// `dirs` クレートを入れずに OS ごとの config dir を返す軽量実装。
fn dirs_like_config_dir() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| {
            let mut p = PathBuf::from(h);
            p.push("Library/Application Support");
            p
        })
    } else {
        // Linux 系: XDG_CONFIG_HOME → ~/.config
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|h| {
                    let mut p = PathBuf::from(h);
                    p.push(".config");
                    p
                })
            })
    }
}

fn cache_path(root: &Path) -> Option<PathBuf> {
    let dir = cache_dir()?;
    let mut h = DefaultHasher::new();
    root.to_string_lossy().to_lowercase().hash(&mut h);
    Some(dir.join(format!("{:x}.json", h.finish())))
}

fn root_mtime_secs(root: &Path) -> u64 {
    std::fs::metadata(root)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// fresh なキャッシュがあれば返す。なければ None。
pub fn try_load<T>(root: &Path) -> Option<T>
where
    T: for<'de> Deserialize<'de>,
{
    let path = cache_path(root)?;
    let raw = std::fs::read(&path).ok()?;
    // ジェネリック T を介すため一度 Value で読んで version + mtime を確認
    let v: serde_json::Value = serde_json::from_slice(&raw).ok()?;
    let version = v.get("version")?.as_u64()? as u32;
    if version != CACHE_VERSION {
        return None;
    }
    let cached_root_mtime = v.get("root_mtime_secs")?.as_u64()?;
    if cached_root_mtime != root_mtime_secs(root) {
        return None; // root の更新時刻が変わっていればキャッシュは古い
    }
    let project_json = v.get("project")?.clone();
    serde_json::from_value::<T>(project_json).ok()
}

/// project を保存。失敗しても致命ではないので Result は string で返す。
pub fn save<T: Serialize>(root: &Path, project: &T) -> Result<(), String> {
    let path = cache_path(root).ok_or("cache dir 不明")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let payload = CachedProject {
        root: root.to_string_lossy().into_owned(),
        root_mtime_secs: root_mtime_secs(root),
        cached_at_secs: now_secs(),
        version: CACHE_VERSION,
        project,
    };
    let body = serde_json::to_vec_pretty(&payload).map_err(|e| format!("serialize: {e}"))?;
    std::fs::write(&path, body).map_err(|e| format!("write: {e}"))?;
    Ok(())
}

/// キャッシュを明示破棄 (`refresh_project` 等から)。
pub fn invalidate(root: &Path) {
    if let Some(p) = cache_path(root) {
        let _ = std::fs::remove_file(p);
    }
}
