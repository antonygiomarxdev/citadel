//! Resolution module — types and orchestration API.
//!
//! Resolution is currently performed in TypeScript (`src/resolution/`).
//! This module provides shared types that are used by the Rust side
//! for edge construction and node ID generation. The Rust resolution
//! logic (framework resolvers, import resolver, name matcher) was
//! removed in v0.3 to eliminate dead code and is planned for Phase 6
//! (native resolution bridge).

use crate::types::*;

/// A resolved reference with metadata about how it was resolved.
#[derive(Debug, Clone)]
pub struct ResolvedRef {
    pub from_node_id: String,
    pub reference_name: String,
    pub reference_kind: String,
    pub target_node_id: String,
    pub confidence: f64,
    pub resolved_by: String,
}

/// Resolution outcome: either resolved to a target, or confirmed unresolvable.
#[derive(Debug, Clone)]
pub enum ResolutionOutcome {
    Resolved(ResolvedRef),
    Unresolvable,
    Skipped,
}

/// Context passed to resolvers — abstracts away the storage backend.
/// This is currently implemented only in TypeScript.
pub trait ResolutionContext {
    fn get_nodes_in_file(&self, file_path: &str) -> Result<Vec<Node>, String>;
    fn get_nodes_by_name(&self, name: &str) -> Result<Vec<Node>, String>;
    fn get_nodes_by_qualified_name(&self, qn: &str) -> Result<Vec<Node>, String>;
    fn get_nodes_by_kind(&self, kind: &NodeKind) -> Result<Vec<Node>, String>;
    fn get_nodes_by_lower_name(&self, name: &str) -> Result<Vec<Node>, String>;
    fn file_exists(&self, file_path: &str) -> bool;
    fn read_file(&self, file_path: &str) -> Result<Option<String>, String>;
    fn get_project_root(&self) -> &str;
    fn get_all_files(&self) -> Result<Vec<String>, String>;
    fn get_all_node_names(&self) -> Result<Vec<String>, String>;
}
