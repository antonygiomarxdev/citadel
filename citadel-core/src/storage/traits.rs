use crate::error::CitadelError;
use crate::types::*;
use std::collections::HashMap;

/// Lifecycle management for storage backends.
pub trait Lifecycle: Send + Sync {
    fn initialize(&mut self, db_path: &str) -> Result<(), CitadelError>;
    fn open(&mut self, db_path: &str) -> Result<(), CitadelError>;
    fn close(&mut self) -> Result<(), CitadelError>;
    fn get_path(&self) -> Option<String>;
    fn clear(&self) -> Result<(), CitadelError>;
}

/// CRUD operations for code graph nodes.
pub trait NodeStore: Send + Sync {
    fn insert_node(&self, node: &Node) -> Result<(), CitadelError>;
    fn insert_nodes(&self, nodes: &[Node]) -> Result<(), CitadelError>;
    fn update_node(&self, node: &Node) -> Result<(), CitadelError>;
    fn delete_node(&self, id: &str) -> Result<(), CitadelError>;
    fn delete_nodes_by_file(&self, file_path: &str) -> Result<(), CitadelError>;
    fn get_node_by_id(&self, id: &str) -> Result<Option<Node>, CitadelError>;
    fn get_nodes_by_file(&self, file_path: &str) -> Result<Vec<Node>, CitadelError>;
    fn get_nodes_by_kind(&self, kind: &NodeKind) -> Result<Vec<Node>, CitadelError>;
    fn get_all_nodes(&self) -> Result<Vec<Node>, CitadelError>;
    fn get_nodes_by_name(&self, name: &str) -> Result<Vec<Node>, CitadelError>;
    fn get_nodes_by_qualified_name(&self, qn: &str) -> Result<Vec<Node>, CitadelError>;
    fn get_nodes_by_lower_name(&self, name: &str) -> Result<Vec<Node>, CitadelError>;

    fn search_nodes(&self, query: &str, options: &SearchOptions) -> Result<Vec<SearchResult>, CitadelError>;
}

/// CRUD operations for edges between nodes.
pub trait EdgeStore: Send + Sync {
    fn insert_edge(&self, edge: &Edge) -> Result<(), CitadelError>;
    fn insert_edges(&self, edges: &[Edge]) -> Result<(), CitadelError>;
    fn delete_edges_by_source(&self, source_id: &str) -> Result<(), CitadelError>;
    fn get_outgoing_edges(&self, source_id: &str, kinds: Option<&[EdgeKind]>, provenance: Option<&str>) -> Result<Vec<Edge>, CitadelError>;
    fn get_incoming_edges(&self, target_id: &str, kinds: Option<&[EdgeKind]>) -> Result<Vec<Edge>, CitadelError>;
    fn find_edges_between_nodes(&self, node_ids: &[String], kinds: Option<&[EdgeKind]>) -> Result<Vec<Edge>, CitadelError>;
}

/// File record management for indexed source files.
pub trait FileStore: Send + Sync {
    fn upsert_file(&self, file: &FileRecord) -> Result<(), CitadelError>;
    fn delete_file(&self, path: &str) -> Result<(), CitadelError>;
    fn get_file_by_path(&self, path: &str) -> Result<Option<FileRecord>, CitadelError>;
    fn get_all_files(&self) -> Result<Vec<FileRecord>, CitadelError>;
    fn get_stale_files(&self, current_hashes: &HashMap<String, String>) -> Result<Vec<FileRecord>, CitadelError>;
    fn get_all_file_paths(&self) -> Result<Vec<String>, CitadelError>;
    fn get_all_node_names(&self) -> Result<Vec<String>, CitadelError>;
}

/// Unresolved reference management — references detected during extraction
/// that need to be resolved later.
pub trait UnresolvedRefStore: Send + Sync {
    fn insert_unresolved_ref(&self, r#ref: &UnresolvedRef) -> Result<(), CitadelError>;
    fn insert_unresolved_refs_batch(&self, refs: &[UnresolvedRef]) -> Result<(), CitadelError>;
    fn delete_unresolved_by_node(&self, node_id: &str) -> Result<(), CitadelError>;
    fn get_unresolved_by_name(&self, name: &str) -> Result<Vec<UnresolvedRef>, CitadelError>;
    fn get_all_unresolved_refs(&self) -> Result<Vec<UnresolvedRef>, CitadelError>;
    fn get_unresolved_refs_count(&self) -> Result<u64, CitadelError>;
    fn get_unresolved_refs_batch(&self, offset: u64, limit: u64) -> Result<Vec<UnresolvedRef>, CitadelError>;
    fn get_unresolved_refs_by_files(&self, file_paths: &[String]) -> Result<Vec<UnresolvedRef>, CitadelError>;
    fn clear_unresolved_refs(&self) -> Result<(), CitadelError>;
    fn delete_resolved_refs(&self, from_node_ids: &[String]) -> Result<(), CitadelError>;
    fn delete_specific_resolved_refs(&self, refs: &[UnresolvedRef]) -> Result<(), CitadelError>;
}

/// Key-value metadata store for arbitrary key-value pairs.
pub trait MetadataStore: Send + Sync {
    fn get_metadata(&self, key: &str) -> Result<Option<String>, CitadelError>;
    fn set_metadata(&self, key: &str, value: &str) -> Result<(), CitadelError>;
    fn get_all_metadata(&self) -> Result<HashMap<String, String>, CitadelError>;
}

/// Statistics and size information about the graph database.
pub trait StatsProvider: Send + Sync {
    fn get_stats(&self) -> Result<GraphStats, CitadelError>;
    fn get_db_size_bytes(&self) -> Result<u64, CitadelError> {
        Ok(0)
    }
}

/// The complete storage interface. Combine all focused traits.
///
/// Backends must implement every focused trait individually; this
/// supertrait exists as a convenience alias for consumers that need
/// full storage access (e.g., the Database NAPI bridge).
pub trait FullStore: Lifecycle + NodeStore + EdgeStore + FileStore
    + UnresolvedRefStore + MetadataStore + StatsProvider + Send + Sync {}
