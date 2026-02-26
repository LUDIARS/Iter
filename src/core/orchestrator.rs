/// Central orchestrator that coordinates error parsing, AST analysis, and graph construction.

use super::error_parser::ErrorParser;
use super::types::*;

pub struct Orchestrator {
    parser: ErrorParser,
    next_id: u32,
    build_dir: String,
}

impl Orchestrator {
    pub fn new() -> Self {
        Self {
            parser: ErrorParser::new(),
            next_id: 0,
            build_dir: ".".to_string(),
        }
    }

    pub fn set_build_dir(&mut self, dir: &str) {
        self.build_dir = dir.to_string();
    }

    /// Build a relay graph from compiler error output.
    pub fn build_graph_from_error(&mut self, error_string: &str) -> RelayGraph {
        let errors = self.parser.parse(error_string);
        let mut graph = RelayGraph::default();

        for err in &errors {
            let id = self.next_id;
            self.next_id += 1;

            let mut node = GraphNode::new(id, &err.message, NodeType::ErrorSource);
            node.file_path = err.file_path.clone();
            node.line = err.line;
            node.column = err.column;
            node.is_error = true;

            graph.nodes.push(node);

            // Phase 6: AST analysis will add related nodes here
        }

        graph
    }

    fn _next_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

impl Default for Orchestrator {
    fn default() -> Self {
        Self::new()
    }
}
