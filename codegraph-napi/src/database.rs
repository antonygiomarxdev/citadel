use std::str::FromStr;
use std::sync::Mutex;

use codegraph_core::storage::sqlite::SqliteStorage;
use codegraph_core::storage::Storage;
use codegraph_core::types::*;

#[napi]
pub struct Database {
    inner: Mutex<SqliteStorage>,
}

impl Default for Database {
    fn default() -> Self {
        Self::new()
    }
}

#[napi]
impl Database {
    #[napi(constructor)]
    pub fn new() -> Self {
        Database {
            inner: Mutex::new(SqliteStorage::new()),
        }
    }

    #[napi]
    pub fn initialize(&self, db_path: String) -> napi::Result<()> {
        let mut storage = self.inner.lock().map_err(napi_err)?;
        storage.initialize(&db_path).map_err(napi_err)
    }

    #[napi]
    pub fn open(&self, db_path: String) -> napi::Result<()> {
        let mut storage = self.inner.lock().map_err(napi_err)?;
        storage.open(&db_path).map_err(napi_err)
    }

    #[napi]
    pub fn close(&self) -> napi::Result<()> {
        let mut storage = self.inner.lock().map_err(napi_err)?;
        storage.close().map_err(napi_err)
    }

    #[napi]
    pub fn is_open(&self) -> bool {
        self.inner.lock().ok().and_then(|s| s.get_path()).is_some()
    }

    // ---- Node CRUD ----

    #[napi]
    pub fn insert_node(&self, node_json: String) -> napi::Result<()> {
        let node: Node = serde_json::from_str(&node_json).map_err(|e| napi::Error::from_reason(e.to_string()))?;
        let storage = self.inner.lock().map_err(napi_err)?;
        storage.insert_node(&node).map_err(napi_err)
    }

    #[napi]
    pub fn insert_nodes(&self, nodes_json: String) -> napi::Result<()> {
        let nodes: Vec<Node> = serde_json::from_str(&nodes_json).map_err(|e| napi::Error::from_reason(e.to_string()))?;
        let storage = self.inner.lock().map_err(napi_err)?;
        storage.insert_nodes(&nodes).map_err(napi_err)
    }

    #[napi]
    pub fn update_node(&self, node_json: String) -> napi::Result<()> {
        let node: Node = serde_json::from_str(&node_json).map_err(|e| napi::Error::from_reason(e.to_string()))?;
        let storage = self.inner.lock().map_err(napi_err)?;
        storage.update_node(&node).map_err(napi_err)
    }

    #[napi]
    pub fn delete_node(&self, id: String) -> napi::Result<()> {
        let storage = self.inner.lock().map_err(napi_err)?;
        storage.delete_node(&id).map_err(napi_err)
    }

    #[napi]
    pub fn delete_nodes_by_file(&self, file_path: String) -> napi::Result<()> {
        let storage = self.inner.lock().map_err(napi_err)?;
        storage.delete_nodes_by_file(&file_path).map_err(napi_err)
    }

    #[napi]
    pub fn get_node_by_id(&self, id: String) -> napi::Result<Option<String>> {
        let storage = self.inner.lock().map_err(napi_err)?;
        let result = storage.get_node_by_id(&id).map_err(napi_err)?;
        match result {
            Some(node) => serde_json::to_string(&node).map(Some).map_err(|e| napi::Error::from_reason(e.to_string())),
            None => Ok(None),
        }
    }

    #[napi]
    pub fn get_nodes_by_file(&self, file_path: String) -> napi::Result<String> {
        let storage = self.inner.lock().map_err(napi_err)?;
        let nodes = storage.get_nodes_by_file(&file_path).map_err(napi_err)?;
        serde_json::to_string(&nodes).map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn get_nodes_by_kind(&self, kind: String) -> napi::Result<String> {
        let parsed = parse_node_kind(&kind);
        let storage = self.inner.lock().map_err(napi_err)?;
        let nodes = storage.get_nodes_by_kind(&parsed).map_err(napi_err)?;
        serde_json::to_string(&nodes).map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn get_all_nodes(&self) -> napi::Result<String> {
        let storage = self.inner.lock().map_err(napi_err)?;
        let nodes = storage.get_all_nodes().map_err(napi_err)?;
        serde_json::to_string(&nodes).map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn get_nodes_by_name(&self, name: String) -> napi::Result<String> {
        let storage = self.inner.lock().map_err(napi_err)?;
        let nodes = storage.get_nodes_by_name(&name).map_err(napi_err)?;
        serde_json::to_string(&nodes).map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    // ---- Edge CRUD ----

    #[napi]
    pub fn insert_edge(&self, edge_json: String) -> napi::Result<()> {
        let edge: Edge = serde_json::from_str(&edge_json).map_err(|e| napi::Error::from_reason(e.to_string()))?;
        let storage = self.inner.lock().map_err(napi_err)?;
        storage.insert_edge(&edge).map_err(napi_err)
    }

    #[napi]
    pub fn insert_edges(&self, edges_json: String) -> napi::Result<()> {
        let edges: Vec<Edge> = serde_json::from_str(&edges_json).map_err(|e| napi::Error::from_reason(e.to_string()))?;
        let storage = self.inner.lock().map_err(napi_err)?;
        storage.insert_edges(&edges).map_err(napi_err)
    }

    #[napi]
    pub fn delete_edges_by_source(&self, source_id: String) -> napi::Result<()> {
        let storage = self.inner.lock().map_err(napi_err)?;
        storage.delete_edges_by_source(&source_id).map_err(napi_err)
    }

    #[napi]
    pub fn get_outgoing_edges(&self, source_id: String, kind: Option<String>, provenance: Option<String>) -> napi::Result<String> {
        let storage = self.inner.lock().map_err(napi_err)?;
        let kind_filter = kind.as_ref().map(|k| vec![parse_edge_kind(k)]);
        let edges = storage
            .get_outgoing_edges(&source_id, kind_filter.as_deref(), provenance.as_deref())
            .map_err(napi_err)?;
        serde_json::to_string(&edges).map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn get_incoming_edges(&self, target_id: String, kind: Option<String>) -> napi::Result<String> {
        let storage = self.inner.lock().map_err(napi_err)?;
        let kind_filter = kind.as_ref().map(|k| vec![parse_edge_kind(k)]);
        let edges = storage
            .get_incoming_edges(&target_id, kind_filter.as_deref())
            .map_err(napi_err)?;
        serde_json::to_string(&edges).map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    // ---- File CRUD ----

    #[napi]
    pub fn upsert_file(&self, file_json: String) -> napi::Result<()> {
        let file: FileRecord = serde_json::from_str(&file_json).map_err(|e| napi::Error::from_reason(e.to_string()))?;
        let storage = self.inner.lock().map_err(napi_err)?;
        storage.upsert_file(&file).map_err(napi_err)
    }

    #[napi]
    pub fn delete_file(&self, path: String) -> napi::Result<()> {
        let storage = self.inner.lock().map_err(napi_err)?;
        storage.delete_file(&path).map_err(napi_err)
    }

    #[napi]
    pub fn get_file_by_path(&self, path: String) -> napi::Result<Option<String>> {
        let storage = self.inner.lock().map_err(napi_err)?;
        let result = storage.get_file_by_path(&path).map_err(napi_err)?;
        match result {
            Some(file) => serde_json::to_string(&file).map(Some).map_err(|e| napi::Error::from_reason(e.to_string())),
            None => Ok(None),
        }
    }

    #[napi]
    pub fn get_all_files(&self) -> napi::Result<String> {
        let storage = self.inner.lock().map_err(napi_err)?;
        let files = storage.get_all_files().map_err(napi_err)?;
        serde_json::to_string(&files).map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn get_all_file_paths(&self) -> napi::Result<Vec<String>> {
        let storage = self.inner.lock().map_err(napi_err)?;
        storage.get_all_file_paths().map_err(napi_err)
    }

    // ---- Search ----

    #[napi]
    pub fn search_nodes(&self, query: String, options_json: String) -> napi::Result<String> {
        let options: SearchOptions = serde_json::from_str(&options_json)
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        let storage = self.inner.lock().map_err(napi_err)?;
        let results = storage.search_nodes(&query, &options).map_err(napi_err)?;
        serde_json::to_string(&results).map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    // ---- Stats & Metadata ----

    #[napi]
    pub fn get_stats(&self) -> napi::Result<String> {
        let storage = self.inner.lock().map_err(napi_err)?;
        let stats = storage.get_stats().map_err(napi_err)?;
        serde_json::to_string(&stats).map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn set_metadata(&self, key: String, value: String) -> napi::Result<()> {
        let storage = self.inner.lock().map_err(napi_err)?;
        storage.set_metadata(&key, &value).map_err(napi_err)
    }

    #[napi]
    pub fn get_metadata(&self, key: String) -> napi::Result<Option<String>> {
        let storage = self.inner.lock().map_err(napi_err)?;
        storage.get_metadata(&key).map_err(napi_err)
    }

    #[napi]
    pub fn clear(&self) -> napi::Result<()> {
        let storage = self.inner.lock().map_err(napi_err)?;
        storage.clear().map_err(napi_err)
    }
}

fn napi_err<E: std::fmt::Display>(e: E) -> napi::Error {
    napi::Error::from_reason(e.to_string())
}

fn parse_node_kind(s: &str) -> NodeKind {
    NodeKind::from_str(s).expect("invalid NodeKind")
}

fn parse_edge_kind(s: &str) -> EdgeKind {
    EdgeKind::from_str(s).expect("invalid EdgeKind")
}
