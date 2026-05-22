pub mod search;

use crate::fs::FileSystem;
use crate::graph::GraphQuery;
use crate::storage::Storage;
use crate::types::*;
use serde::{Serialize, Deserialize};
use std::collections::{HashMap, HashSet};

/// Built context ready for consumption by an AI agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuiltContext {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub source_blocks: Vec<SourceBlock>,
    pub entry_points: Vec<String>,
}

/// A block of source code from a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceBlock {
    pub file_path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub source: String,
}

/// Options for find_relevant_context.
#[derive(Debug, Clone)]
pub struct FindContextOptions {
    pub max_nodes: usize,
    pub max_files: usize,
    pub include_tests: bool,
    pub include_non_production: bool,
}

impl Default for FindContextOptions {
    fn default() -> Self {
        FindContextOptions {
            max_nodes: 50,
            max_files: 20,
            include_tests: false,
            include_non_production: true,
        }
    }
}

/// The ContextBuilder: hybrid search + graph expansion + code blocks.
pub struct ContextBuilder {
    storage: Box<dyn Storage>,
    fs: Box<dyn FileSystem>,
}

impl ContextBuilder {
    pub fn new(storage: Box<dyn Storage>, fs: Box<dyn FileSystem>) -> Self {
        ContextBuilder { storage, fs }
    }

    /// Main entry point: given a natural language query, find relevant context.
    pub fn find_relevant_context(
        &self,
        query: &str,
        options: &FindContextOptions,
    ) -> Result<BuiltContext, String> {
        // Phase 1: Extract symbols from query
        let symbols = search::extract_symbols_from_query(query);

        // Phase 2: Hybrid search — try each search strategy
        let mut results: Vec<(Node, f64)> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();

        // 2a. Exact name match
        for symbol in &symbols {
            if let Ok(nodes) = self.storage.get_nodes_by_name(symbol) {
                for node in nodes {
                    if seen.insert(node.id.clone()) {
                        results.push((node, 1.0));
                    }
                }
            }
        }

        // 2b. Prefix match (titleCase -> class name prefix)
        for symbol in &symbols {
            if symbol.chars().next().is_some_and(|c| c.is_uppercase()) {
                let prefix = symbol.to_lowercase();
                if let Ok(all) = self.storage.get_all_nodes() {
                    for node in all {
                        if seen.contains(&node.id) { continue; }
                        let name_lower = node.name.to_lowercase();
                        if name_lower.starts_with(&prefix) {
                            seen.insert(node.id.clone());
                            results.push((node, 0.7));
                        }
                    }
                }
            }
        }

        // 2c. FTS5 search with remaining query terms
        let remaining_terms: Vec<&str> = query.split_whitespace()
            .filter(|w| !symbols.iter().any(|s| w.eq_ignore_ascii_case(s)))
            .collect();
        if !remaining_terms.is_empty() {
            let search_query = remaining_terms.join(" ");
            let opts = SearchOptions {
                query: Some(search_query.clone()),
                limit: options.max_nodes,
                ..Default::default()
            };
            if let Ok(search_results) = self.storage.search_nodes(&search_query, &opts) {
                for result in search_results {
                    if seen.insert(result.node.id.clone()) {
                        results.push((result.node, result.score));
                    }
                }
            }
        }

        // Sort by score
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(options.max_nodes);

        // Phase 3: Graph expansion from entry points
        let mut entry_point_ids: Vec<String> = Vec::new();
        for (node, _score) in &results[..results.len().min(10)] {
            entry_point_ids.push(node.id.clone());
        }

        let mut expanded_nodes: Vec<Node> = Vec::new();
        let mut expanded_edges: Vec<Edge> = Vec::new();
        let mut expanded_seen: HashSet<String> = HashSet::new();

        for (node, _score) in &results {
            if expanded_seen.insert(node.id.clone()) {
                expanded_nodes.push(node.clone());
            }
        }

        for entry_id in &entry_point_ids {
            // Get callers and callees (depth 1)
            if let Ok(callers) = self.storage.get_callers(entry_id, 1) {
                for (caller, edge) in callers {
                    if expanded_seen.insert(caller.id.clone()) {
                        expanded_nodes.push(caller);
                        expanded_edges.push(edge);
                    }
                }
            }
            if let Ok(callees) = self.storage.get_callees(entry_id, 1) {
                for (callee, edge) in callees {
                    if expanded_seen.insert(callee.id.clone()) {
                        expanded_nodes.push(callee);
                        expanded_edges.push(edge);
                    }
                }
            }
            // Get children (containment)
            if let Ok(children) = self.storage.get_children(entry_id) {
                for child in children {
                    if expanded_seen.insert(child.id.clone()) {
                        expanded_nodes.push(child);
                    }
                }
            }
        }

        // Phase 4: Edge recovery — find edges between selected nodes
        let selected_ids: Vec<String> = expanded_nodes.iter().map(|n| n.id.clone()).collect();
        if let Ok(between_edges) = self.storage.find_edges_between_nodes(&selected_ids, None) {
            for edge in between_edges {
                if !expanded_edges.iter().any(|e| e.source == edge.source && e.target == edge.target && e.kind == edge.kind) {
                    expanded_edges.push(edge);
                }
            }
        }

        // Phase 5: Per-file diversity cap
        let mut file_counts: HashMap<String, usize> = HashMap::new();
        expanded_nodes.retain(|n| {
            let count = file_counts.entry(n.file_path.clone()).or_insert(0);
            *count += 1;
            *count <= options.max_files
        });

        // Phase 6: Code block extraction
        let source_blocks = self.extract_source_blocks(&expanded_nodes, options);

        Ok(BuiltContext {
            nodes: expanded_nodes,
            edges: expanded_edges,
            source_blocks,
            entry_points: entry_point_ids,
        })
    }

    /// Extract source code blocks for each node.
    fn extract_source_blocks(
        &self,
        nodes: &[Node],
        _options: &FindContextOptions,
    ) -> Vec<SourceBlock> {
        let mut blocks = Vec::new();
        // Group nodes by file to avoid re-reading
        let mut files_seen: HashSet<String> = HashSet::new();

        for node in nodes {
            if !files_seen.insert(node.file_path.clone()) {
                continue;
            }

            // Read the file and slice the relevant lines
            if let Ok(source) = self.fs.read_to_string(std::path::Path::new(&node.file_path)) {
                let lines: Vec<&str> = source.lines().collect();
                let start = node.start_line.saturating_sub(3).max(1) as usize;
                let end = (node.end_line as usize + 3).min(lines.len());
                if start <= end {
                    let snippet = lines[start - 1..end].join("\n");
                    blocks.push(SourceBlock {
                        file_path: node.file_path.clone(),
                        start_line: start as u32,
                        end_line: end as u32,
                        source: snippet,
                    });
                }
            }
        }
        blocks
    }

    /// Format context as markdown.
    pub fn format_markdown(&self, ctx: &BuiltContext) -> String {
        let mut out = String::new();
        out.push_str("## Relevant Symbols\n\n");
        for node in &ctx.nodes {
            out.push_str(&format!(
                "- **{}** (`{}`) in `{}` line {}\n",
                node.name, node.kind.as_str(), node.file_path, node.start_line
            ));
        }

        if !ctx.edges.is_empty() {
            out.push_str("\n## Relationships\n\n");
            for edge in &ctx.edges {
                let source_name = ctx.nodes.iter().find(|n| n.id == edge.source).map(|n| n.name.as_str()).unwrap_or("?");
                let target_name = ctx.nodes.iter().find(|n| n.id == edge.target).map(|n| n.name.as_str()).unwrap_or("?");
                out.push_str(&format!("- {} → {} ({})\n", source_name, target_name, edge.kind.as_str()));
            }
        }

        if !ctx.source_blocks.is_empty() {
            out.push_str("\n## Source Code\n\n");
            for block in &ctx.source_blocks {
                out.push_str(&format!("### {} (lines {}-{})\n\n```\n{}\n```\n\n",
                    block.file_path, block.start_line, block.end_line, block.source));
            }
        }

        out
    }

    /// Format context as JSON.
    pub fn format_json(&self, ctx: &BuiltContext) -> Result<String, String> {
        serde_json::to_string_pretty(ctx).map_err(|e| e.to_string())
    }
}
