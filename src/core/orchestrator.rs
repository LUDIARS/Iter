/// Central orchestrator that coordinates error parsing, AST analysis, and graph construction.

use super::error_parser::ErrorParser;
use super::types::*;
use crate::analysis::ast_analyzer::AstAnalyzer;

pub struct Orchestrator {
    parser: ErrorParser,
    ast: AstAnalyzer,
    next_id: u32,
    build_dir: String,
}

impl Orchestrator {
    pub fn new() -> Self {
        Self {
            parser: ErrorParser::new(),
            ast: AstAnalyzer::new(),
            next_id: 0,
            build_dir: ".".to_string(),
        }
    }

    pub fn set_build_dir(&mut self, dir: &str) {
        self.build_dir = dir.to_string();
        self.ast.set_compilation_database(dir);
    }

    /// Build a relay graph from compiler error output.
    pub fn build_graph_from_error(&mut self, error_string: &str) -> RelayGraph {
        let errors = self.parser.parse(error_string);
        let mut graph = RelayGraph::default();

        // Phase 1: Create error nodes
        let mut error_nodes: Vec<(u32, ErrorInfo)> = Vec::new();
        for err in &errors {
            let id = self.next_id;
            self.next_id += 1;

            let mut node = GraphNode::new(id, &err.message, NodeType::ErrorSource);
            node.file_path = err.file_path.clone();
            node.line = err.line;
            node.column = err.column;
            node.is_error = true;

            graph.nodes.push(node);
            error_nodes.push((id, err.clone()));
        }

        // Phase 6: AST analysis — discover related symbols for each error
        self.ast.set_next_id_start(self.next_id + 1000);
        for (error_id, err) in &error_nodes {
            self.ast.analyze_error_location(err, *error_id, &mut graph);
        }

        graph
    }
}

impl Default for Orchestrator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_graph_from_single_error() {
        let mut orch = Orchestrator::new();
        let graph = orch.build_graph_from_error(
            "src/main.cpp:42:10: error: use of undeclared identifier 'foo'",
        );
        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.nodes[0].file_path, "src/main.cpp");
        assert!(graph.nodes[0].is_error);
    }

    #[test]
    fn test_build_graph_from_multiple_errors() {
        let mut orch = Orchestrator::new();
        let graph = orch.build_graph_from_error(
            "src/a.cpp:10:5: error: undeclared 'x'\nsrc/b.cpp:20:3: error: no matching function",
        );
        assert_eq!(graph.nodes.len(), 2);
    }
}
