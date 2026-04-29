//! スタックトレース文字列を frame 配列に分解する。
//!
//! 対応する書式 (best-effort、複数を 1 入力で混ぜても OK):
//!   - GCC/Clang sanitizer: `    #0 0x... in func /path/file.cpp:LINE:COL`
//!   - GDB:                 `#0  func (args) at /path/file.cpp:LINE`
//!   - V8 / Node:           `    at func (/path/file.js:LINE:COL)`
//!   - Python:              `  File "/path/file.py", line LINE, in func`
//!   - Rust panic:          `   N: func\n             at /path/file.rs:LINE:COL`
//!
//! 各 frame は (path, line, function?) を抽出し、project_root に含まれるかで
//! `in_project` を立てる。project_root が None のときは全部 false。

use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct Frame {
    pub index: usize,
    pub function: Option<String>,
    pub path: String,
    pub line: u32,
    pub column: Option<u32>,
    pub in_project: bool,
}

#[tauri::command]
pub fn parse_stack_trace(text: String, project_root: Option<String>) -> Vec<Frame> {
    let root = project_root.as_deref().map(Path::new);
    let mut frames = Vec::new();

    for line in text.lines() {
        if let Some(f) = try_parse(line) {
            let in_project = match root {
                Some(r) => Path::new(&f.0).starts_with(r),
                None => false,
            };
            frames.push(Frame {
                index: frames.len(),
                function: f.3,
                path: f.0,
                line: f.1,
                column: f.2,
                in_project,
            });
        }
    }
    frames
}

/// (path, line, col?, function?) を返す。マッチしなければ None。
fn try_parse(line: &str) -> Option<(String, u32, Option<u32>, Option<String>)> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    // V8 / Node: `at func (/path:LINE:COL)`  または  `at /path:LINE:COL`
    if let Some(rest) = trimmed.strip_prefix("at ") {
        if let Some((func, paren)) = rest.split_once(" (") {
            if let Some(loc) = paren.strip_suffix(')') {
                if let Some((p, l, c)) = parse_path_line_col(loc) {
                    return Some((p, l, c, Some(func.to_string())));
                }
            }
        }
        if let Some((p, l, c)) = parse_path_line_col(rest) {
            return Some((p, l, c, None));
        }
    }

    // Python:  File "/path", line N, in func
    if let Some(rest) = trimmed.strip_prefix("File \"") {
        if let Some(close) = rest.find('"') {
            let p = &rest[..close];
            // ", line N, in func"
            let after = &rest[close..];
            if let Some(idx) = after.find("line ") {
                let after = &after[idx + 5..];
                let line_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
                let line_n: u32 = line_str.parse().ok()?;
                let func = after
                    .find("in ")
                    .map(|i| after[i + 3..].trim().to_string())
                    .filter(|s| !s.is_empty());
                return Some((p.to_string(), line_n, None, func));
            }
        }
    }

    // GCC/Clang sanitizer:  #N 0x... in func /path/file.cpp:LINE[:COL]
    if let Some(rest) = trimmed.strip_prefix('#') {
        // skip "<n> 0x... "
        let parts: Vec<&str> = rest.splitn(4, ' ').collect();
        if parts.len() >= 4 && parts[2] == "in" {
            let tail = parts[3]; // "func /path:LINE[:COL]"
            if let Some(sp) = tail.find(' ') {
                let func = tail[..sp].to_string();
                let loc = &tail[sp + 1..];
                if let Some((p, l, c)) = parse_path_line_col(loc) {
                    return Some((p, l, c, Some(func)));
                }
            }
        }
    }

    // GDB:  #N  func (args) at /path:LINE
    if let Some(rest) = trimmed.strip_prefix('#') {
        if let Some(idx) = rest.find(" at ") {
            let after_at = &rest[idx + 4..];
            if let Some((p, l, c)) = parse_path_line_col(after_at) {
                let func = rest[..idx]
                    .splitn(2, ' ')
                    .nth(1)
                    .map(|s| s.trim().split_whitespace().next().unwrap_or("").to_string())
                    .filter(|s| !s.is_empty());
                return Some((p, l, c, func));
            }
        }
    }

    // Rust panic style:  "             at /path:LINE:COL"
    if let Some(rest) = trimmed.strip_prefix("at ") {
        if let Some((p, l, c)) = parse_path_line_col(rest) {
            return Some((p, l, c, None));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_path_line_col_unix() {
        let r = parse_path_line_col("/home/user/main.cpp:42:7").unwrap();
        assert_eq!(r.0, "/home/user/main.cpp");
        assert_eq!(r.1, 42);
        assert_eq!(r.2, Some(7));
    }

    #[test]
    fn parse_path_line_col_unix_no_col() {
        let r = parse_path_line_col("/home/user/main.cpp:42").unwrap();
        assert_eq!(r.0, "/home/user/main.cpp");
        assert_eq!(r.1, 42);
        assert_eq!(r.2, None);
    }

    #[test]
    fn parse_path_line_col_windows_drive() {
        let r = parse_path_line_col("C:/proj/src/main.cpp:99").unwrap();
        assert_eq!(r.0, "C:/proj/src/main.cpp");
        assert_eq!(r.1, 99);
        assert_eq!(r.2, None);
    }

    #[test]
    fn parse_path_line_col_invalid() {
        assert!(parse_path_line_col("not a location").is_none());
        assert!(parse_path_line_col("/foo").is_none());
    }

    #[test]
    fn parses_v8_node_with_function() {
        let txt = "    at myFunc (/home/u/app.js:12:5)";
        let frames = parse_stack_trace(txt.to_string(), None);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].function.as_deref(), Some("myFunc"));
        assert_eq!(frames[0].path, "/home/u/app.js");
        assert_eq!(frames[0].line, 12);
        assert_eq!(frames[0].column, Some(5));
    }

    #[test]
    fn parses_v8_node_anonymous() {
        let txt = "    at /home/u/app.js:1:1";
        let frames = parse_stack_trace(txt.to_string(), None);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].function, None);
        assert_eq!(frames[0].line, 1);
    }

    #[test]
    fn parses_python_traceback() {
        let txt = r#"  File "/srv/app/main.py", line 33, in handler"#;
        let frames = parse_stack_trace(txt.to_string(), None);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].path, "/srv/app/main.py");
        assert_eq!(frames[0].line, 33);
        assert_eq!(frames[0].function.as_deref(), Some("handler"));
    }

    #[test]
    fn parses_gcc_sanitizer() {
        let txt = "    #0 0x7f00 in do_thing /work/src/foo.cpp:88:3";
        let frames = parse_stack_trace(txt.to_string(), None);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].function.as_deref(), Some("do_thing"));
        assert_eq!(frames[0].path, "/work/src/foo.cpp");
        assert_eq!(frames[0].line, 88);
        assert_eq!(frames[0].column, Some(3));
    }

    #[test]
    fn parses_gdb_with_args() {
        let txt = "#3  some_func (n=42, p=0x0) at /work/src/bar.c:120";
        let frames = parse_stack_trace(txt.to_string(), None);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].function.as_deref(), Some("some_func"));
        assert_eq!(frames[0].path, "/work/src/bar.c");
        assert_eq!(frames[0].line, 120);
    }

    #[test]
    fn parses_rust_panic_at_indent() {
        let txt = "             at /work/src/lib.rs:55:9";
        let frames = parse_stack_trace(txt.to_string(), None);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].path, "/work/src/lib.rs");
        assert_eq!(frames[0].line, 55);
        assert_eq!(frames[0].column, Some(9));
    }

    #[test]
    fn skips_unparseable_lines() {
        let txt = "this is some preamble\n\
                   nothing matches here\n\
                   ===========\n\
                   ";
        let frames = parse_stack_trace(txt.to_string(), None);
        assert_eq!(frames.len(), 0);
    }

    #[test]
    fn assigns_indices_in_order() {
        let txt = "    at a (/x/a.js:1:1)\n    at b (/x/b.js:2:2)\n    at c (/x/c.js:3:3)";
        let frames = parse_stack_trace(txt.to_string(), None);
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].index, 0);
        assert_eq!(frames[1].index, 1);
        assert_eq!(frames[2].index, 2);
    }

    #[test]
    fn in_project_flag_when_root_matches() {
        let txt = "    at f (/work/proj/src/main.cpp:10:5)\n    at g (/sys/lib/x.cpp:1:1)";
        let frames = parse_stack_trace(txt.to_string(), Some("/work/proj".to_string()));
        assert_eq!(frames.len(), 2);
        assert!(frames[0].in_project);
        assert!(!frames[1].in_project);
    }

    #[test]
    fn in_project_false_when_no_root() {
        let txt = "    at f (/work/proj/src/main.cpp:10:5)";
        let frames = parse_stack_trace(txt.to_string(), None);
        assert!(!frames[0].in_project);
    }

    #[test]
    fn handles_mixed_formats_in_one_input() {
        let txt = "    at js_fn (/a/app.js:1:1)\n\
                   #0 0x00 in c_fn /a/app.cpp:2:2\n\
                   File \"/a/app.py\", line 3, in py_fn";
        let frames = parse_stack_trace(txt.to_string(), None);
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].path, "/a/app.js");
        assert_eq!(frames[1].path, "/a/app.cpp");
        assert_eq!(frames[2].path, "/a/app.py");
    }
}

/// `path:line[:col]` を分解。Windows のドライブレター (`C:`) を想定して、
/// **末尾から** コロンを探す。
fn parse_path_line_col(s: &str) -> Option<(String, u32, Option<u32>)> {
    let s = s.trim();
    let mut parts: Vec<&str> = s.rsplitn(3, ':').collect();
    parts.reverse();
    match parts.len() {
        2 => {
            let line: u32 = parts[1].trim().parse().ok()?;
            Some((parts[0].to_string(), line, None))
        }
        3 => {
            // 3 分割の最初がドライブレター 1 文字なら path に戻す (Windows 対応)
            if parts[0].len() == 1
                && parts[0]
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_alphabetic())
                    .unwrap_or(false)
            {
                let line: u32 = parts[2].trim().parse().ok()?;
                let path = format!("{}:{}", parts[0], parts[1]);
                Some((path, line, None))
            } else {
                let line: u32 = parts[1].trim().parse().ok()?;
                let col: Option<u32> = parts[2].trim().parse().ok();
                Some((parts[0].to_string(), line, col))
            }
        }
        _ => None,
    }
}
