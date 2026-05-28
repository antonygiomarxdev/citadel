use crate::error::CitadelError;
use crate::extraction::ExtractionResult;
use crate::types::*;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

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

/// Batch transaction control. Backends may override to batch multiple
/// CRUD operations in a single SQLite transaction for performance.
pub trait BatchOps: Send + Sync {
    fn begin_batch(&self) -> Result<(), CitadelError> { Ok(()) }
    fn commit_batch(&self) -> Result<(), CitadelError> { Ok(()) }
    fn rollback_batch(&self) -> Result<(), CitadelError> { Ok(()) }
}

/// The complete storage interface. Combine all focused traits.
///
/// Backends must implement every focused trait individually; this
/// supertrait exists as a convenience alias for consumers that need
/// full storage access (e.g., the Database NAPI bridge).
pub trait FullStore: Lifecycle + NodeStore + EdgeStore + FileStore
    + UnresolvedRefStore + MetadataStore + StatsProvider + BatchOps + Send + Sync
{
    /// Store flat extraction results (pre-flattened, no per-file hash check).
    ///
    /// Takes already-reconciled flat slices of all nodes, edges, refs, and file records.
    /// The caller is responsible for filtering out unchanged files via hash comparison.
    /// Default implementation delegates to individual CRUD trait methods.
    /// Storage backends SHOULD override with a single-transaction multi-VALUES version.
    fn bulk_store_extractions(
        &self,
        nodes: &[Node],
        edges: &[Edge],
        unresolved_refs: &[UnresolvedRef],
        files: &[FileRecord],
    ) -> Result<(u32, u32), CitadelError> {
        if !nodes.is_empty() {
            self.insert_nodes(nodes)?;
        }
        if !edges.is_empty() {
            self.insert_edges(edges)?;
        }
        if !unresolved_refs.is_empty() {
            self.insert_unresolved_refs_batch(unresolved_refs)?;
        }
        for file in files {
            self.upsert_file(file)?;
        }
        Ok((nodes.len() as u32, edges.len() as u32))
    }

    /// Store multiple files' extraction results in one batch.
    ///
    /// Returns `(indexed, errored, skipped, errors)`. The default implementation
    /// falls back to calling `store_file_extraction` per file. Storage backends
    /// SHOULD override this with a single-transaction version for performance.
    fn batch_store_file_extractions(
        &self,
        paths_and_hashes: &[(&str, &str)],
        languages: &[&str],
        results: &[ExtractionResult],
        root_dir: &str,
    ) -> Result<(u32, u32, u32, Vec<String>), CitadelError> {
        let mut indexed = 0u32;
        let mut errored = 0u32;
        let mut skipped = 0u32;
        let mut errors = Vec::new();
        for (i, &(path, content_hash)) in paths_and_hashes.iter().enumerate() {
            let language = languages[i];
            let result = &results[i];
            match self.store_file_extraction(
                path, content_hash, language,
                &result.nodes, &result.edges, &result.unresolved_references,
                &result.errors, root_dir,
            ) {
                Ok(true) => indexed += 1,
                Ok(false) => skipped += 1,
                Err(e) => {
                    errored += 1;
                    errors.push(format!("{}: {}", path, e));
                }
            }
        }
        Ok((indexed, errored, skipped, errors))
    }

    /// Store all extracted data for one file in a single atomic operation.
    ///
    /// Returns `true` if the file was indexed (new or changed), `false` if skipped
    /// (same content hash already present). The default implementation delegates
    /// to individual CRUD trait methods. Storage backends SHOULD override this
    /// with a single-transaction version for performance.
    #[allow(clippy::too_many_arguments)]
    fn store_file_extraction(
        &self,
        path: &str,
        content_hash: &str,
        language: &str,
        nodes: &[Node],
        edges: &[Edge],
        unresolved_refs: &[UnresolvedRef],
        extraction_errors: &[ExtractionError],
        root_dir: &str,
    ) -> Result<bool, CitadelError> {
        // Skip if content hash matches cached data
        if let Ok(Some(existing)) = self.get_file_by_path(path)
            && existing.content_hash == content_hash
        {
            return Ok(false);
        }

        // Delete old data for this file
        self.delete_nodes_by_file(path)?;
        self.delete_file(path)?;

        // Filter nodes with valid IDs
        let valid_nodes: Vec<Node> = nodes.iter()
            .filter(|n| !n.id.is_empty() && !n.name.is_empty() && !n.file_path.is_empty())
            .cloned()
            .collect();

        if !valid_nodes.is_empty() {
            self.insert_nodes(&valid_nodes)?;

            // Filter edges where both endpoints are among the new nodes
            let valid_ids: HashSet<&str> = valid_nodes.iter().map(|n| n.id.as_str()).collect();
            let valid_edges: Vec<Edge> = edges.iter()
                .filter(|e| valid_ids.contains(e.source.as_str()) && valid_ids.contains(e.target.as_str()))
                .cloned()
                .collect();

            if !valid_edges.is_empty() {
                self.insert_edges(&valid_edges)?;
            }

            let valid_refs: Vec<UnresolvedRef> = unresolved_refs.iter()
                .filter(|r| valid_ids.contains(r.from_node_id.as_str()))
                .cloned()
                .collect();

            if !valid_refs.is_empty() {
                self.insert_unresolved_refs_batch(&valid_refs)?;
            }
        }

        // File metadata
        let lang = Language::from_str(language).unwrap_or(Language::Unknown);
        let full_path = if root_dir.is_empty() {
            path.to_string()
        } else {
            format!("{}/{}", root_dir.trim_end_matches('/'), path)
        };
        let (size, modified_at) = match std::fs::metadata(&full_path) {
            Ok(meta) => {
                let mtime = meta.modified().ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                (meta.len(), mtime)
            }
            Err(_) => (0, 0),
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        let file = FileRecord {
            path: path.to_string(),
            content_hash: content_hash.to_string(),
            language: lang,
            size,
            modified_at,
            indexed_at: now,
            node_count: valid_nodes.len() as u32,
            errors: if extraction_errors.is_empty() { None } else { Some(extraction_errors.to_vec()) },
        };

        self.upsert_file(&file)?;
        Ok(true)
    }
}
