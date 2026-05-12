//! プロジェクト走査結果のディスクキャッシュ。
//!
//! - 保存先: `<config_dir>/iter/projects/<hash>.json`
//!   - Windows: `%APPDATA%/iter/projects/`
//!   - Linux:   `$XDG_CONFIG_HOME/iter/projects/` (既定 `~/.config/iter/projects/`)
//!   - macOS:   `~/Library/Application Support/iter/projects/`
//! - キー: project root の絶対パスを SHA-like (DefaultHasher) でハッシュ
//! - 妥当性: cache 中の `signature` と現在の project signature を比較。
//!   signature は root mtime に加えて、 関連ファイル (CMakeLists / *.sln /
//!   *.vcxproj / *.csproj / *.c / *.cc / *.cpp / *.cxx / *.h / *.hpp / *.hxx)
//!   の (path, mtime) を hash したもの。 子ディレクトリで source 追加・編集
//!   されると signature が変わるので cache が正しく invalidate される。
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
    /// project signature: root mtime + (path, mtime) hash of relevant files.
    /// 旧 `root_mtime_secs` は v1 cache の名残で互換のため deserialize 側でも
    /// 拾うが、 新規保存時はこちらだけ書く。
    pub signature: u64,
    pub cached_at_secs: u64,
    pub version: u32,
    pub project: T,
}

const CACHE_VERSION: u32 = 2;
const WALK_MAX_DEPTH: usize = 8;
/// 走査時に降りないディレクトリ (build 成果物 / VCS / 依存). 名前一致で skip.
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    ".git",
    ".svn",
    ".hg",
    "dist",
    "build",
    "out",
    ".iter",
    ".cache",
    ".vs",
    ".idea",
];

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

/// 走査対象とみなすファイル拡張子 + 特定ファイル名。 ここに無い拡張子は signature
/// に寄与しない (= IDE の swap file / log 等の頻繁な変更で cache が無効化しないよう絞る)。
fn is_relevant_file(path: &Path) -> bool {
    if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
        if name.eq_ignore_ascii_case("CMakeLists.txt") {
            return true;
        }
    }
    let Some(ext) = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
    else {
        return false;
    };
    matches!(
        ext.as_str(),
        "c" | "cc"
            | "cpp"
            | "cxx"
            | "h"
            | "hpp"
            | "hxx"
            | "sln"
            | "vcxproj"
            | "csproj"
            | "cmake"
    )
}

/// project signature を計算する。 走査 relevant な files の (relative-path, mtime)
/// を hash した値。 順序非依存にするため (path, mtime) のペアを一旦 Vec に集めて
/// path で sort してから hash。
///
/// root 自体の mtime は **意図的に含めない**: README 追加や `node_modules/` 作成
/// だけでも root inode mtime が変わってしまい、 関係ない変更で cache が
/// invalidate されるため。 source 系拡張子 (`is_relevant_file`) と
/// `CMakeLists.txt` の変更のみが signature に効く。
fn project_signature(root: &Path) -> u64 {
    let mut entries: Vec<(PathBuf, u64)> = Vec::new();
    collect_relevant(root, root, 0, &mut entries);
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut h = DefaultHasher::new();
    for (rel, mtime) in &entries {
        rel.to_string_lossy().hash(&mut h);
        mtime.hash(&mut h);
    }
    h.finish()
}

fn collect_relevant(root: &Path, dir: &Path, depth: usize, out: &mut Vec<(PathBuf, u64)>) {
    if depth > WALK_MAX_DEPTH {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_symlink() {
            // symlink は cycle 防止で skip
            continue;
        }
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if ft.is_dir() {
            if SKIP_DIRS.iter().any(|s| s.eq_ignore_ascii_case(name)) {
                continue;
            }
            collect_relevant(root, &path, depth + 1, out);
        } else if ft.is_file() && is_relevant_file(&path) {
            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            out.push((rel, mtime));
        }
    }
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
    // ジェネリック T を介すため一度 Value で読んで version + signature を確認
    let v: serde_json::Value = serde_json::from_slice(&raw).ok()?;
    let version = v.get("version")?.as_u64()? as u32;
    if version != CACHE_VERSION {
        return None; // 旧 v1 cache (root_mtime_secs のみ) は破棄
    }
    let cached_sig = v.get("signature")?.as_u64()?;
    if cached_sig != project_signature(root) {
        return None; // 子ディレクトリも含めた signature が変わっていればキャッシュは古い
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
        signature: project_signature(root),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread::sleep;
    use std::time::Duration;

    /// fs::metadata の mtime 反映を待つための短い sleep。
    /// Windows は mtime resolution が比較的粗いので余裕を取る。
    fn wait_mtime_tick() {
        sleep(Duration::from_millis(1100));
    }

    struct TempProject {
        path: PathBuf,
    }
    impl TempProject {
        fn new() -> Self {
            static N: AtomicUsize = AtomicUsize::new(0);
            let id = N.fetch_add(1, Ordering::SeqCst);
            let mut p = std::env::temp_dir();
            p.push(format!("iter-cache-test-{}-{}", std::process::id(), id));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            TempProject { path: p }
        }
    }
    impl Drop for TempProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn signature_changes_when_source_added_in_subdir() {
        let tp = TempProject::new();
        fs::write(tp.path.join("CMakeLists.txt"), "project(test)\n").unwrap();
        let s0 = project_signature(&tp.path);

        wait_mtime_tick();
        let sub = tp.path.join("src");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("main.cpp"), "int main() {}\n").unwrap();
        let s1 = project_signature(&tp.path);
        assert_ne!(
            s0, s1,
            "adding a relevant source in a sub-directory must change the signature"
        );
    }

    #[test]
    fn signature_unchanged_for_irrelevant_files() {
        let tp = TempProject::new();
        fs::write(tp.path.join("CMakeLists.txt"), "project(test)\n").unwrap();
        let s0 = project_signature(&tp.path);

        wait_mtime_tick();
        fs::write(tp.path.join("README.md"), "hello\n").unwrap();
        fs::write(tp.path.join("notes.txt"), "memo\n").unwrap();
        let s1 = project_signature(&tp.path);
        assert_eq!(
            s0, s1,
            "irrelevant files (README, notes) must not affect signature"
        );
    }

    #[test]
    fn signature_skips_blocklisted_directories() {
        let tp = TempProject::new();
        fs::write(tp.path.join("CMakeLists.txt"), "project(test)\n").unwrap();
        let s0 = project_signature(&tp.path);

        wait_mtime_tick();
        for skip in &["node_modules", "target", ".git", "build", "dist"] {
            let d = tp.path.join(skip);
            fs::create_dir(&d).unwrap();
            fs::write(d.join("foo.cpp"), "int x;\n").unwrap();
        }
        let s1 = project_signature(&tp.path);
        assert_eq!(
            s0, s1,
            "blocklisted dirs (node_modules / target / .git / build / dist) must be skipped"
        );
    }

    #[test]
    fn signature_changes_when_source_modified() {
        let tp = TempProject::new();
        let src = tp.path.join("main.cpp");
        fs::write(&src, "int x;\n").unwrap();
        let s0 = project_signature(&tp.path);

        wait_mtime_tick();
        fs::write(&src, "int x; int y;\n").unwrap();
        let s1 = project_signature(&tp.path);
        assert_ne!(s0, s1, "modifying a tracked source must change signature");
    }
}
