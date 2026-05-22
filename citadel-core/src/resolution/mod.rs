pub mod import_resolver;
pub mod name_matcher;
pub mod path_aliases;
pub mod frameworks;

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

/// A framework resolver: detect if the framework is present, resolve a ref.
pub trait FrameworkResolver: Send + Sync {
    fn name(&self) -> &str;
    fn detect(&self, ctx: &dyn ResolutionContext) -> bool;
    fn resolve(&self, r#ref: &UnresolvedRef, ctx: &dyn ResolutionContext) -> Option<ResolvedRef>;
}

/// Orchestrator: takes a batch of UnresolvedRef and returns ResolvedRef.
pub struct ReferenceResolver {
    frameworks: Vec<Box<dyn FrameworkResolver>>,
}

impl ReferenceResolver {
    pub fn new(frameworks: Vec<Box<dyn FrameworkResolver>>) -> Self {
        ReferenceResolver { frameworks }
    }

    pub fn resolve_one(&self, r#ref: &UnresolvedRef, ctx: &dyn ResolutionContext) -> Option<ResolvedRef> {
        // Try framework resolvers first
        for fw in &self.frameworks {
            if let Some(resolved) = fw.resolve(r#ref, ctx)
                && resolved.confidence >= 0.9 {
                    return Some(resolved);
                }
        }
        // Fallback to import resolution
        if let Some(resolved) = import_resolver::resolve_import(r#ref, ctx) {
            return Some(resolved);
        }
        // Fallback to name matching
        name_matcher::match_name(r#ref, ctx)
    }

    pub fn resolve_batch(&self, refs: &[UnresolvedRef], ctx: &dyn ResolutionContext) -> Vec<ResolvedRef> {
        let mut resolved = Vec::new();
        for r in refs {
            if let Some(r) = self.resolve_one(r, ctx) {
                resolved.push(r);
            }
        }
        resolved
    }
}
