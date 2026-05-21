pub mod error;
pub mod sqlite;

use crate::storage::error::StorageError;
use crate::types::*;

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

    // Search
    fn search_nodes(&self, query: &str, options: &SearchOptions) -> Result<Vec<SearchResult>, StorageError>;

    // Edges
    fn insert_edge(&self, edge: &Edge) -> Result<(), StorageError>;
    fn insert_edges(&self, edges: &[Edge]) -> Result<(), StorageError>;
    fn delete_edges_by_source(&self, source_id: &str) -> Result<(), StorageError>;
    fn get_outgoing_edges(&self, source_id: &str, kinds: Option<&[EdgeKind]>, provenance: Option<&str>) -> Result<Vec<Edge>, StorageError>;
    fn get_incoming_edges(&self, target_id: &str, kinds: Option<&[EdgeKind]>) -> Result<Vec<Edge>, StorageError>;
    fn find_edges_between_nodes(&self, node_ids: &[String], kinds: Option<&[EdgeKind]>) -> Result<Vec<Edge>, StorageError>;

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
    fn get_unresolved_refs_by_files(&self, file_paths: &[String]) -> Result<Vec<UnresolvedRef>, StorageError>;
    fn clear_unresolved_refs(&self) -> Result<(), StorageError>;
    fn delete_resolved_refs(&self, from_node_ids: &[String]) -> Result<(), StorageError>;
    fn delete_specific_resolved_refs(&self, refs: &[UnresolvedRef]) -> Result<(), StorageError>;

    // Stats & Metadata
    fn get_stats(&self) -> Result<GraphStats, StorageError>;
    fn get_metadata(&self, key: &str) -> Result<Option<String>, StorageError>;
    fn set_metadata(&self, key: &str, value: &str) -> Result<(), StorageError>;
    fn get_all_metadata(&self) -> Result<std::collections::HashMap<String, String>, StorageError>;
    fn clear(&self) -> Result<(), StorageError>;
}
