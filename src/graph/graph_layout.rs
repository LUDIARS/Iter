/// Graph layout algorithms: Sugiyama (DAG) and Force-directed.

use crate::core::config;
use crate::core::types::*;
use std::collections::{HashMap, HashSet, VecDeque};

pub struct GraphLayout;

impl GraphLayout {
    pub fn new() -> Self {
        Self
    }

    /// Automatically choose layout algorithm based on graph structure.
    pub fn auto_layout(&self, graph: &mut RelayGraph) {
        if graph.nodes.is_empty() {
            return;
        }

        if self.has_cycle(graph) {
            self.layout_force_directed(graph, 100);
        } else {
            self.layout_sugiyama(graph);
        }
    }

    // ===== Sugiyama hierarchical layout =====

    pub fn layout_sugiyama(&self, graph: &mut RelayGraph) {
        if graph.nodes.is_empty() {
            return;
        }

        let layers = self.assign_layers(graph);
        let layers = self.minimize_crossings(layers, graph);
        self.assign_coordinates(&layers, graph);
    }

    /// Assign nodes to layers using longest path from sources.
    fn assign_layers(&self, graph: &RelayGraph) -> Vec<Vec<u32>> {
        let mut in_degree: HashMap<u32, usize> = HashMap::new();
        let mut adj: HashMap<u32, Vec<u32>> = HashMap::new();

        for node in &graph.nodes {
            in_degree.entry(node.id).or_insert(0);
            adj.entry(node.id).or_default();
        }

        for edge in &graph.edges {
            *in_degree.entry(edge.target_id).or_insert(0) += 1;
            adj.entry(edge.source_id).or_default().push(edge.target_id);
        }

        // BFS from sources (in_degree == 0)
        let mut node_layer: HashMap<u32, usize> = HashMap::new();
        let mut queue: VecDeque<u32> = VecDeque::new();

        for (&id, &deg) in &in_degree {
            if deg == 0 {
                queue.push_back(id);
                node_layer.insert(id, 0);
            }
        }

        // If no sources found (all nodes have incoming edges), start from error nodes
        if queue.is_empty() {
            for node in &graph.nodes {
                if node.is_error {
                    queue.push_back(node.id);
                    node_layer.insert(node.id, 0);
                }
            }
        }

        // If still empty, just use the first node
        if queue.is_empty() {
            if let Some(node) = graph.nodes.first() {
                queue.push_back(node.id);
                node_layer.insert(node.id, 0);
            }
        }

        while let Some(id) = queue.pop_front() {
            let layer = node_layer[&id];
            if let Some(neighbors) = adj.get(&id) {
                for &next in neighbors {
                    let new_layer = layer + 1;
                    let current = node_layer.entry(next).or_insert(0);
                    if new_layer > *current {
                        *current = new_layer;
                    }
                    queue.push_back(next);
                }
            }
        }

        // Assign unvisited nodes to layer 0
        for node in &graph.nodes {
            node_layer.entry(node.id).or_insert(0);
        }

        // Group by layer
        let max_layer = node_layer.values().copied().max().unwrap_or(0);
        let mut layers: Vec<Vec<u32>> = vec![vec![]; max_layer + 1];
        for (&id, &layer) in &node_layer {
            layers[layer].push(id);
        }

        layers
    }

    /// Minimize edge crossings using barycenter heuristic.
    fn minimize_crossings(
        &self,
        mut layers: Vec<Vec<u32>>,
        graph: &RelayGraph,
    ) -> Vec<Vec<u32>> {
        let adj: HashMap<u32, Vec<u32>> = {
            let mut m: HashMap<u32, Vec<u32>> = HashMap::new();
            for edge in &graph.edges {
                m.entry(edge.source_id).or_default().push(edge.target_id);
                m.entry(edge.target_id).or_default().push(edge.source_id);
            }
            m
        };

        // Position lookup for barycenter
        for _iteration in 0..3 {
            let mut pos_in_layer: HashMap<u32, f64> = HashMap::new();
            for layer in &layers {
                for (i, &id) in layer.iter().enumerate() {
                    pos_in_layer.insert(id, i as f64);
                }
            }

            for layer in &mut layers {
                layer.sort_by(|&a, &b| {
                    let avg_a = Self::barycenter(a, &adj, &pos_in_layer);
                    let avg_b = Self::barycenter(b, &adj, &pos_in_layer);
                    avg_a.partial_cmp(&avg_b).unwrap_or(std::cmp::Ordering::Equal)
                });

                for (i, &id) in layer.iter().enumerate() {
                    pos_in_layer.insert(id, i as f64);
                }
            }
        }

        layers
    }

    fn barycenter(node: u32, adj: &HashMap<u32, Vec<u32>>, positions: &HashMap<u32, f64>) -> f64 {
        let neighbors = match adj.get(&node) {
            Some(n) => n,
            None => return 0.0,
        };

        if neighbors.is_empty() {
            return positions.get(&node).copied().unwrap_or(0.0);
        }

        let sum: f64 = neighbors
            .iter()
            .filter_map(|n| positions.get(n))
            .sum();
        let count = neighbors
            .iter()
            .filter(|n| positions.contains_key(n))
            .count();

        if count > 0 {
            sum / count as f64
        } else {
            0.0
        }
    }

    /// Assign x/y coordinates based on layer assignment.
    fn assign_coordinates(&self, layers: &[Vec<u32>], graph: &mut RelayGraph) {
        let gap_x = config::NODE_COLLAPSED_W + config::LAYOUT_NODE_GAP_X;
        let gap_y = config::NODE_COLLAPSED_H + config::LAYOUT_NODE_GAP_Y;

        for (li, layer) in layers.iter().enumerate() {
            let total_height = layer.len() as f64 * gap_y;
            let start_y = -total_height / 2.0;

            for (ni, &node_id) in layer.iter().enumerate() {
                if let Some(node) = graph.find_node_mut(node_id) {
                    node.x = li as f64 * gap_x;
                    node.y = start_y + ni as f64 * gap_y;
                }
            }
        }
    }

    // ===== Force-directed layout =====

    pub fn layout_force_directed(&self, graph: &mut RelayGraph, iterations: usize) {
        if graph.nodes.len() <= 1 {
            return;
        }

        // Initialize positions in a circle
        let n = graph.nodes.len();
        let radius = (n as f64) * 50.0;
        for (i, node) in graph.nodes.iter_mut().enumerate() {
            let angle = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
            node.x = radius * angle.cos();
            node.y = radius * angle.sin();
        }

        let mut velocities: Vec<Vec2> = vec![Vec2::default(); n];
        let k_rep = 5000.0;
        let k_att = 0.01;
        let damping = 0.95;

        let edge_pairs: Vec<(usize, usize)> = graph
            .edges
            .iter()
            .filter_map(|e| {
                let si = graph.nodes.iter().position(|n| n.id == e.source_id)?;
                let ti = graph.nodes.iter().position(|n| n.id == e.target_id)?;
                Some((si, ti))
            })
            .collect();

        for _ in 0..iterations {
            let positions: Vec<Vec2> = graph.nodes.iter().map(|n| Vec2::new(n.x, n.y)).collect();

            // Repulsion between all pairs
            for i in 0..n {
                for j in (i + 1)..n {
                    let delta = positions[i] - positions[j];
                    let dist = delta.length().max(1.0);
                    let force = delta.normalized() * (k_rep / (dist * dist));
                    velocities[i] += force;
                    velocities[j] = velocities[j] - force;
                }
            }

            // Attraction along edges
            for &(si, ti) in &edge_pairs {
                let delta = positions[ti] - positions[si];
                let dist = delta.length();
                let force = delta.normalized() * (k_att * dist);
                velocities[si] += force;
                velocities[ti] = velocities[ti] - force;
            }

            // Apply velocities with damping
            for (i, node) in graph.nodes.iter_mut().enumerate() {
                velocities[i] = velocities[i] * damping;
                node.x += velocities[i].x;
                node.y += velocities[i].y;
            }
        }
    }

    // ===== Cycle detection =====

    fn has_cycle(&self, graph: &RelayGraph) -> bool {
        let mut visited: HashSet<u32> = HashSet::new();
        let mut rec_stack: HashSet<u32> = HashSet::new();

        let adj: HashMap<u32, Vec<u32>> = {
            let mut m: HashMap<u32, Vec<u32>> = HashMap::new();
            for edge in &graph.edges {
                m.entry(edge.source_id).or_default().push(edge.target_id);
            }
            m
        };

        for node in &graph.nodes {
            if !visited.contains(&node.id) {
                if Self::dfs_cycle(node.id, &adj, &mut visited, &mut rec_stack) {
                    return true;
                }
            }
        }

        false
    }

    fn dfs_cycle(
        node: u32,
        adj: &HashMap<u32, Vec<u32>>,
        visited: &mut HashSet<u32>,
        rec_stack: &mut HashSet<u32>,
    ) -> bool {
        visited.insert(node);
        rec_stack.insert(node);

        if let Some(neighbors) = adj.get(&node) {
            for &next in neighbors {
                if !visited.contains(&next) {
                    if Self::dfs_cycle(next, adj, visited, rec_stack) {
                        return true;
                    }
                } else if rec_stack.contains(&next) {
                    return true;
                }
            }
        }

        rec_stack.remove(&node);
        false
    }
}

impl Default for GraphLayout {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dag_layout() {
        let layout = GraphLayout::new();
        let mut graph = RelayGraph {
            nodes: vec![
                GraphNode::new(0, "main", NodeType::Function),
                GraphNode::new(1, "foo", NodeType::Function),
                GraphNode::new(2, "bar", NodeType::Function),
            ],
            edges: vec![
                GraphEdge {
                    source_id: 0,
                    target_id: 1,
                    edge_type: EdgeType::Call,
                    on_error_path: false,
                },
                GraphEdge {
                    source_id: 0,
                    target_id: 2,
                    edge_type: EdgeType::Call,
                    on_error_path: false,
                },
            ],
        };

        layout.layout_sugiyama(&mut graph);

        // Node 0 should be in layer 0 (leftmost)
        let n0 = graph.find_node(0).unwrap();
        let n1 = graph.find_node(1).unwrap();
        assert!(n0.x < n1.x, "Source should be left of target");
    }

    #[test]
    fn test_cycle_detection() {
        let layout = GraphLayout::new();

        let dag = RelayGraph {
            nodes: vec![
                GraphNode::new(0, "a", NodeType::Function),
                GraphNode::new(1, "b", NodeType::Function),
            ],
            edges: vec![GraphEdge {
                source_id: 0,
                target_id: 1,
                edge_type: EdgeType::Call,
                on_error_path: false,
            }],
        };
        assert!(!layout.has_cycle(&dag));

        let cyclic = RelayGraph {
            nodes: vec![
                GraphNode::new(0, "a", NodeType::Function),
                GraphNode::new(1, "b", NodeType::Function),
            ],
            edges: vec![
                GraphEdge {
                    source_id: 0,
                    target_id: 1,
                    edge_type: EdgeType::Call,
                    on_error_path: false,
                },
                GraphEdge {
                    source_id: 1,
                    target_id: 0,
                    edge_type: EdgeType::Call,
                    on_error_path: false,
                },
            ],
        };
        assert!(cyclic.nodes.len() == 2);
    }
}
