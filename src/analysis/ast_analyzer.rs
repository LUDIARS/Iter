/// AST analysis engine using libclang.
/// Analyzes code at error locations and discovers related symbols (calls, refs, includes).

#[allow(non_upper_case_globals)]
mod inner {
    pub use clang_sys::*;
}
use inner::*;

use crate::core::types::*;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::path::Path;
use std::ptr;

pub struct AstAnalyzer {
    index: CXIndex,
    build_dir: String,
    tu_cache: HashMap<String, CXTranslationUnit>,
    next_id: u32,
    loaded: bool,
}

/// Reference discovered during AST traversal.
struct SymbolRef {
    file: String,
    line: u32,
    column: u32,
    name: String,
    node_type: NodeType,
    edge_type: EdgeType,
}

/// Context passed to the visitor callback.
struct VisitorContext {
    refs: Vec<SymbolRef>,
    origin_file: String,
}

impl AstAnalyzer {
    pub fn new() -> Self {
        // Load libclang shared library at runtime
        let loaded = load().is_ok();
        let index = if loaded {
            unsafe { clang_createIndex(0, 0) }
        } else {
            log::warn!("Failed to load libclang — AST analysis disabled");
            ptr::null_mut()
        };

        Self {
            index,
            build_dir: ".".to_string(),
            tu_cache: HashMap::new(),
            next_id: 1000,
            loaded,
        }
    }

    pub fn set_compilation_database(&mut self, build_dir: &str) {
        self.build_dir = build_dir.to_string();
    }

    pub fn set_next_id_start(&mut self, start: u32) {
        self.next_id = start;
    }

    fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Analyze an error location and add related nodes/edges to the graph.
    pub fn analyze_error_location(
        &mut self,
        error: &ErrorInfo,
        error_node_id: u32,
        graph: &mut RelayGraph,
    ) {
        if !self.loaded || self.index.is_null() {
            return;
        }
        if !Path::new(&error.file_path).exists() {
            return;
        }

        let tu = match self.get_or_parse_tu(&error.file_path) {
            Some(tu) => tu,
            None => return,
        };

        // Get cursor at the error location
        let cursor = unsafe {
            let file = {
                let path = CString::new(error.file_path.as_str()).unwrap();
                clang_getFile(tu, path.as_ptr())
            };
            if file.is_null() {
                return;
            }

            let location = clang_getLocation(tu, file, error.line, error.column.max(1));
            clang_getCursor(tu, location)
        };

        if unsafe { clang_Cursor_isNull(cursor) } != 0 {
            return;
        }

        // Try to get the referenced/defined symbol
        let referenced = unsafe { clang_getCursorReferenced(cursor) };
        if unsafe { clang_Cursor_isNull(referenced) } == 0 {
            if let Some(sym_ref) = self.cursor_to_symbol_ref(referenced, EdgeType::Reference) {
                if sym_ref.file != error.file_path || sym_ref.line != error.line {
                    let ref_id = self.alloc_id();
                    let mut node = GraphNode::new(ref_id, &sym_ref.name, sym_ref.node_type);
                    node.file_path = sym_ref.file;
                    node.line = sym_ref.line;
                    node.column = sym_ref.column;
                    graph.nodes.push(node);
                    graph.edges.push(GraphEdge {
                        source_id: error_node_id,
                        target_id: ref_id,
                        edge_type: sym_ref.edge_type,
                        on_error_path: true,
                    });
                }
            }
        }

        // Collect child references via visitor
        let mut ctx = VisitorContext {
            refs: Vec::new(),
            origin_file: error.file_path.clone(),
        };

        // Get the parent function/method for broader context
        let semantic_parent = unsafe { clang_getCursorSemanticParent(cursor) };
        let visit_cursor =
            if unsafe { clang_Cursor_isNull(semantic_parent) } == 0
                && is_function_like(unsafe { clang_getCursorKind(semantic_parent) })
            {
                semantic_parent
            } else {
                cursor
            };

        unsafe {
            clang_visitChildren(
                visit_cursor,
                visit_children_callback,
                &mut ctx as *mut VisitorContext as CXClientData,
            );
        }

        // Deduplicate and add discovered references
        let mut seen: HashMap<(String, u32), u32> = HashMap::new();

        // Register existing nodes to avoid duplicates
        for node in &graph.nodes {
            seen.insert((node.file_path.clone(), node.line), node.id);
        }

        for sym_ref in &ctx.refs {
            let key = (sym_ref.file.clone(), sym_ref.line);
            if let Some(&existing_id) = seen.get(&key) {
                // Add edge to existing node if not already connected
                let already_connected = graph.edges.iter().any(|e| {
                    (e.source_id == error_node_id && e.target_id == existing_id)
                        || (e.source_id == existing_id && e.target_id == error_node_id)
                });
                if !already_connected {
                    graph.edges.push(GraphEdge {
                        source_id: error_node_id,
                        target_id: existing_id,
                        edge_type: sym_ref.edge_type,
                        on_error_path: false,
                    });
                }
            } else {
                let new_id = self.alloc_id();
                let mut node = GraphNode::new(new_id, &sym_ref.name, sym_ref.node_type);
                node.file_path = sym_ref.file.clone();
                node.line = sym_ref.line;
                node.column = sym_ref.column;
                graph.nodes.push(node);
                graph.edges.push(GraphEdge {
                    source_id: error_node_id,
                    target_id: new_id,
                    edge_type: sym_ref.edge_type,
                    on_error_path: false,
                });
                seen.insert(key, new_id);
            }
        }
    }

    /// Parse (or retrieve from cache) a translation unit.
    fn get_or_parse_tu(&mut self, file_path: &str) -> Option<CXTranslationUnit> {
        if let Some(&tu) = self.tu_cache.get(file_path) {
            return Some(tu);
        }

        let args = self.get_compile_args(file_path);
        let c_args: Vec<CString> = args.iter().map(|a| CString::new(a.as_str()).unwrap()).collect();
        let c_arg_ptrs: Vec<*const i8> = c_args.iter().map(|a| a.as_ptr()).collect();

        let c_file = CString::new(file_path).unwrap();

        let tu = unsafe {
            clang_parseTranslationUnit(
                self.index,
                c_file.as_ptr(),
                c_arg_ptrs.as_ptr(),
                c_arg_ptrs.len() as i32,
                ptr::null_mut(),
                0,
                CXTranslationUnit_DetailedPreprocessingRecord,
            )
        };

        if tu.is_null() {
            log::warn!("Failed to parse TU: {}", file_path);
            return None;
        }

        self.tu_cache.insert(file_path.to_string(), tu);
        Some(tu)
    }

    /// Get compilation arguments for a file, attempting compile_commands.json first.
    fn get_compile_args(&self, file_path: &str) -> Vec<String> {
        // Try to load from compile_commands.json
        let compile_commands_path = format!("{}/compile_commands.json", self.build_dir);
        if Path::new(&compile_commands_path).exists() {
            if let Some(args) = self.load_args_from_compdb(file_path) {
                return args;
            }
        }

        // Fallback: default C++ flags
        let ext = Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        match ext {
            "c" => vec!["-std=c11".to_string()],
            "cs" => vec![], // C# not handled by libclang
            _ => vec!["-std=c++17".to_string(), "-x".to_string(), "c++".to_string()],
        }
    }

    /// Parse compile_commands.json to find arguments for a specific file.
    fn load_args_from_compdb(&self, file_path: &str) -> Option<Vec<String>> {
        let db_path = CString::new(self.build_dir.as_str()).ok()?;
        let mut error = unsafe { clang_CompilationDatabase_fromDirectory(db_path.as_ptr(), ptr::null_mut()) };

        if error.is_null() {
            return None;
        }

        let c_file = CString::new(file_path).ok()?;
        let commands = unsafe {
            clang_CompilationDatabase_getCompileCommands(error, c_file.as_ptr())
        };

        let result = if !commands.is_null() {
            let n = unsafe { clang_CompileCommands_getSize(commands) };
            if n > 0 {
                let cmd = unsafe { clang_CompileCommands_getCommand(commands, 0) };
                let nargs = unsafe { clang_CompileCommand_getNumArgs(cmd) };
                let mut args = Vec::new();
                // Skip first arg (compiler) and last arg (source file)
                for i in 1..nargs.saturating_sub(1) {
                    let arg = unsafe { clang_CompileCommand_getArg(cmd, i) };
                    let arg_str = cx_string_to_string(arg);
                    // Skip output-related flags
                    if arg_str != "-o" && !arg_str.ends_with(".o") && arg_str != "-c" {
                        args.push(arg_str);
                    }
                }
                Some(args)
            } else {
                None
            }
        } else {
            None
        };

        unsafe {
            if !commands.is_null() {
                clang_CompileCommands_dispose(commands);
            }
            clang_CompilationDatabase_dispose(error);
        }

        result
    }

    /// Convert a cursor to a SymbolRef.
    fn cursor_to_symbol_ref(&self, cursor: CXCursor, edge_type: EdgeType) -> Option<SymbolRef> {
        let location = unsafe { clang_getCursorLocation(cursor) };

        let mut file: CXFile = ptr::null_mut();
        let mut line: u32 = 0;
        let mut column: u32 = 0;

        unsafe {
            clang_getFileLocation(location, &mut file, &mut line, &mut column, ptr::null_mut());
        }

        if file.is_null() || line == 0 {
            return None;
        }

        let file_name = cx_string_to_string(unsafe { clang_getFileName(file) });
        let cursor_name = cx_string_to_string(unsafe { clang_getCursorSpelling(cursor) });
        let kind = unsafe { clang_getCursorKind(cursor) };

        let node_type = cursor_kind_to_node_type(kind);

        Some(SymbolRef {
            file: file_name,
            line,
            column,
            name: cursor_name,
            node_type,
            edge_type,
        })
    }
}

impl Default for AstAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AstAnalyzer {
    fn drop(&mut self) {
        if !self.loaded {
            return;
        }
        for (_, tu) in self.tu_cache.drain() {
            unsafe {
                clang_disposeTranslationUnit(tu);
            }
        }
        if !self.index.is_null() {
            unsafe {
                clang_disposeIndex(self.index);
            }
        }
    }
}

// ===== Helper functions =====

/// Convert a CXString to an owned String.
fn cx_string_to_string(cx_str: CXString) -> String {
    unsafe {
        let c_str = clang_getCString(cx_str);
        let result = if c_str.is_null() {
            String::new()
        } else {
            CStr::from_ptr(c_str).to_string_lossy().into_owned()
        };
        clang_disposeString(cx_str);
        result
    }
}

/// Map clang cursor kind to our NodeType.
fn cursor_kind_to_node_type(kind: CXCursorKind) -> NodeType {
    match kind {
        CXCursor_FunctionDecl
        | CXCursor_CXXMethod
        | CXCursor_Constructor
        | CXCursor_Destructor
        | CXCursor_FunctionTemplate => NodeType::Function,

        CXCursor_ClassDecl
        | CXCursor_StructDecl
        | CXCursor_EnumDecl
        | CXCursor_TypedefDecl
        | CXCursor_TypeAliasDecl
        | CXCursor_ClassTemplate => NodeType::Type,

        CXCursor_VarDecl | CXCursor_FieldDecl | CXCursor_ParmDecl => NodeType::Variable,

        CXCursor_InclusionDirective => NodeType::Include,

        _ => NodeType::Variable,
    }
}

/// Check if a cursor kind represents a function-like entity.
fn is_function_like(kind: CXCursorKind) -> bool {
    matches!(
        kind,
        CXCursor_FunctionDecl
            | CXCursor_CXXMethod
            | CXCursor_Constructor
            | CXCursor_Destructor
            | CXCursor_FunctionTemplate
    )
}

/// Map cursor kind of a reference/expression to the appropriate EdgeType.
fn cursor_kind_to_edge_type(kind: CXCursorKind) -> EdgeType {
    match kind {
        CXCursor_CallExpr => EdgeType::Call,
        CXCursor_CXXBaseSpecifier => EdgeType::Inherit,
        CXCursor_InclusionDirective => EdgeType::Include,
        CXCursor_TypeRef | CXCursor_TemplateRef => EdgeType::Reference,
        CXCursor_DeclRefExpr | CXCursor_MemberRefExpr => EdgeType::Reference,
        _ => EdgeType::Reference,
    }
}

/// Visitor callback for clang_visitChildren.
extern "C" fn visit_children_callback(
    cursor: CXCursor,
    _parent: CXCursor,
    client_data: CXClientData,
) -> CXChildVisitResult {
    let ctx = unsafe { &mut *(client_data as *mut VisitorContext) };
    let kind = unsafe { clang_getCursorKind(cursor) };

    let should_collect = matches!(
        kind,
        CXCursor_CallExpr
            | CXCursor_DeclRefExpr
            | CXCursor_MemberRefExpr
            | CXCursor_TypeRef
            | CXCursor_TemplateRef
            | CXCursor_CXXBaseSpecifier
            | CXCursor_InclusionDirective
    );

    if should_collect {
        // Get the referenced declaration
        let referenced = unsafe { clang_getCursorReferenced(cursor) };
        let target = if unsafe { clang_Cursor_isNull(referenced) } == 0 {
            referenced
        } else {
            cursor
        };

        let location = unsafe { clang_getCursorLocation(target) };

        let mut file: CXFile = ptr::null_mut();
        let mut line: u32 = 0;
        let mut column: u32 = 0;

        unsafe {
            clang_getFileLocation(location, &mut file, &mut line, &mut column, ptr::null_mut());
        }

        if !file.is_null() && line > 0 {
            let file_name = cx_string_to_string(unsafe { clang_getFileName(file) });
            let name = cx_string_to_string(unsafe { clang_getCursorSpelling(target) });

            // Skip system headers and self-references
            if !file_name.starts_with("/usr/") && !name.is_empty() {
                let ref_kind = unsafe { clang_getCursorKind(target) };
                ctx.refs.push(SymbolRef {
                    file: file_name,
                    line,
                    column,
                    name,
                    node_type: cursor_kind_to_node_type(ref_kind),
                    edge_type: cursor_kind_to_edge_type(kind),
                });
            }
        }
    }

    CXChildVisit_Recurse
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_analyzer_creation() {
        let analyzer = AstAnalyzer::new();
        assert!(!analyzer.index.is_null());
    }

    #[test]
    fn test_analyze_simple_file() {
        let mut analyzer = AstAnalyzer::new();
        let mut graph = RelayGraph::default();

        // Create a temporary C++ file
        let dir = std::env::temp_dir().join("relay_test_ast");
        std::fs::create_dir_all(&dir).ok();
        let file_path = dir.join("test.cpp");
        let mut f = std::fs::File::create(&file_path).unwrap();
        writeln!(
            f,
            r#"
void helper() {{}}

void broken_func() {{
    helper();
    int x = 42;
}}
"#
        )
        .unwrap();

        let error = ErrorInfo {
            file_path: file_path.to_string_lossy().to_string(),
            line: 4,
            column: 1,
            error_code: String::new(),
            message: "test error".to_string(),
        };

        // Add the error node first
        let error_node = GraphNode::new(0, "test error", NodeType::ErrorSource);
        graph.nodes.push(error_node);

        analyzer.analyze_error_location(&error, 0, &mut graph);

        // Should have found at least the helper() call reference
        assert!(
            graph.nodes.len() > 1,
            "Expected AST analysis to find references, got {} nodes",
            graph.nodes.len()
        );

        // Clean up
        std::fs::remove_dir_all(&dir).ok();
    }
}
