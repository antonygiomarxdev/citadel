pub mod error;
pub mod sqlite;
pub mod test_utils;

#[cfg(test)]
mod contract_tests;

use crate::storage::error::StorageError;
use crate::types::*;
use serde::{Deserialize, Serialize};

// ahash is used for HashSet/HashMap in BFS/DFS traversal.
// It is 2-3x faster than SipHash (the default std hasher) because
// it is not cryptographically secure — only DoS-resistant against
// HashDoS attacks via its randomized seed. Since our keys (node IDs,
// edge kinds) are not user-controlled input for hashing purposes,
// this is a safe tradeoff.
//
// Note: we cannot use Default::default() because ahash::RandomState
// does not impl Default when default-features are disabled.
// Instead we construct with RandomState::new() which generates
// fresh random keys per instance.
use ahash::RandomState;
type FastSet<K> = std::collections::HashSet<K, RandomState>;
type FastMap<K, V> = std::collections::HashMap<K, V, RandomState>;

fn fast_set<K: std::cmp::Eq + std::hash::Hash>() -> FastSet<K> {
    FastSet::with_hasher(RandomState::new())
}

fn fast_map<K: std::cmp::Eq + std::hash::Hash, V>() -> FastMap<K, V> {
    FastMap::with_hasher(RandomState::new())
}

pub trait Storage: Send + Sync {
    fn initialize(&mut self, db_path: &str) -> Result<(), StorageError>;
    fn open(&mut self, db_path: &str) -> Result<(), StorageError>;
    fn close(&mut self) -> Result<(), StorageError>;
    fn get_path(&self) -> Option<String>;

    // Nodes
    fn insert_node(&self, node: &Node) -> Result<(), StorageError>;
    fn insert_nodes(&self, nodes: &[Node]) -> Result<(), StorageError>;
    fn update_node(&self, node: &Node) -> Result<(), StorageError>;
    fn delete_node(&self, id: &str) -> Result<(), StorageError>;
    fn delete_nodes_by_file(&self, file_path: &str) -> Result<(), StorageError>;
    fn get_node_by_id(&self, id: &str) -> Result<Option<Node>, StorageError>;
    fn get_nodes_by_file(&self, file_path: &str) -> Result<Vec<Node>, StorageError>;
    fn get_nodes_by_kind(&self, kind: &NodeKind) -> Result<Vec<Node>, StorageError>;
    fn get_all_nodes(&self) -> Result<Vec<Node>, StorageError>;
    fn get_nodes_by_name(&self, name: &str) -> Result<Vec<Node>, StorageError>;
    fn get_nodes_by_qualified_name(&self, qn: &str) -> Result<Vec<Node>, StorageError>;
    fn get_nodes_by_lower_name(&self, name: &str) -> Result<Vec<Node>, StorageError>;

    // Search — default uses basic name matching; backends override for performance
    fn search_nodes(&self, query: &str, options: &SearchOptions) -> Result<Vec<SearchResult>, StorageError> {
        let query_lower = query.to_lowercase();
        let mut results: Vec<SearchResult> = Vec::new();

        let nodes = self.get_nodes_by_lower_name(&query_lower)?;
        for node in nodes {
            if let Some(ref kinds) = options.kinds
                && !kinds.contains(&node.kind)
            {
                continue;
            }
            if let Some(ref languages) = options.languages
                && !languages.contains(&node.language)
            {
                continue;
            }
            if let Some(ref include_patterns) = options.include_patterns
                && !include_patterns.iter().any(|p| picomatch_like(&node.file_path, p))
            {
                continue;
            }
            if let Some(ref exclude_patterns) = options.exclude_patterns
                && exclude_patterns.iter().any(|p| picomatch_like(&node.file_path, p))
            {
                continue;
            }
            let score = if node.name.to_lowercase() == query_lower {
                1.0
            } else {
                0.8
            };
            results.push(SearchResult { node: node.clone(), score, highlights: None });
        }

        let offset = options.offset.min(results.len());
        let end = (offset + options.limit).min(results.len());
        if offset > 0 || end < results.len() {
            results = results[offset..end].to_vec();
        }
        Ok(results)
    }

    // Edges
    fn insert_edge(&self, edge: &Edge) -> Result<(), StorageError>;
    fn insert_edges(&self, edges: &[Edge]) -> Result<(), StorageError>;
    fn delete_edges_by_source(&self, source_id: &str) -> Result<(), StorageError>;
    fn get_outgoing_edges(&self, source_id: &str, kinds: Option<&[EdgeKind]>, provenance: Option<&str>) -> Result<Vec<Edge>, StorageError>;
    fn get_incoming_edges(&self, target_id: &str, kinds: Option<&[EdgeKind]>) -> Result<Vec<Edge>, StorageError>;

    // find_edges_between_nodes — default iterates; backends override for batch SQL
    fn find_edges_between_nodes(&self, node_ids: &[String], kinds: Option<&[EdgeKind]>) -> Result<Vec<Edge>, StorageError> {
        let mut results = Vec::new();
        if node_ids.is_empty() {
            return Ok(results);
        }
        for source_id in node_ids {
            let outgoing = self.get_outgoing_edges(source_id, kinds, None)?;
            for edge in outgoing {
                if node_ids.iter().any(|id| id == &edge.target) {
                    results.push(edge);
                }
            }
        }
        Ok(results)
    }

    // Files
    fn upsert_file(&self, file: &FileRecord) -> Result<(), StorageError>;
    fn delete_file(&self, path: &str) -> Result<(), StorageError>;
    fn get_file_by_path(&self, path: &str) -> Result<Option<FileRecord>, StorageError>;
    fn get_all_files(&self) -> Result<Vec<FileRecord>, StorageError>;
    fn get_stale_files(&self, current_hashes: &std::collections::HashMap<String, String>) -> Result<Vec<FileRecord>, StorageError>;
    fn get_all_file_paths(&self) -> Result<Vec<String>, StorageError>;
    fn get_all_node_names(&self) -> Result<Vec<String>, StorageError>;

    // Unresolved refs
    fn insert_unresolved_ref(&self, r#ref: &UnresolvedRef) -> Result<(), StorageError>;
    fn insert_unresolved_refs_batch(&self, refs: &[UnresolvedRef]) -> Result<(), StorageError>;
    fn delete_unresolved_by_node(&self, node_id: &str) -> Result<(), StorageError>;
    fn get_unresolved_by_name(&self, name: &str) -> Result<Vec<UnresolvedRef>, StorageError>;
    fn get_all_unresolved_refs(&self) -> Result<Vec<UnresolvedRef>, StorageError>;
    fn get_unresolved_refs_count(&self) -> Result<u64, StorageError>;
    fn get_unresolved_refs_batch(&self, offset: u64, limit: u64) -> Result<Vec<UnresolvedRef>, StorageError>;

    // get_unresolved_refs_by_files — default filters all refs; backends override for performance
    fn get_unresolved_refs_by_files(&self, file_paths: &[String]) -> Result<Vec<UnresolvedRef>, StorageError> {
        let all = self.get_all_unresolved_refs()?;
        Ok(all.into_iter().filter(|r| file_paths.contains(&r.file_path)).collect())
    }

    fn clear_unresolved_refs(&self) -> Result<(), StorageError>;
    fn delete_resolved_refs(&self, from_node_ids: &[String]) -> Result<(), StorageError>;
    fn delete_specific_resolved_refs(&self, refs: &[UnresolvedRef]) -> Result<(), StorageError>;

    // Stats & Metadata
    fn get_stats(&self) -> Result<GraphStats, StorageError>;
    fn get_db_size_bytes(&self) -> Result<u64, StorageError> {
        Ok(0)
    }
    fn get_metadata(&self, key: &str) -> Result<Option<String>, StorageError>;
    fn set_metadata(&self, key: &str, value: &str) -> Result<(), StorageError>;
    fn get_all_metadata(&self) -> Result<std::collections::HashMap<String, String>, StorageError>;
    fn clear(&self) -> Result<(), StorageError>;

    // ---- Graph Traversal (default implementations) ----

    fn traverse_bfs(
        &self,
        start_id: &str,
        options: &TraversalOptions,
    ) -> Result<Vec<(Node, Vec<Edge>)>, StorageError> {
        use std::collections::VecDeque;
        let start_node = self.get_node_by_id(start_id)?
            .ok_or_else(|| StorageError::NotFound(format!("start node not found: {start_id}")))?;

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
    ) -> Result<Vec<(Node, Vec<Edge>)>, StorageError> {
        let start_node = self.get_node_by_id(start_id)?
            .ok_or_else(|| StorageError::NotFound(format!("start node not found: {start_id}")))?;

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
    ) -> Result<Option<Vec<(Node, Edge)>>, StorageError> {
        use std::collections::VecDeque;
        if from_id == to_id {
            let node = self.get_node_by_id(from_id)?
                .ok_or_else(|| StorageError::NotFound(format!("node not found: {from_id}")))?;
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
            .ok_or_else(|| StorageError::NotFound(format!("from node not found: {from_id}")))?;

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

    fn get_callers(&self, node_id: &str, max_depth: u32) -> Result<Vec<(Node, Edge)>, StorageError> {
        let opts = TraversalOptions {
            max_depth: max_depth as usize,
            edge_kinds: vec![EdgeKind::Calls, EdgeKind::References, EdgeKind::Imports],
            direction: TraversalDirection::Incoming,
            limit: 1000,
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

    fn get_callees(&self, node_id: &str, max_depth: u32) -> Result<Vec<(Node, Edge)>, StorageError> {
        let opts = TraversalOptions {
            max_depth: max_depth as usize,
            edge_kinds: vec![EdgeKind::Calls, EdgeKind::References, EdgeKind::Imports],
            direction: TraversalDirection::Outgoing,
            limit: 1000,
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
    ) -> Result<Vec<(Node, Vec<Edge>)>, StorageError> {
        let opts = TraversalOptions {
            max_depth: max_depth as usize,
            edge_kinds: vec![EdgeKind::Calls, EdgeKind::References, EdgeKind::Imports, EdgeKind::Contains, EdgeKind::Extends, EdgeKind::Implements],
            direction: TraversalDirection::Incoming,
            limit: 1000,
            include_start: true,
            ..Default::default()
        };
        self.traverse_bfs(node_id, &opts)
    }

    fn get_call_graph(&self, node_id: &str, depth: u32) -> Result<Vec<(Node, Vec<Edge>)>, StorageError> {
        let mut results = Vec::new();
        let start_node = self.get_node_by_id(node_id)?
            .ok_or_else(|| StorageError::NotFound(format!("node not found: {node_id}")))?;

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

    fn get_type_hierarchy(&self, node_id: &str) -> Result<Vec<(Node, Vec<Edge>)>, StorageError> {
        let types_kinds = vec![EdgeKind::Extends, EdgeKind::Implements];
        let mut ancestors = self.traverse_bfs(node_id, &TraversalOptions {
            max_depth: 20,
            edge_kinds: types_kinds.clone(),
            direction: TraversalDirection::Incoming,
            include_start: false,
            ..Default::default()
        })?;

        let mut descendants = self.traverse_bfs(node_id, &TraversalOptions {
            max_depth: 20,
            edge_kinds: types_kinds,
            direction: TraversalDirection::Outgoing,
            include_start: false,
            ..Default::default()
        })?;

        let start_node = self.get_node_by_id(node_id)?
            .ok_or_else(|| StorageError::NotFound(format!("node not found: {node_id}")))?;
        let mut results = vec![(start_node, Vec::new())];
        results.append(&mut ancestors);
        results.append(&mut descendants);
        Ok(results)
    }

    fn find_usages(&self, node_id: &str) -> Result<Vec<(Node, Edge)>, StorageError> {
        let edges = self.get_incoming_edges(node_id, None)?;
        let mut usages = Vec::new();
        for edge in edges {
            if let Some(source_node) = self.get_node_by_id(&edge.source)? {
                usages.push((source_node, edge));
            }
        }
        Ok(usages)
    }

    fn get_ancestors(&self, node_id: &str) -> Result<Vec<Node>, StorageError> {
        let mut ancestors = Vec::new();
        let mut current_id = node_id.to_string();
        loop {
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

    fn get_children(&self, node_id: &str) -> Result<Vec<Node>, StorageError> {
        let edges = self.get_outgoing_edges(node_id, Some(&[EdgeKind::Contains]), None)?;
        let mut children = Vec::new();
        for edge in edges {
            if let Some(child) = self.get_node_by_id(&edge.target)? {
                children.push(child);
            }
        }
        Ok(children)
    }

    fn get_node_metrics(&self, node_id: &str) -> Result<NodeMetrics, StorageError> {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetrics {
    pub incoming_edge_count: u32,
    pub outgoing_edge_count: u32,
    pub call_count: u32,
    pub caller_count: u32,
    pub child_count: u32,
    pub depth: u32,
}

fn filter_kinds(kinds: &[EdgeKind]) -> Option<&[EdgeKind]> {
    if kinds.is_empty() { None } else { Some(kinds) }
}

fn picomatch_like(file_path: &str, pattern: &str) -> bool {
    if pattern.contains('*') {
        let re = pattern
            .replace("**", "___DOUBLESTAR___")
            .replace('.', "\\.")
            .replace('*', "[^/]*")
            .replace("___DOUBLESTAR___", ".*");
        if let Ok(regex) = regex_lite::Regex::new(&format!("^{}$", re)) {
            return regex.is_match(file_path);
        }
    }
    file_path.contains(pattern)
}
