/// Compiler error output parser supporting GCC/Clang, MSVC, and Unity C# formats.

use super::types::ErrorInfo;
use regex::Regex;

pub struct ErrorParser {
    gcc_re: Regex,
    msvc_re: Regex,
    unity_re: Regex,
}

impl ErrorParser {
    pub fn new() -> Self {
        Self {
            gcc_re: Regex::new(r"^(.+?):(\d+):(\d+):\s*(error|warning):\s*(.+)$").unwrap(),
            msvc_re: Regex::new(r"^(.+?)\((\d+)\):\s*(error|warning)\s+(\w+):\s*(.+)$").unwrap(),
            unity_re: Regex::new(r"^(.+?)\((\d+),(\d+)\):\s*(error|warning)\s+(\w+):\s*(.+)$")
                .unwrap(),
        }
    }

    pub fn parse(&self, compiler_output: &str) -> Vec<ErrorInfo> {
        let mut errors = Vec::new();

        for line in compiler_output.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some(info) = self
                .try_gcc_clang(line)
                .or_else(|| self.try_unity_csharp(line))
                .or_else(|| self.try_msvc(line))
            {
                errors.push(info);
            }
        }

        errors
    }

    fn try_gcc_clang(&self, line: &str) -> Option<ErrorInfo> {
        let caps = self.gcc_re.captures(line)?;
        Some(ErrorInfo {
            file_path: caps[1].to_string(),
            line: caps[2].parse().ok()?,
            column: caps[3].parse().ok()?,
            error_code: String::new(),
            message: caps[5].to_string(),
        })
    }

    fn try_msvc(&self, line: &str) -> Option<ErrorInfo> {
        let caps = self.msvc_re.captures(line)?;
        Some(ErrorInfo {
            file_path: caps[1].to_string(),
            line: caps[2].parse().ok()?,
            column: 0,
            error_code: caps[4].to_string(),
            message: caps[5].to_string(),
        })
    }

    fn try_unity_csharp(&self, line: &str) -> Option<ErrorInfo> {
        let caps = self.unity_re.captures(line)?;
        Some(ErrorInfo {
            file_path: caps[1].to_string(),
            line: caps[2].parse().ok()?,
            column: caps[3].parse().ok()?,
            error_code: caps[5].to_string(),
            message: caps[6].to_string(),
        })
    }
}

impl Default for ErrorParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gcc_format() {
        let parser = ErrorParser::new();
        let errors =
            parser.parse("src/main.cpp:42:10: error: use of undeclared identifier 'foo'");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].file_path, "src/main.cpp");
        assert_eq!(errors[0].line, 42);
        assert_eq!(errors[0].column, 10);
        assert_eq!(errors[0].message, "use of undeclared identifier 'foo'");
    }

    #[test]
    fn test_msvc_format() {
        let parser = ErrorParser::new();
        let errors =
            parser.parse(r"src\main.cpp(42): error C2065: 'foo': undeclared identifier");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].file_path, r"src\main.cpp");
        assert_eq!(errors[0].line, 42);
        assert_eq!(errors[0].error_code, "C2065");
    }

    #[test]
    fn test_unity_format() {
        let parser = ErrorParser::new();
        let errors = parser.parse(
            "Assets/Scripts/Player.cs(42,10): error CS0246: The type or namespace 'Health' could not be found",
        );
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].file_path, "Assets/Scripts/Player.cs");
        assert_eq!(errors[0].line, 42);
        assert_eq!(errors[0].column, 10);
        assert_eq!(errors[0].error_code, "CS0246");
    }

    #[test]
    fn test_multiline() {
        let parser = ErrorParser::new();
        let output = r#"
src/a.cpp:10:5: error: undeclared 'x'
src/b.cpp:20:3: error: no matching function
src/a.cpp:15:1: warning: unused variable
"#;
        let errors = parser.parse(output);
        assert_eq!(errors.len(), 3);
        assert_eq!(errors[0].file_path, "src/a.cpp");
        assert_eq!(errors[1].file_path, "src/b.cpp");
        assert_eq!(errors[2].file_path, "src/a.cpp");
    }
}
