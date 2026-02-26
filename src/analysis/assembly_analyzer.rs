/// Assembly analysis engine (Phase 7 - stub).
/// Parses llvm-objdump output to map source lines to assembly.

use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct AssemblyLine {
    pub address: u64,
    pub instruction: String,
    pub source_file: String,
    pub source_line: u32,
}

pub struct AssemblyAnalyzer {
    line_map: HashMap<u64, (String, u32)>,
    asm_lines: Vec<AssemblyLine>,
}

impl AssemblyAnalyzer {
    pub fn new() -> Self {
        Self {
            line_map: HashMap::new(),
            asm_lines: Vec::new(),
        }
    }

    /// Analyze an object file using llvm-objdump.
    pub fn analyze_object(&mut self, obj_path: &str) -> bool {
        let output = match Command::new("llvm-objdump")
            .args(["-d", "-l", "--no-show-raw-insn", obj_path])
            .output()
        {
            Ok(o) => o,
            Err(_) => return false,
        };

        if !output.status.success() {
            return false;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        self.parse_objdump_output(&stdout)
    }

    /// Get assembly lines corresponding to a source line.
    pub fn get_assembly_for_line(
        &self,
        source_file: &str,
        line: u32,
    ) -> Vec<&AssemblyLine> {
        self.asm_lines
            .iter()
            .filter(|al| al.source_line == line && al.source_file.ends_with(source_file))
            .collect()
    }

    fn parse_objdump_output(&mut self, output: &str) -> bool {
        self.asm_lines.clear();
        self.line_map.clear();

        let mut current_file = String::new();
        let mut current_line: u32 = 0;

        for text_line in output.lines() {
            let trimmed = text_line.trim();

            // Source file reference: ; /path/to/file.cpp:42
            if let Some(rest) = trimmed.strip_prefix("; ") {
                if let Some((file, line_str)) = rest.rsplit_once(':') {
                    if let Ok(line_num) = line_str.parse::<u32>() {
                        current_file = file.to_string();
                        current_line = line_num;
                    }
                }
                continue;
            }

            // Assembly line: address: instruction
            if let Some((addr_str, instr)) = trimmed.split_once(':') {
                let addr_str = addr_str.trim();
                if let Ok(addr) = u64::from_str_radix(addr_str.trim_start_matches("0x"), 16) {
                    let asm_line = AssemblyLine {
                        address: addr,
                        instruction: instr.trim().to_string(),
                        source_file: current_file.clone(),
                        source_line: current_line,
                    };
                    self.line_map
                        .insert(addr, (current_file.clone(), current_line));
                    self.asm_lines.push(asm_line);
                }
            }
        }

        !self.asm_lines.is_empty()
    }
}

impl Default for AssemblyAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
