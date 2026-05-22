use crate::error::CitadelError;
use crate::types::*;
use std::collections::VecDeque;

// ahash imports for fast traversal sets/maps
use ahash::RandomState;
type FastSet<K> = std::collections::HashSet<K, RandomState>;
type FastMap<K, V> = std::collections::HashMap<K, V, RandomState>;

fn fast_set<K: std::cmp::Eq + std::hash::Hash>() -> FastSet<K> {
    FastSet::with_hasher(RandomState::new())
}

fn fast_map<K: std::cmp::Eq + std::hash::Hash, V>() -> FastMap<K, V> {
    FastMap::with_hasher(RandomState::new())
}

fn filter_kinds(kinds: &[EdgeKind]) -> Option<&[EdgeKind]> {
    if kinds.is_empty() { None } else { Some(kinds) }
}

pub use crate::storage::NodeMetrics;

/// Graph traversal operations. Blanket-implemented for any type that
/// provides `NodeStore + EdgeStore`, so all storage backends get
/// BFS/DFS/pathfinding for free.
pub trait GraphQuery: crate::storage::NodeStore + crate::storage::EdgeStore {
    fn traverse_bfs(
        &self,
        start_id: &str,
        options: &TraversalOptions,
    ) -> Result<Vec<(Node, Vec<Edge>)>, CitadelError>;

    fn traverse_dfs(
        &self,
        start_id: &str,
        options: &TraversalOptions,
    ) -> Result<Vec<(Node, Vec<Edge>)>, CitadelError>;

    fn find_shortest_path(
        &self,
        from_id: &str,
        to_id: &str,
        edge_kinds: Option<&[EdgeKind]>,
    ) -> Result<Option<Vec<(Node, Edge)>>, CitadelError>;

    fn get_callers(&self, node_id: &str, max_depth: u32) -> Result<Vec<(Node, Edge)>, CitadelError>;
    fn get_callees(&self, node_id: &str, max_depth: u32) -> Result<Vec<(Node, Edge)>, CitadelError>;
    fn get_impact_radius(&self, node_id: &str, max_depth: u32) -> Result<Vec<(Node, Vec<Edge>)>, CitadelError>;
    fn get_call_graph(&self, node_id: &str, depth: u32) -> Result<Vec<(Node, Vec<Edge>)>, CitadelError>;
    fn get_type_hierarchy(&self, node_id: &str) -> Result<Vec<(Node, Vec<Edge>)>, CitadelError>;
    fn find_usages(&self, node_id: &str) -> Result<Vec<(Node, Edge)>, CitadelError>;
    fn get_ancestors(&self, node_id: &str) -> Result<Vec<Node>, CitadelError>;
    fn get_children(&self, node_id: &str) -> Result<Vec<Node>, CitadelError>;
    fn get_node_metrics(&self, node_id: &str) -> Result<NodeMetrics, CitadelError>;
}

/// Blanket implementation: any `NodeStore + EdgeStore` gets basic traversal.
/// Backends can override individual methods for SQL-optimized performance.
impl<T: crate::storage::NodeStore + crate::storage::EdgeStore + Send + Sync + ?Sized> GraphQuery for T {
    fn traverse_bfs(
        &self,
        start_id: &str,
        options: &TraversalOptions,
    ) -> Result<Vec<(Node, Vec<Edge>)>, CitadelError> {
        let start_node = self.get_node_by_id(start_id)?
            .ok_or_else(|| CitadelError::NotFound(format!("start node not found: {start_id}")))?;

        let mut visited: FastSet<String> = fast_set();
        let mut queue: VecDeque<(Node, usize)> = VecDeque::new();
        let mut results: Vec<(Node, Vec<Edge>)> = Vec::new();
        let mut node_edges: FastMap<String, Vec<Edge>> = fast_map();

        visited.insert(start_id.to_string());
        queue.push_back((start_node.clone(), 0));

        while let Some((current_node, depth)) = queue.pop_front() {
            if depth > options.max_depth {
                break;
            }
            if results.len() >= options.limit {
                break;
            }

            let edges: Vec<Edge> = match options.direction {
                TraversalDirection::Outgoing => self.get_outgoing_edges(&current_node.id, filter_kinds(&options.edge_kinds), None)?,
                TraversalDirection::Incoming => self.get_incoming_edges(&current_node.id, filter_kinds(&options.edge_kinds))?,
                TraversalDirection::Both => {
                    let mut out = self.get_outgoing_edges(&current_node.id, filter_kinds(&options.edge_kinds), None)?;
                    let inc = self.get_incoming_edges(&current_node.id, filter_kinds(&options.edge_kinds))?;
                    out.extend(inc);
                    out
                }
            };

            node_edges.insert(current_node.id.clone(), edges.clone());

            for edge in &edges {
                let neighbor_id = if edge.source == current_node.id { &edge.target } else { &edge.source };
                if visited.contains(neighbor_id) {
                    continue;
                }
                if !options.node_kinds.is_empty()
                    && let Some(neighbor) = self.get_node_by_id(neighbor_id)?
                    && !options.node_kinds.contains(&neighbor.kind)
                {
                    continue;
                }
                visited.insert(neighbor_id.to_string());
                if let Some(neighbor) = self.get_node_by_id(neighbor_id)? {
                    queue.push_back((neighbor, depth + 1));
                }
            }

            let include = options.include_start || current_node.id != start_id;
            if include {
                results.push((current_node, edges));
            }
        }

        Ok(results)
    }

    fn traverse_dfs(
        &self,
        start_id: &str,
        options: &TraversalOptions,
    ) -> Result<Vec<(Node, Vec<Edge>)>, CitadelError> {
        let start_node = self.get_node_by_id(start_id)?
            .ok_or_else(|| CitadelError::NotFound(format!("start node not found: {start_id}")))?;

        let mut visited: FastSet<String> = fast_set();
        let mut results: Vec<(Node, Vec<Edge>)> = Vec::new();
        let mut stack: Vec<(Node, usize)> = Vec::new();

        visited.insert(start_id.to_string());
        stack.push((start_node.clone(), 0));

        while let Some((current_node, depth)) = stack.pop() {
            if depth > options.max_depth {
                continue;
            }
            if results.len() >= options.limit {
                break;
            }

            let edges: Vec<Edge> = match options.direction {
                TraversalDirection::Outgoing => self.get_outgoing_edges(&current_node.id, filter_kinds(&options.edge_kinds), None)?,
                TraversalDirection::Incoming => self.get_incoming_edges(&current_node.id, filter_kinds(&options.edge_kinds))?,
                TraversalDirection::Both => {
                    let mut out = self.get_outgoing_edges(&current_node.id, filter_kinds(&options.edge_kinds), None)?;
                    let inc = self.get_incoming_edges(&current_node.id, filter_kinds(&options.edge_kinds))?;
                    out.extend(inc);
                    out
                }
            };

            let include = options.include_start || current_node.id != start_id;
            if include {
                results.push((current_node.clone(), edges.clone()));
            }

            for edge in edges.iter().rev() {
                let neighbor_id = if edge.source == current_node.id { &edge.target } else { &edge.source };
                if visited.contains(neighbor_id) {
                    continue;
                }
                if !options.node_kinds.is_empty()
                    && let Some(neighbor) = self.get_node_by_id(neighbor_id)?
                    && !options.node_kinds.contains(&neighbor.kind)
                {
                    continue;
                }
                visited.insert(neighbor_id.to_string());
                if let Some(neighbor) = self.get_node_by_id(neighbor_id)? {
                    stack.push((neighbor, depth + 1));
                }
            }
        }

        Ok(results)
    }

    fn find_shortest_path(
        &self,
        from_id: &str,
        to_id: &str,
        edge_kinds: Option<&[EdgeKind]>,
    ) -> Result<Option<Vec<(Node, Edge)>>, CitadelError> {
        if from_id == to_id {
            let node = self.get_node_by_id(from_id)?
                .ok_or_else(|| CitadelError::NotFound(format!("node not found: {from_id}")))?;
            return Ok(Some(vec![(node.clone(), Edge {
                source: node.id.clone(),
                target: node.id.clone(),
                kind: EdgeKind::References,
                metadata: None,
                line: None,
                column: None,
                provenance: None,
            })]));
        }

        let from_node = self.get_node_by_id(from_id)?
            .ok_or_else(|| CitadelError::NotFound(format!("from node not found: {from_id}")))?;

        let mut visited: FastSet<String> = fast_set();
        let mut queue: VecDeque<String> = VecDeque::new();
        let mut parent: FastMap<String, (String, Edge)> = fast_map();

        visited.insert(from_id.to_string());
        queue.push_back(from_id.to_string());

        while let Some(current_id) = queue.pop_front() {
            let edges = self.get_outgoing_edges(&current_id, edge_kinds, None)?;
            for edge in edges {
                if visited.contains(&edge.target) {
                    continue;
                }
                parent.insert(edge.target.clone(), (current_id.clone(), edge.clone()));
                if edge.target == to_id {
                    let mut path: Vec<(Node, Edge)> = Vec::new();
                    let mut cur = to_id.to_string();
                    path.push((from_node.clone(), Edge {
                        source: from_id.to_string(),
                        target: from_id.to_string(),
                        kind: EdgeKind::References,
                        metadata: None,
                        line: None,
                        column: None,
                        provenance: None,
                    }));
                    let mut steps: Vec<(String, Edge)> = Vec::new();
                    while cur != from_id {
                        if let Some((prev, step_edge)) = parent.get(&cur) {
                            steps.push((cur.clone(), step_edge.clone()));
                            cur = prev.clone();
                        } else {
                            break;
                        }
                    }
                    steps.reverse();
                    for (node_id, step_edge) in steps {
                        if let Some(node) = self.get_node_by_id(&node_id)? {
                            path.push((node, step_edge));
                        }
                    }
                    return Ok(Some(path));
                }
                visited.insert(edge.target.clone());
                queue.push_back(edge.target);
            }
        }

        Ok(None)
    }

    fn get_callers(&self, node_id: &str, max_depth: u32) -> Result<Vec<(Node, Edge)>, CitadelError> {
        let opts = TraversalOptions {
            max_depth: max_depth as usize,
            edge_kinds: vec![EdgeKind::Calls, EdgeKind::References, EdgeKind::Imports],
            direction: TraversalDirection::Incoming,
            limit: crate::constants::DEFAULT_TRAVERSAL_LIMIT,
            include_start: true,
            ..Default::default()
        };
        let results = self.traverse_bfs(node_id, &opts)?;
        let mut seen: FastSet<String> = fast_set();
        let mut callers = Vec::new();
        for (_current_node, edges) in &results {
            for edge in edges {
                if edge.target == *node_id && edge.source != *node_id && !seen.contains(&edge.source) {
                    seen.insert(edge.source.clone());
                    if let Some(source_node) = self.get_node_by_id(&edge.source)? {
                        callers.push((source_node, edge.clone()));
                    }
                }
            }
        }
        Ok(callers)
    }

    fn get_callees(&self, node_id: &str, max_depth: u32) -> Result<Vec<(Node, Edge)>, CitadelError> {
        let opts = TraversalOptions {
            max_depth: max_depth as usize,
            edge_kinds: vec![EdgeKind::Calls, EdgeKind::References, EdgeKind::Imports],
            direction: TraversalDirection::Outgoing,
            limit: crate::constants::DEFAULT_TRAVERSAL_LIMIT,
            include_start: true,
            ..Default::default()
        };
        let results = self.traverse_bfs(node_id, &opts)?;
        let mut seen: FastSet<String> = fast_set();
        let mut callees = Vec::new();
        for (_node, edges) in &results {
            for edge in edges {
                if edge.source == node_id && edge.target != node_id && !seen.contains(&edge.target) {
                    seen.insert(edge.target.clone());
                    if let Some(target_node) = self.get_node_by_id(&edge.target)? {
                        callees.push((target_node, edge.clone()));
                    }
                }
            }
        }
        Ok(callees)
    }

    fn get_impact_radius(
        &self,
        node_id: &str,
        max_depth: u32,
    ) -> Result<Vec<(Node, Vec<Edge>)>, CitadelError> {
        let opts = TraversalOptions {
            max_depth: max_depth as usize,
            edge_kinds: vec![EdgeKind::Calls, EdgeKind::References, EdgeKind::Imports, EdgeKind::Contains, EdgeKind::Extends, EdgeKind::Implements],
            direction: TraversalDirection::Incoming,
            limit: crate::constants::DEFAULT_TRAVERSAL_LIMIT,
            include_start: true,
            ..Default::default()
        };
        self.traverse_bfs(node_id, &opts)
    }

    fn get_call_graph(&self, node_id: &str, depth: u32) -> Result<Vec<(Node, Vec<Edge>)>, CitadelError> {
        let mut results = Vec::new();
        let start_node = self.get_node_by_id(node_id)?
            .ok_or_else(|| CitadelError::NotFound(format!("node not found: {node_id}")))?;

        let mut incoming = self.traverse_bfs(node_id, &TraversalOptions {
            max_depth: depth as usize,
            edge_kinds: vec![EdgeKind::Calls, EdgeKind::References, EdgeKind::Imports],
            direction: TraversalDirection::Incoming,
            include_start: false,
            ..Default::default()
        })?;

        let mut outgoing = self.traverse_bfs(node_id, &TraversalOptions {
            max_depth: depth as usize,
            edge_kinds: vec![EdgeKind::Calls, EdgeKind::References, EdgeKind::Imports],
            direction: TraversalDirection::Outgoing,
            include_start: false,
            ..Default::default()
        })?;

        results.push((start_node, Vec::new()));
        results.append(&mut incoming);
        results.append(&mut outgoing);
        Ok(results)
    }

    fn get_type_hierarchy(&self, node_id: &str) -> Result<Vec<(Node, Vec<Edge>)>, CitadelError> {
        use crate::constants::TYPE_HIERARCHY_MAX_DEPTH;
        let types_kinds = vec![EdgeKind::Extends, EdgeKind::Implements];
        let mut ancestors = self.traverse_bfs(node_id, &TraversalOptions {
            max_depth: TYPE_HIERARCHY_MAX_DEPTH,
            edge_kinds: types_kinds.clone(),
            direction: TraversalDirection::Incoming,
            include_start: false,
            ..Default::default()
        })?;

        let mut descendants = self.traverse_bfs(node_id, &TraversalOptions {
            max_depth: TYPE_HIERARCHY_MAX_DEPTH,
            edge_kinds: types_kinds,
            direction: TraversalDirection::Outgoing,
            include_start: false,
            ..Default::default()
        })?;

        let start_node = self.get_node_by_id(node_id)?
            .ok_or_else(|| CitadelError::NotFound(format!("node not found: {node_id}")))?;
        let mut results = vec![(start_node, Vec::new())];
        results.append(&mut ancestors);
        results.append(&mut descendants);
        Ok(results)
    }

    fn find_usages(&self, node_id: &str) -> Result<Vec<(Node, Edge)>, CitadelError> {
        let edges = self.get_incoming_edges(node_id, None)?;
        let mut usages = Vec::new();
        for edge in edges {
            if let Some(source_node) = self.get_node_by_id(&edge.source)? {
                usages.push((source_node, edge));
            }
        }
        Ok(usages)
    }

    fn get_ancestors(&self, node_id: &str) -> Result<Vec<Node>, CitadelError> {
        use crate::constants::ANCESTOR_LOOP_GUARD;
        let mut ancestors = Vec::new();
        let mut current_id = node_id.to_string();
        let mut iterations = 0;
        loop {
            if iterations >= ANCESTOR_LOOP_GUARD {
                break;
            }
            iterations += 1;
            let parent_edges = self.get_incoming_edges(
                &current_id,
                Some(&[EdgeKind::Contains]),
            )?;
            if parent_edges.is_empty() {
                break;
            }
            let parent_id = &parent_edges[0].source;
            if let Some(parent) = self.get_node_by_id(parent_id)? {
                ancestors.push(parent);
                current_id = parent_id.clone();
            } else {
                break;
            }
        }
        Ok(ancestors)
    }

    fn get_children(&self, node_id: &str) -> Result<Vec<Node>, CitadelError> {
        let edges = self.get_outgoing_edges(node_id, Some(&[EdgeKind::Contains]), None)?;
        let mut children = Vec::new();
        for edge in edges {
            if let Some(child) = self.get_node_by_id(&edge.target)? {
                children.push(child);
            }
        }
        Ok(children)
    }

    fn get_node_metrics(&self, node_id: &str) -> Result<NodeMetrics, CitadelError> {
        let incoming = self.get_incoming_edges(node_id, None)?;
        let outgoing = self.get_outgoing_edges(node_id, None, None)?;
        let call_count = incoming.iter().filter(|e| e.kind == EdgeKind::Calls).count() as u32
            + outgoing.iter().filter(|e| e.kind == EdgeKind::Calls).count() as u32;
        let caller_count = incoming.iter().filter(|e| e.kind == EdgeKind::Calls).count() as u32;
        let child_count = outgoing.iter().filter(|e| e.kind == EdgeKind::Contains).count() as u32;
        let depth = self.get_ancestors(node_id)?.len() as u32;

        Ok(NodeMetrics {
            incoming_edge_count: incoming.len() as u32,
            outgoing_edge_count: outgoing.len() as u32,
            call_count,
            caller_count,
            child_count,
            depth,
        })
    }
}
