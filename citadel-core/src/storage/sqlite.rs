use crate::types::*;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use rusqlite::types::ToSql;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Mutex;

use super::{FastSet, FastMap, fast_set, fast_map, NodeMetrics};
use super::traits::*;
use crate::error::CitadelError;
use crate::extraction;
use crate::fs::{FileSystem, RealFileSystem};
use crate::storage::ffi;

const SCHEMA_SQL: &str = include_str!("../../../src/db/schema.sql");

const PRAGMAS: &[(&str, bool)] = &[
    ("PRAGMA journal_mode = WAL", true),
    ("PRAGMA foreign_keys = ON", false),
    ("PRAGMA busy_timeout = 120000", false),
    ("PRAGMA synchronous = NORMAL", false),
    ("PRAGMA cache_size = -64000", false),
    ("PRAGMA temp_store = MEMORY", false),
    ("PRAGMA mmap_size = 268435456", false),
];

/// Wrapper around `*mut sqlite3` that is Send/Sync.
/// Only accessed behind a Mutex (via StorageInner), so thread-safe.
#[derive(Clone, Copy)]
struct RawDb(*mut rusqlite::ffi::sqlite3);
unsafe impl Send for RawDb {}
unsafe impl Sync for RawDb {}

struct StorageInner {
    conn: Option<Connection>,
    path: Option<String>,
    in_batch: bool,
    db_ptr: Option<RawDb>,
}

pub struct SqliteStorage {
    inner: Mutex<StorageInner>,
    fs: Box<dyn FileSystem>,
}

impl SqliteStorage {
    pub fn new() -> Self {
        SqliteStorage {
            inner: Mutex::new(StorageInner { conn: None, path: None, in_batch: false, db_ptr: None }),
            fs: Box::new(RealFileSystem),
        }
    }

    pub fn with_fs(fs: Box<dyn FileSystem>) -> Self {
        SqliteStorage {
            inner: Mutex::new(StorageInner { conn: None, path: None, in_batch: false, db_ptr: None }),
            fs,
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, StorageInner>, CitadelError> {
        self.inner.lock().map_err(|e| CitadelError::Database(e.to_string()))
    }

    fn with<F, T>(&self, f: F) -> Result<T, CitadelError>
    where
        F: FnOnce(&Connection) -> Result<T, CitadelError>,
    {
        let guard = self.lock()?;
        let conn = guard.conn.as_ref().ok_or_else(|| CitadelError::NotInitialized("database not opened".into()))?;
        f(conn)
    }

    fn apply_pragmas(conn: &Connection, _is_wal_allowed: bool) -> Result<(), CitadelError> {
        for (pragma, _is_wal) in PRAGMAS {
            conn.execute_batch(pragma)?;
        }
        Ok(())
    }

    fn get_schema_version(conn: &Connection) -> Result<i32, CitadelError> {
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='schema_versions'",
                [],
                |row| row.get(0),
            )
            .map_err(|e: rusqlite::Error| CitadelError::Database(e.to_string()))?;

        if count == 0 {
            return Ok(0);
        }

        conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_versions",
            [],
            |row| row.get(0),
        )
        .map_err(|e| CitadelError::Database(e.to_string()))
    }

    fn run_schema(conn: &Connection) -> Result<(), CitadelError> {
        conn.execute_batch(SCHEMA_SQL)
            .map_err(|e| CitadelError::Database(e.to_string()))?;
        conn.execute(
            "UPDATE schema_versions SET version = 4, description = 'schema (current)' WHERE version = 1",
            [],
        )
        .map_err(|e| CitadelError::Database(e.to_string()))?;
        Ok(())
    }

    fn run_migrations(conn: &Connection, from_version: i32) -> Result<(), CitadelError> {
        if from_version < 2 {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS project_metadata (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL,
                    updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
                );
                ALTER TABLE unresolved_refs ADD COLUMN file_path TEXT NOT NULL DEFAULT '';
                ALTER TABLE unresolved_refs ADD COLUMN language TEXT NOT NULL DEFAULT '';
                ALTER TABLE edges ADD COLUMN provenance TEXT;
                CREATE INDEX IF NOT EXISTS idx_unresolved_file_path ON unresolved_refs(file_path);
                CREATE INDEX IF NOT EXISTS idx_edges_provenance ON edges(provenance);",
            )
            .map_err(|e| CitadelError::Migration(e.to_string()))?;
            conn.execute(
                "INSERT INTO schema_versions (version, description) VALUES (2, 'add project_metadata + provenance')",
                [],
            )
            .map_err(|e| CitadelError::Migration(e.to_string()))?;
        }
        if from_version < 3 {
            conn.execute_batch(
                "CREATE INDEX IF NOT EXISTS idx_nodes_lower_name ON nodes(lower(name));",
            )
            .map_err(|e| CitadelError::Migration(e.to_string()))?;
            conn.execute(
                "INSERT INTO schema_versions (version, description) VALUES (3, 'add lower(name) index')",
                [],
            )
            .map_err(|e| CitadelError::Migration(e.to_string()))?;
        }
        if from_version < 4 {
            conn.execute_batch(
                "DROP INDEX IF EXISTS idx_edges_source;
                 DROP INDEX IF EXISTS idx_edges_target;",
            )
            .map_err(|e| CitadelError::Migration(e.to_string()))?;
            conn.execute(
                "INSERT INTO schema_versions (version, description) VALUES (4, 'drop redundant edge indexes')",
                [],
            )
            .map_err(|e| CitadelError::Migration(e.to_string()))?;
        }
        Ok(())
    }

    fn insert_node_tx(tx: &Transaction, node: &Node) -> Result<(), CitadelError> {
        tx.execute(
            "INSERT OR REPLACE INTO nodes (
                id, kind, name, qualified_name, file_path, language,
                start_line, end_line, start_column, end_column,
                docstring, signature, visibility,
                is_exported, is_async, is_static, is_abstract,
                decorators, type_parameters, updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6,
                ?7, ?8, ?9, ?10,
                ?11, ?12, ?13,
                ?14, ?15, ?16, ?17,
                ?18, ?19, ?20
            )",
            params![
                node.id,
                node.kind.as_str(),
                node.name,
                node.qualified_name,
                node.file_path,
                node.language.as_str(),
                node.start_line,
                node.end_line,
                node.start_column,
                node.end_column,
                node.docstring,
                node.signature,
                node.visibility,
                node.is_exported as i32,
                node.is_async as i32,
                node.is_static as i32,
                node.is_abstract as i32,
                node.decorators.as_ref().map(|d| serde_json::to_string(d).unwrap_or_default()),
                node.type_parameters.as_ref().map(|t| serde_json::to_string(t).unwrap_or_default()),
                node.updated_at,
            ],
        )
        .map_err(|e| CitadelError::Database(e.to_string()))?;
        Ok(())
    }

    fn row_to_node(row: &rusqlite::Row) -> Result<Node, rusqlite::Error> {
        Ok(Node {
            id: row.get("id")?,
            kind: parse_node_kind(row.get::<_, String>("kind")?.as_str()),
            name: row.get("name")?,
            qualified_name: row.get("qualified_name")?,
            file_path: row.get("file_path")?,
            language: parse_language(row.get::<_, String>("language")?.as_str()),
            start_line: row.get("start_line")?,
            end_line: row.get("end_line")?,
            start_column: row.get("start_column")?,
            end_column: row.get("end_column")?,
            docstring: row.get("docstring")?,
            signature: row.get("signature")?,
            visibility: row.get("visibility")?,
            is_exported: row.get::<_, i32>("is_exported")? != 0,
            is_async: row.get::<_, i32>("is_async")? != 0,
            is_static: row.get::<_, i32>("is_static")? != 0,
            is_abstract: row.get::<_, i32>("is_abstract")? != 0,
            decorators: row
                .get::<_, Option<String>>("decorators")?
                .and_then(|s| serde_json::from_str(&s).ok()),
            type_parameters: row
                .get::<_, Option<String>>("type_parameters")?
                .and_then(|s| serde_json::from_str(&s).ok()),
            updated_at: row.get("updated_at")?,
        })
    }

    fn row_to_edge(row: &rusqlite::Row) -> Result<Edge, rusqlite::Error> {
        Ok(Edge {
            source: row.get("source")?,
            target: row.get("target")?,
            kind: parse_edge_kind(row.get::<_, String>("kind")?.as_str()),
            metadata: row
                .get::<_, Option<String>>("metadata")?
                .and_then(|s| serde_json::from_str(&s).ok()),
            line: row.get("line")?,
            column: row.get("col")?,
            provenance: row.get("provenance")?,
        })
    }

    fn row_to_file(row: &rusqlite::Row) -> Result<FileRecord, rusqlite::Error> {
        Ok(FileRecord {
            path: row.get("path")?,
            content_hash: row.get("content_hash")?,
            language: parse_language(row.get::<_, String>("language")?.as_str()),
            size: row.get("size")?,
            modified_at: row.get("modified_at")?,
            indexed_at: row.get("indexed_at")?,
            node_count: row.get("node_count")?,
            errors: row
                .get::<_, Option<String>>("errors")?
                .and_then(|s| serde_json::from_str(&s).ok()),
        })
    }

    fn row_to_unresolved_ref(row: &rusqlite::Row) -> Result<UnresolvedRef, rusqlite::Error> {
        Ok(UnresolvedRef {
            from_node_id: row.get("from_node_id")?,
            reference_name: row.get("reference_name")?,
            reference_kind: row.get("reference_kind")?,
            line: row.get("line")?,
            column: row.get("col")?,
            candidates: row
                .get::<_, Option<String>>("candidates")?
                .and_then(|s| serde_json::from_str(&s).ok()),
            file_path: row.get("file_path")?,
            language: row.get("language")?,
        })
    }
}

impl Default for SqliteStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl Lifecycle for SqliteStorage {
    fn initialize(&mut self, db_path: &str) -> Result<(), CitadelError> {
        let path = std::path::Path::new(db_path);
        self.fs.create_dir_all(path)?;
        let conn = Connection::open(db_path)?;
        Self::apply_pragmas(&conn, true)?;
        Self::run_schema(&conn)?;
        let mut guard = self.inner.lock().map_err(|e| CitadelError::Database(e.to_string()))?;
        guard.conn = Some(conn);
        guard.path = Some(db_path.to_string());
        Ok(())
    }

    fn open(&mut self, db_path: &str) -> Result<(), CitadelError> {
        use std::ffi::CString;
        use rusqlite::ffi;

        let path = std::path::Path::new(db_path);
        let was_new = !self.fs.exists(path);
        if was_new {
            if let Some(parent) = path.parent() {
                self.fs.create_dir_all(parent)?;
            }
        }

        let c_path = CString::new(db_path).map_err(|e| CitadelError::Database(e.to_string()))?;
        let flags = ffi::SQLITE_OPEN_READWRITE | ffi::SQLITE_OPEN_CREATE | ffi::SQLITE_OPEN_URI;
        let db: *mut ffi::sqlite3;

        unsafe {
            let mut raw_db: *mut ffi::sqlite3 = std::ptr::null_mut();
            let rc = ffi::sqlite3_open_v2(c_path.as_ptr(), &mut raw_db, flags, std::ptr::null());
            if rc != ffi::SQLITE_OK {
                let err = if raw_db.is_null() {
                    ffi::sqlite3_errstr(rc)
                } else {
                    ffi::sqlite3_errmsg(raw_db)
                };
                return Err(CitadelError::Database(format!("open failed: {}", std::ffi::CStr::from_ptr(err).to_string_lossy())));
            }
            db = raw_db;
        }

        let conn = unsafe {
            Connection::from_handle(db).map_err(|e| CitadelError::Database(e.to_string()))?
        };
        Self::apply_pragmas(&conn, true)?;
        if was_new {
            Self::run_schema(&conn)?;
        }
        let current_version = Self::get_schema_version(&conn)?;
        if current_version > 0 && current_version < 4 {
            Self::run_migrations(&conn, current_version)?;
        }
        let mut guard = self.inner.lock().map_err(|e| CitadelError::Database(e.to_string()))?;
        guard.conn = Some(conn);
        guard.path = Some(db_path.to_string());
        guard.db_ptr = Some(RawDb(db));
        Ok(())
    }

    fn close(&mut self) -> Result<(), CitadelError> {
        use rusqlite::ffi;
        let mut guard = self.inner.lock().map_err(|e| CitadelError::Database(e.to_string()))?;
        guard.conn = None;
        guard.path = None;
        if let Some(RawDb(ptr)) = guard.db_ptr.take() {
            unsafe { ffi::sqlite3_close(ptr); }
        }
        Ok(())
    }

    fn get_path(&self) -> Option<String> {
        self.lock().ok().and_then(|g| g.path.clone())
    }

    fn clear(&self) -> Result<(), CitadelError> {
        self.with(|conn| {
            conn.execute_batch(
                "DELETE FROM nodes; DELETE FROM edges; DELETE FROM files; DELETE FROM unresolved_refs;",
            )?;
            Ok(())
        })
    }
}

impl NodeStore for SqliteStorage {
    // ---- Nodes ----

    fn insert_node(&self, node: &Node) -> Result<(), CitadelError> {
        self.with(|conn| {
            let tx = conn.unchecked_transaction()?;
            Self::insert_node_tx(&tx, node)?;
            tx.commit()?;
            Ok(())
        })
    }

    fn insert_nodes(&self, nodes: &[Node]) -> Result<(), CitadelError> {
        self.with(|conn| {
            let tx = conn.unchecked_transaction()?;
            {
                let mut stmt = tx.prepare(
                    "INSERT OR REPLACE INTO nodes (
                        id, kind, name, qualified_name, file_path, language,
                        start_line, end_line, start_column, end_column,
                        docstring, signature, visibility,
                        is_exported, is_async, is_static, is_abstract,
                        decorators, type_parameters, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                              ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)"
                )?;
                for node in nodes {
                    stmt.execute(params![
                        node.id, node.kind.as_str(), node.name, node.qualified_name,
                        node.file_path, node.language.as_str(),
                        node.start_line, node.end_line, node.start_column, node.end_column,
                        node.docstring, node.signature, node.visibility,
                        node.is_exported as i32, node.is_async as i32,
                        node.is_static as i32, node.is_abstract as i32,
                        node.decorators.as_ref().map(|d| serde_json::to_string(d).unwrap_or_default()),
                        node.type_parameters.as_ref().map(|t| serde_json::to_string(t).unwrap_or_default()),
                        node.updated_at,
                    ])?;
                }
            }
            tx.commit()?;
            Ok(())
        })
    }

    fn update_node(&self, node: &Node) -> Result<(), CitadelError> {
        self.with(|conn| {
            conn.execute(
                "UPDATE nodes SET
                    kind=?2, name=?3, qualified_name=?4, file_path=?5, language=?6,
                    start_line=?7, end_line=?8, start_column=?9, end_column=?10,
                    docstring=?11, signature=?12, visibility=?13,
                    is_exported=?14, is_async=?15, is_static=?16, is_abstract=?17,
                    decorators=?18, type_parameters=?19, updated_at=?20
                WHERE id=?1",
                params![
                    node.id, node.kind.as_str(), node.name, node.qualified_name,
                    node.file_path, node.language.as_str(),
                    node.start_line, node.end_line, node.start_column, node.end_column,
                    node.docstring, node.signature, node.visibility,
                    node.is_exported as i32, node.is_async as i32,
                    node.is_static as i32, node.is_abstract as i32,
                    node.decorators.as_ref().map(|d| serde_json::to_string(d).unwrap_or_default()),
                    node.type_parameters.as_ref().map(|t| serde_json::to_string(t).unwrap_or_default()),
                    node.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    fn delete_node(&self, id: &str) -> Result<(), CitadelError> {
        self.with(|conn| {
            conn.execute("DELETE FROM nodes WHERE id = ?1", params![id])?;
            Ok(())
        })
    }

    fn delete_nodes_by_file(&self, file_path: &str) -> Result<(), CitadelError> {
        self.with(|conn| {
            conn.execute("DELETE FROM nodes WHERE file_path = ?1", params![file_path])?;
            Ok(())
        })
    }

    fn get_node_by_id(&self, id: &str) -> Result<Option<Node>, CitadelError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM nodes WHERE id = ?1")?;
            let result = stmt
                .query_row(params![id], Self::row_to_node)
                .optional()?;
            Ok(result)
        })
    }

    fn get_nodes_by_file(&self, file_path: &str) -> Result<Vec<Node>, CitadelError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM nodes WHERE file_path = ?1 ORDER BY start_line")?;
            let rows = stmt.query_map(params![file_path], Self::row_to_node)?;
            let mut nodes = Vec::new();
            for row in rows {
                nodes.push(row?);
            }
            Ok(nodes)
        })
    }

    fn get_nodes_by_kind(&self, kind: &NodeKind) -> Result<Vec<Node>, CitadelError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM nodes WHERE kind = ?1")?;
            let rows = stmt.query_map(params![kind.as_str()], Self::row_to_node)?;
            let mut nodes = Vec::new();
            for row in rows {
                nodes.push(row?);
            }
            Ok(nodes)
        })
    }

    fn get_all_nodes(&self) -> Result<Vec<Node>, CitadelError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM nodes")?;
            let rows = stmt.query_map([], Self::row_to_node)?;
            // Pre-allocate based on a reasonable estimate: typical codebases
            // have 1k-100k nodes. Using 4096 avoids the first few reallocations
            // without wasting memory on tiny projects.
            let mut nodes = Vec::with_capacity(4096);
            for row in rows {
                nodes.push(row?);
            }
            Ok(nodes)
        })
    }

    fn get_nodes_by_name(&self, name: &str) -> Result<Vec<Node>, CitadelError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM nodes WHERE name = ?1")?;
            let rows = stmt.query_map(params![name], Self::row_to_node)?;
            let mut nodes = Vec::new();
            for row in rows {
                nodes.push(row?);
            }
            Ok(nodes)
        })
    }

    fn get_nodes_by_qualified_name(&self, qn: &str) -> Result<Vec<Node>, CitadelError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM nodes WHERE qualified_name = ?1")?;
            let rows = stmt.query_map(params![qn], Self::row_to_node)?;
            let mut nodes = Vec::new();
            for row in rows {
                nodes.push(row?);
            }
            Ok(nodes)
        })
    }

    fn get_nodes_by_lower_name(&self, name: &str) -> Result<Vec<Node>, CitadelError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM nodes WHERE lower(name) = lower(?1)")?;
            let rows = stmt.query_map(params![name], Self::row_to_node)?;
            let mut nodes = Vec::new();
            for row in rows {
                nodes.push(row?);
            }
            Ok(nodes)
        })
    }

    // ---- Search ----

    fn search_nodes(&self, query: &str, options: &SearchOptions) -> Result<Vec<SearchResult>, CitadelError> {
        self.with(|conn| {
            let trimmed = query.trim();
            if !trimmed.is_empty() {
                let results = self.fts5_search(conn, trimmed, options)?;
                if !results.is_empty() {
                    return Ok(results);
                }
            }
            if trimmed.len() >= 2 {
                let results = self.like_search(conn, trimmed, options)?;
                if !results.is_empty() {
                    return Ok(results);
                }
            }
            self.filter_search(conn, options)
        })
    }
}

impl EdgeStore for SqliteStorage {
    // ---- Edges ----

    fn insert_edge(&self, edge: &Edge) -> Result<(), CitadelError> {
        self.with(|conn| {
            conn.execute(
                "INSERT OR IGNORE INTO edges (source, target, kind, metadata, line, col, provenance)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    edge.source,
                    edge.target,
                    edge.kind.as_str(),
                    edge.metadata.as_ref().map(|m| m.to_string()),
                    edge.line,
                    edge.column,
                    edge.provenance,
                ],
            )?;
            Ok(())
        })
    }

    fn insert_edges(&self, edges: &[Edge]) -> Result<(), CitadelError> {
        self.with(|conn| {
            let tx = conn.unchecked_transaction()?;
            let mut stmt = tx.prepare(
                "INSERT OR IGNORE INTO edges (source, target, kind, metadata, line, col, provenance)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
            )?;
            for edge in edges {
                stmt.execute(params![
                    edge.source, edge.target, edge.kind.as_str(),
                    edge.metadata.as_ref().map(|m| m.to_string()),
                    edge.line, edge.column, edge.provenance,
                ])?;
            }
            drop(stmt);
            tx.commit()?;
            Ok(())
        })
    }

    fn delete_edges_by_source(&self, source_id: &str) -> Result<(), CitadelError> {
        self.with(|conn| {
            conn.execute("DELETE FROM edges WHERE source = ?1", params![source_id])?;
            Ok(())
        })
    }

    fn get_outgoing_edges(&self, source_id: &str, kinds: Option<&[EdgeKind]>, provenance: Option<&str>) -> Result<Vec<Edge>, CitadelError> {
        self.with(|conn| {
            let mut sql = String::from("SELECT * FROM edges WHERE source = ?");
            let mut param_values: Vec<Box<dyn ToSql>> = vec![Box::new(source_id.to_string())];

            let kind_filter = kinds.and_then(|k| k.first().map(|k| k.as_str()));
            if let Some(k) = kind_filter {
                sql.push_str(" AND kind = ?");
                param_values.push(Box::new(k.to_string()));
            }
            if let Some(p) = provenance {
                sql.push_str(" AND provenance = ?");
                param_values.push(Box::new(p.to_string()));
            }

            let param_refs: Vec<&dyn ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(param_refs.as_slice(), Self::row_to_edge)?;
            let mut edges = Vec::new();
            for row in rows {
                edges.push(row?);
            }
            Ok(edges)
        })
    }

    fn get_incoming_edges(&self, target_id: &str, kinds: Option<&[EdgeKind]>) -> Result<Vec<Edge>, CitadelError> {
        self.with(|conn| {
            let mut sql = String::from("SELECT * FROM edges WHERE target = ?");
            let mut param_values: Vec<Box<dyn ToSql>> = vec![Box::new(target_id.to_string())];

            if let Some(k) = kinds.and_then(|k| k.first().map(|k| k.as_str())) {
                sql.push_str(" AND kind = ?");
                param_values.push(Box::new(k.to_string()));
            }

            let param_refs: Vec<&dyn ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(param_refs.as_slice(), Self::row_to_edge)?;
            let mut edges = Vec::new();
            for row in rows {
                edges.push(row?);
            }
            Ok(edges)
        })
    }

    fn find_edges_between_nodes(&self, node_ids: &[String], kinds: Option<&[EdgeKind]>) -> Result<Vec<Edge>, CitadelError> {
        self.with(|conn| {
            let ids_json = serde_json::to_string(node_ids)
                .map_err(|e| CitadelError::Serialization(e.to_string()))?;

            let mut sql = String::from(
                "SELECT * FROM edges WHERE source IN (SELECT value FROM json_each(?)) \
                 AND target IN (SELECT value FROM json_each(?))"
            );
            let mut param_values: Vec<Box<dyn ToSql>> = vec![
                Box::new(ids_json.clone()),
                Box::new(ids_json),
            ];

            if let Some(k) = kinds.and_then(|k| k.first().map(|k| k.as_str())) {
                sql.push_str(" AND kind = ?");
                param_values.push(Box::new(k.to_string()));
            }

            let param_refs: Vec<&dyn ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(param_refs.as_slice(), Self::row_to_edge)?;
            let mut edges = Vec::new();
            for row in rows {
                edges.push(row?);
            }
            Ok(edges)
        })
    }
}

impl FileStore for SqliteStorage {
    // ---- Files ----

    fn upsert_file(&self, file: &FileRecord) -> Result<(), CitadelError> {
        self.with(|conn| {
            conn.execute(
                "INSERT INTO files (path, content_hash, language, size, modified_at, indexed_at, node_count, errors)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(path) DO UPDATE SET
                    content_hash=excluded.content_hash, language=excluded.language,
                    size=excluded.size, modified_at=excluded.modified_at,
                    indexed_at=excluded.indexed_at, node_count=excluded.node_count,
                    errors=excluded.errors",
                params![
                    file.path, file.content_hash, file.language.as_str(),
                    file.size, file.modified_at, file.indexed_at,
                    file.node_count,
                    file.errors.as_ref().map(|e| serde_json::to_string(e).unwrap_or_default()),
                ],
            )?;
            Ok(())
        })
    }

    fn delete_file(&self, path: &str) -> Result<(), CitadelError> {
        self.with(|conn| {
            let tx = conn.unchecked_transaction()?;
            tx.execute("DELETE FROM nodes WHERE file_path = ?1", params![path])?;
            tx.execute("DELETE FROM files WHERE path = ?1", params![path])?;
            tx.commit()?;
            Ok(())
        })
    }

    fn get_file_by_path(&self, path: &str) -> Result<Option<FileRecord>, CitadelError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM files WHERE path = ?1")?;
            let result = stmt
                .query_row(params![path], Self::row_to_file)
                .optional()?;
            Ok(result)
        })
    }

    fn get_all_files(&self) -> Result<Vec<FileRecord>, CitadelError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM files ORDER BY path")?;
            let rows = stmt.query_map([], Self::row_to_file)?;
            // Pre-allocate: typical projects have 50-5000 files. 256 avoids
            // the first few reallocations without over-committing on small repos.
            let mut files = Vec::with_capacity(256);
            for row in rows {
                files.push(row?);
            }
            Ok(files)
        })
    }

    fn get_stale_files(&self, current_hashes: &HashMap<String, String>) -> Result<Vec<FileRecord>, CitadelError> {
        let all = self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM files ORDER BY path")?;
            let rows = stmt.query_map([], Self::row_to_file)?;
            let mut files = Vec::new();
            for row in rows {
                files.push(row?);
            }
            Ok(files)
        })?;
        Ok(all
            .into_iter()
            .filter(|f| current_hashes.get(&f.path).is_none_or(|h| *h != f.content_hash))
            .collect())
    }

    fn get_all_file_paths(&self) -> Result<Vec<String>, CitadelError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT path FROM files ORDER BY path")?;
            let paths = stmt.query_map([], |row| row.get::<_, String>(0))?;
            let mut result = Vec::new();
            for p in paths {
                result.push(p?);
            }
            Ok(result)
        })
    }

    fn get_all_node_names(&self) -> Result<Vec<String>, CitadelError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT DISTINCT name FROM nodes")?;
            let names = stmt.query_map([], |row| row.get::<_, String>(0))?;
            let mut result = Vec::new();
            for n in names {
                result.push(n?);
            }
            Ok(result)
        })
    }
}

impl UnresolvedRefStore for SqliteStorage {
    // ---- Unresolved refs ----

    fn insert_unresolved_ref(&self, r#ref: &UnresolvedRef) -> Result<(), CitadelError> {
        self.with(|conn| {
            conn.execute(
                "INSERT INTO unresolved_refs (from_node_id, reference_name, reference_kind, line, col, candidates, file_path, language)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    r#ref.from_node_id, r#ref.reference_name, r#ref.reference_kind,
                    r#ref.line, r#ref.column,
                    r#ref.candidates.as_ref().map(|c| c.to_string()),
                    r#ref.file_path, r#ref.language,
                ],
            )?;
            Ok(())
        })
    }

    fn insert_unresolved_refs_batch(&self, refs: &[UnresolvedRef]) -> Result<(), CitadelError> {
        self.with(|conn| {
            let tx = conn.unchecked_transaction()?;
            let mut stmt = tx.prepare(
                "INSERT INTO unresolved_refs (from_node_id, reference_name, reference_kind, line, col, candidates, file_path, language)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
            )?;
            for r in refs {
                stmt.execute(params![
                    r.from_node_id, r.reference_name, r.reference_kind,
                    r.line, r.column,
                    r.candidates.as_ref().map(|c| c.to_string()),
                    r.file_path, r.language,
                ])?;
            }
            drop(stmt);
            tx.commit()?;
            Ok(())
        })
    }

    fn delete_unresolved_by_node(&self, node_id: &str) -> Result<(), CitadelError> {
        self.with(|conn| {
            conn.execute("DELETE FROM unresolved_refs WHERE from_node_id = ?1", params![node_id])?;
            Ok(())
        })
    }

    fn get_unresolved_by_name(&self, name: &str) -> Result<Vec<UnresolvedRef>, CitadelError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM unresolved_refs WHERE reference_name = ?1")?;
            let rows = stmt.query_map(params![name], Self::row_to_unresolved_ref)?;
            let mut refs = Vec::new();
            for row in rows {
                refs.push(row?);
            }
            Ok(refs)
        })
    }

    fn get_all_unresolved_refs(&self) -> Result<Vec<UnresolvedRef>, CitadelError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM unresolved_refs")?;
            let rows = stmt.query_map([], Self::row_to_unresolved_ref)?;
            let mut refs = Vec::new();
            for row in rows {
                refs.push(row?);
            }
            Ok(refs)
        })
    }

    fn get_unresolved_refs_count(&self) -> Result<u64, CitadelError> {
        self.with(|conn| {
            let count: u64 = conn.query_row("SELECT COUNT(*) FROM unresolved_refs", [], |row| row.get(0))?;
            Ok(count)
        })
    }

    fn get_unresolved_refs_batch(&self, offset: u64, limit: u64) -> Result<Vec<UnresolvedRef>, CitadelError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM unresolved_refs LIMIT ?1 OFFSET ?2")?;
            let rows = stmt.query_map(params![limit, offset], Self::row_to_unresolved_ref)?;
            let mut refs = Vec::new();
            for row in rows {
                refs.push(row?);
            }
            Ok(refs)
        })
    }

    fn get_unresolved_refs_by_files(&self, file_paths: &[String]) -> Result<Vec<UnresolvedRef>, CitadelError> {
        self.with(|conn| {
            let paths_json = serde_json::to_string(file_paths)
                .map_err(|e| CitadelError::Serialization(e.to_string()))?;
            let mut stmt = conn.prepare(
                "SELECT * FROM unresolved_refs WHERE file_path IN (SELECT value FROM json_each(?1))"
            )?;
            let rows = stmt.query_map(params![paths_json], Self::row_to_unresolved_ref)?;
            let mut refs = Vec::new();
            for row in rows {
                refs.push(row?);
            }
            Ok(refs)
        })
    }

    fn clear_unresolved_refs(&self) -> Result<(), CitadelError> {
        self.with(|conn| {
            conn.execute("DELETE FROM unresolved_refs", [])?;
            Ok(())
        })
    }

    fn delete_resolved_refs(&self, from_node_ids: &[String]) -> Result<(), CitadelError> {
        self.with(|conn| {
            let ids_json = serde_json::to_string(from_node_ids)
                .map_err(|e| CitadelError::Serialization(e.to_string()))?;
            conn.execute(
                "DELETE FROM unresolved_refs WHERE from_node_id IN (SELECT value FROM json_each(?1))",
                params![ids_json],
            )?;
            Ok(())
        })
    }

    fn delete_specific_resolved_refs(&self, refs: &[UnresolvedRef]) -> Result<(), CitadelError> {
        self.with(|conn| {
            let tx = conn.unchecked_transaction()?;
            for r in refs {
                tx.execute(
                    "DELETE FROM unresolved_refs WHERE from_node_id = ?1 AND reference_name = ?2 AND reference_kind = ?3",
                    params![r.from_node_id, r.reference_name, r.reference_kind],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
    }
}

impl StatsProvider for SqliteStorage {
    // ---- Stats & Metadata ----

    fn get_stats(&self) -> Result<GraphStats, CitadelError> {
        self.with(|conn| {
            let (node_count, edge_count, file_count): (u64, u64, u64) = conn
                .query_row(
                    "SELECT
                        (SELECT COUNT(*) FROM nodes),
                        (SELECT COUNT(*) FROM edges),
                        (SELECT COUNT(*) FROM files)",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?;

            let nodes_by_kind = Self::count_map(conn, "SELECT kind, COUNT(*) as c FROM nodes GROUP BY kind")?;
            let edges_by_kind = Self::count_map(conn, "SELECT kind, COUNT(*) as c FROM edges GROUP BY kind")?;
            let files_by_language = Self::count_map(conn, "SELECT language, COUNT(*) as c FROM files GROUP BY language")?;

            let db_size = conn
                .query_row("SELECT page_count * page_size FROM pragma_page_count, pragma_page_size", [], |row| {
                    row.get::<_, u64>(0)
                })
                .unwrap_or(0);

            let last_updated = conn
                .query_row("SELECT COALESCE(MAX(updated_at), 0) FROM nodes", [], |row| row.get(0))
                .unwrap_or(0);

            Ok(GraphStats {
                node_count,
                edge_count,
                file_count,
                nodes_by_kind,
                edges_by_kind,
                files_by_language,
                db_size_bytes: db_size,
                last_updated,
            })
        })
    }
}

impl MetadataStore for SqliteStorage {
    fn get_metadata(&self, key: &str) -> Result<Option<String>, CitadelError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT value FROM project_metadata WHERE key = ?1")?;
            let result = stmt.query_row(params![key], |row| row.get(0)).optional()?;
            Ok(result)
        })
    }

    fn set_metadata(&self, key: &str, value: &str) -> Result<(), CitadelError> {
        self.with(|conn| {
            conn.execute(
                "INSERT INTO project_metadata (key, value, updated_at) VALUES (?1, ?2, strftime('%s','now'))
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=excluded.updated_at",
                params![key, value],
            )?;
            Ok(())
        })
    }

    fn get_all_metadata(&self) -> Result<HashMap<String, String>, CitadelError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT key, value FROM project_metadata")?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            let mut map = HashMap::new();
            for row in rows {
                let (k, v) = row?;
                map.insert(k, v);
            }
            Ok(map)
        })
    }
}

impl SqliteStorage {
    // Traversal and utility methods that will move to GraphQuery in Phase 2.
    // These remain on SqliteStorage directly until the Graph module is created.

    #[allow(dead_code)]
    fn traverse_bfs(
        &self,
        start_id: &str,
        options: &TraversalOptions,
    ) -> Result<Vec<(Node, Vec<Edge>)>, CitadelError> {
        use std::collections::VecDeque;
        let start_node = self.get_node_by_id(start_id)?
            .ok_or_else(|| CitadelError::NotFound(format!("start node not found: {start_id}")))?;

        let mut visited: FastSet<String> = fast_set();
        let mut queue: VecDeque<(Node, usize)> = VecDeque::new();
        let mut results: Vec<(Node, Vec<Edge>)> = Vec::new();
        let mut node_edges_map: FastMap<String, Vec<Edge>> = fast_map();

        visited.insert(start_id.to_string());
        queue.push_back((start_node.clone(), 0));

        while let Some((current_node, depth)) = queue.pop_front() {
            if depth > options.max_depth {
                break;
            }
            if results.len() >= options.limit {
                break;
            }

            let edges = self.with(|conn| {
                let mut sql = String::from("SELECT id, source, target, kind, metadata, line, col, provenance FROM edges WHERE ");
                let mut params_vec: Vec<Box<dyn ToSql>> = Vec::new();
                match options.direction {
                    TraversalDirection::Outgoing => {
                        sql.push_str("source = ?");
                        params_vec.push(Box::new(current_node.id.clone()));
                    }
                    TraversalDirection::Incoming => {
                        sql.push_str("target = ?");
                        params_vec.push(Box::new(current_node.id.clone()));
                    }
                    TraversalDirection::Both => {
                        sql.push_str("(source = ? OR target = ?)");
                        params_vec.push(Box::new(current_node.id.clone()));
                        params_vec.push(Box::new(current_node.id.clone()));
                    }
                }
                if !options.edge_kinds.is_empty() {
                    sql.push_str(" AND kind IN (");
                    for (i, _) in options.edge_kinds.iter().enumerate() {
                        if i > 0 { sql.push(','); }
                        sql.push_str(&format!("?{}", i + params_vec.len() + 1));
                    }
                    sql.push(')');
                    for k in &options.edge_kinds {
                        params_vec.push(Box::new(k.as_str().to_string()));
                    }
                }
                let params_refs: Vec<&dyn ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
                let mut stmt = conn.prepare(&sql)?;
                let rows = stmt.query_map(params_refs.as_slice(), |row| {
                    Ok(Edge {
                        source: row.get(1)?,
                        target: row.get(2)?,
                        kind: EdgeKind::from_str(row.get::<_, String>(3)?.as_str()).unwrap_or(EdgeKind::References),
                        metadata: row.get::<_, Option<String>>(4)?.and_then(|s| serde_json::from_str(&s).ok()),
                        line: row.get(5)?,
                        column: row.get(6)?,
                        provenance: row.get(7)?,
                    })
                })?;
                let mut edges = Vec::new();
                for e in rows.flatten() {
                    edges.push(e);
                }
                Ok::<_, CitadelError>(edges)
            })?;

            node_edges_map.insert(current_node.id.clone(), edges.clone());

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

    #[allow(dead_code)]
    fn get_ancestors(&self, node_id: &str) -> Result<Vec<Node>, CitadelError> {
        let mut ancestors = Vec::new();
        let mut current_id = node_id.to_string();
        let max_iterations = 100;
        for _ in 0..max_iterations {
            let parent_edges = self.with(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT e.source, n.id, n.kind, n.name, n.qualified_name, n.file_path, n.language, n.start_line, n.end_line, n.start_column, n.end_column, n.docstring, n.signature, n.visibility, n.is_exported, n.is_async, n.is_static, n.is_abstract, n.decorators, n.type_parameters, n.updated_at FROM edges e JOIN nodes n ON e.source = n.id WHERE e.target = ? AND e.kind = 'contains' LIMIT 1"
                )?;
                let row = stmt.query_row(params![current_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        Self::row_to_node(row)?,
                    ))
                }).optional()?;
                Ok::<_, CitadelError>(row)
            })?;

            if let Some((parent_id, parent_node)) = parent_edges {
                ancestors.push(parent_node);
                current_id = parent_id;
            } else {
                break;
            }
        }
        Ok(ancestors)
    }

    #[allow(dead_code)]
    fn get_node_metrics(&self, node_id: &str) -> Result<NodeMetrics, CitadelError> {
        let incoming = self.with(|conn| {
            let mut stmt = conn.prepare("SELECT kind FROM edges WHERE target = ?")?;
            let kinds: Vec<String> = stmt.query_map(params![node_id], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            Ok::<_, CitadelError>(kinds)
        })?;

        let outgoing = self.with(|conn| {
            let mut stmt = conn.prepare("SELECT kind FROM edges WHERE source = ?")?;
            let kinds: Vec<String> = stmt.query_map(params![node_id], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            Ok::<_, CitadelError>(kinds)
        })?;

        let call_count = incoming.iter().filter(|k| *k == "calls").count() as u32
            + outgoing.iter().filter(|k| *k == "calls").count() as u32;
        let caller_count = incoming.iter().filter(|k| *k == "calls").count() as u32;
        let child_count = outgoing.iter().filter(|k| *k == "contains").count() as u32;
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

impl SqliteStorage {
    fn count_map(conn: &Connection, sql: &str) -> Result<HashMap<String, u64>, CitadelError> {
        let mut stmt = conn.prepare(sql).map_err(|e| CitadelError::Database(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
            })
            .map_err(|e| CitadelError::Database(e.to_string()))?;
        let mut map = HashMap::new();
        for row in rows {
            let (k, v) = row.map_err(|e| CitadelError::Database(e.to_string()))?;
            map.insert(k, v);
        }
        Ok(map)
    }

    fn fts5_search(&self, conn: &Connection, query: &str, options: &SearchOptions) -> Result<Vec<SearchResult>, CitadelError> {
        let fts_query = query
            .replace("::", " ")
            .chars()
            .filter(|c| !matches!(c, '\'' | '"' | '*' | '(' | ')' | ':' | '^'))
            .collect::<String>()
            .split_whitespace()
            .filter(|t| !t.is_empty() && !matches!(*t, "AND" | "OR" | "NOT" | "NEAR"))
            .map(|t| format!("\"{}\"*", t))
            .collect::<Vec<_>>()
            .join(" OR ");

        if fts_query.is_empty() {
            return Ok(vec![]);
        }

        let fts_limit = (options.limit * 5).max(100) as i64;
        let offset = options.offset as i64;

        let mut sql = String::from(
            "SELECT nodes.*, bm25(nodes_fts, 0, 20, 5, 1, 2) as score \
             FROM nodes_fts JOIN nodes ON nodes_fts.id = nodes.id \
             WHERE nodes_fts MATCH ?"
        );

        let mut params: Vec<Box<dyn ToSql>> = vec![Box::new(fts_query.to_string())];

        if let Some(kinds) = &options.kinds {
            let placeholders = vec!["?"; kinds.len()].join(",");
            sql.push_str(&format!(" AND nodes.kind IN ({})", placeholders));
            for kind in kinds {
                params.push(Box::new(kind.as_str().to_string()));
            }
        }

        if let Some(languages) = &options.languages {
            let placeholders = vec!["?"; languages.len()].join(",");
            sql.push_str(&format!(" AND nodes.language IN ({})", placeholders));
            for lang in languages {
                params.push(Box::new(lang.as_str().to_string()));
            }
        }

        sql.push_str(" ORDER BY score LIMIT ? OFFSET ?");
        params.push(Box::new(fts_limit));
        params.push(Box::new(offset));

        let param_refs: Vec<&dyn ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(|e| CitadelError::Database(e.to_string()))?;

        let rows = stmt
            .query_map(param_refs.as_slice(), |row| {
                let node = Self::row_to_node(row)?;
                let score: f64 = row.get("score")?;
                Ok(SearchResult {
                    node,
                    score: score.abs(),
                    highlights: None,
                })
            })
            .map_err(|e| CitadelError::Database(e.to_string()))?;

        let mut results = Vec::with_capacity(fts_limit as usize);
        for row in rows {
            results.push(row.map_err(|e| CitadelError::Database(e.to_string()))?);
        }
        Ok(results)
    }

    fn like_search(&self, conn: &Connection, query: &str, options: &SearchOptions) -> Result<Vec<SearchResult>, CitadelError> {
        let starts_with = format!("{}%", query);
        let contains = format!("%{}%", query);
        let limit = options.limit as i64;
        let offset = options.offset as i64;

        let mut sql = String::from(
            "SELECT nodes.*, \
             CASE \
               WHEN name = ? THEN 1.0 \
               WHEN name LIKE ? THEN 0.9 \
               WHEN name LIKE ? THEN 0.8 \
               WHEN qualified_name LIKE ? THEN 0.7 \
               ELSE 0.5 \
             END as score \
             FROM nodes \
             WHERE (name LIKE ? OR qualified_name LIKE ? OR name LIKE ?)"
        );

        let mut params: Vec<Box<dyn ToSql>> = vec![
            Box::new(query.to_string()),
            Box::new(starts_with.clone()),
            Box::new(contains.clone()),
            Box::new(contains.clone()),
            Box::new(contains.clone()),
            Box::new(contains),
            Box::new(starts_with),
        ];

        if let Some(kinds) = &options.kinds {
            let placeholders = vec!["?"; kinds.len()].join(",");
            sql.push_str(&format!(" AND kind IN ({})", placeholders));
            for kind in kinds {
                params.push(Box::new(kind.as_str().to_string()));
            }
        }

        if let Some(languages) = &options.languages {
            let placeholders = vec!["?"; languages.len()].join(",");
            sql.push_str(&format!(" AND language IN ({})", placeholders));
            for lang in languages {
                params.push(Box::new(lang.as_str().to_string()));
            }
        }

        sql.push_str(" ORDER BY score DESC LIMIT ? OFFSET ?");
        params.push(Box::new(limit));
        params.push(Box::new(offset));

        let param_refs: Vec<&dyn ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(|e| CitadelError::Database(e.to_string()))?;

        let rows = stmt
            .query_map(param_refs.as_slice(), |row| {
                let node = Self::row_to_node(row)?;
                let score: f64 = row.get("score")?;
                Ok(SearchResult {
                    node,
                    score,
                    highlights: None,
                })
            })
            .map_err(|e| CitadelError::Database(e.to_string()))?;

        let mut results = Vec::with_capacity(limit as usize);
        for row in rows {
            results.push(row.map_err(|e| CitadelError::Database(e.to_string()))?);
        }
        Ok(results)
    }

    fn filter_search(&self, conn: &Connection, options: &SearchOptions) -> Result<Vec<SearchResult>, CitadelError> {
        let limit = options.limit as i64;
        let offset = options.offset as i64;

        let mut sql = String::from("SELECT * FROM nodes WHERE 1=1");
        let mut params: Vec<Box<dyn ToSql>> = vec![];

        if let Some(kinds) = &options.kinds {
            let placeholders = vec!["?"; kinds.len()].join(",");
            sql.push_str(&format!(" AND kind IN ({})", placeholders));
            for kind in kinds {
                params.push(Box::new(kind.as_str().to_string()));
            }
        }

        if let Some(languages) = &options.languages {
            let placeholders = vec!["?"; languages.len()].join(",");
            sql.push_str(&format!(" AND language IN ({})", placeholders));
            for lang in languages {
                params.push(Box::new(lang.as_str().to_string()));
            }
        }

        sql.push_str(" ORDER BY name LIMIT ? OFFSET ?");
        params.push(Box::new(limit));
        params.push(Box::new(offset));

        let param_refs: Vec<&dyn ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(|e| CitadelError::Database(e.to_string()))?;

        let rows = stmt
            .query_map(param_refs.as_slice(), |row| {
                let node = Self::row_to_node(row)?;
                Ok(SearchResult {
                    node,
                    score: 1.0,
                    highlights: None,
                })
            })
            .map_err(|e| CitadelError::Database(e.to_string()))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| CitadelError::Database(e.to_string()))?);
        }
        Ok(results)
    }
}

fn parse_node_kind(s: &str) -> NodeKind {
    NodeKind::from_str(s).unwrap_or(NodeKind::Variable)
}

fn parse_edge_kind(s: &str) -> EdgeKind {
    EdgeKind::from_str(s).unwrap_or(EdgeKind::References)
}

fn parse_language(s: &str) -> Language {
    Language::from_str(s).unwrap_or(Language::Unknown)
}

impl BatchOps for SqliteStorage {
    fn begin_batch(&self) -> Result<(), CitadelError> {
        let mut guard = self.inner.lock().map_err(|e| CitadelError::Database(e.to_string()))?;
        if let Some(ref conn) = guard.conn {
            conn.execute_batch("BEGIN")?;
        }
        guard.in_batch = true;
        Ok(())
    }

    fn commit_batch(&self) -> Result<(), CitadelError> {
        let mut guard = self.inner.lock().map_err(|e| CitadelError::Database(e.to_string()))?;
        guard.in_batch = false;
        if let Some(ref conn) = guard.conn {
            conn.execute_batch("COMMIT")?;
        }
        Ok(())
    }

    fn rollback_batch(&self) -> Result<(), CitadelError> {
        let mut guard = self.inner.lock().map_err(|e| CitadelError::Database(e.to_string()))?;
        guard.in_batch = false;
        if let Some(ref conn) = guard.conn {
            conn.execute_batch("ROLLBACK")?;
        }
        Ok(())
    }
}

fn esc(sql: &mut String, s: &str) {
    for ch in s.chars() {
        if ch == '\'' { sql.push_str("''"); }
        else { sql.push(ch); }
    }
}

fn sql_opt_str(sql: &mut String, s: &Option<String>) {
    sql.push(',');
    match s {
        Some(v) => { sql.push('\''); esc(sql, v); sql.push('\''); }
        None => { sql.push_str("NULL"); }
    }
}

fn sql_opt_i32(sql: &mut String, v: Option<u32>) {
    sql.push(',');
    match v {
        Some(n) => { sql.push_str(&n.to_string()); }
        None => { sql.push_str("NULL"); }
    }
}

fn sql_bool(n: bool) -> &'static str {
    if n { "1" } else { "0" }
}

impl FullStore for SqliteStorage {
    fn bulk_store_extractions(
        &self,
        nodes: &[Node],
        edges: &[Edge],
        unresolved_refs: &[UnresolvedRef],
        files: &[FileRecord],
    ) -> Result<(u32, u32), CitadelError> {
        let guard = self.lock()?;
        let conn = guard.conn.as_ref().ok_or_else(|| CitadelError::NotInitialized("database not opened".into()))?;

        // Tune for bulk insert: large cache + no fsync during load
        conn.execute_batch("PRAGMA cache_size = -512000; PRAGMA synchronous = OFF;")
            .map_err(|e| CitadelError::Database(e.to_string()))?;

        let tx = conn.unchecked_transaction()?;
        use std::fmt::Write;

        let t0 = std::time::Instant::now();
        let mut t_nodes = t0.elapsed().as_millis() as u64;
        let mut t_edges = t_nodes;
        let mut t_refs = t_nodes;
        let t_files;

        // ── Nodes: multi-VALUES in batches of 500 ──
        if !nodes.is_empty() {
            let mut sql = String::with_capacity(128 * 1024);
            for chunk in nodes.chunks(500) {
                sql.clear();
                sql.push_str("INSERT OR REPLACE INTO nodes VALUES ");
                for (j, n) in chunk.iter().enumerate() {
                    if j > 0 { sql.push(','); }
                    sql.push('(');
                    sql.push('\''); esc(&mut sql, &n.id); sql.push('\'');
                    sql.push(','); sql.push('\''); esc(&mut sql, n.kind.as_str()); sql.push('\'');
                    sql.push(','); sql.push('\''); esc(&mut sql, &n.name); sql.push('\'');
                    sql.push(','); sql.push('\''); esc(&mut sql, &n.qualified_name); sql.push('\'');
                    sql.push(','); sql.push('\''); esc(&mut sql, &n.file_path); sql.push('\'');
                    sql.push(','); sql.push('\''); esc(&mut sql, n.language.as_str()); sql.push('\'');
                    write!(sql, ",{},{},{},{}", n.start_line, n.end_line, n.start_column, n.end_column).unwrap();
                    sql_opt_str(&mut sql, &n.docstring);
                    sql_opt_str(&mut sql, &n.signature);
                    sql_opt_str(&mut sql, &n.visibility);
                    write!(sql, ",{},{},{},{}",
                        sql_bool(n.is_exported), sql_bool(n.is_async),
                        sql_bool(n.is_static), sql_bool(n.is_abstract),
                    ).unwrap();
                    sql_opt_str(&mut sql, &n.decorators.as_ref().map(|d| serde_json::to_string(d).unwrap_or_default()));
                    sql_opt_str(&mut sql, &n.type_parameters.as_ref().map(|t| serde_json::to_string(t).unwrap_or_default()));
                    write!(sql, ",{}", n.updated_at).unwrap();
                    sql.push(')');
                }
                tx.execute_batch(&sql).map_err(|e| CitadelError::Database(e.to_string()))?;
            }
            t_nodes = t0.elapsed().as_millis() as u64;
        }

        // ── Edges: multi-VALUES ──
        if !edges.is_empty() {
            let mut sql = String::with_capacity(64 * 1024);
            for chunk in edges.chunks(500) {
                sql.clear();
                sql.push_str("INSERT OR IGNORE INTO edges(source,target,kind,metadata,line,col,provenance) VALUES ");
                for (j, e) in chunk.iter().enumerate() {
                    if j > 0 { sql.push(','); }
                    sql.push('(');
                    sql.push('\''); esc(&mut sql, &e.source); sql.push('\'');
                    sql.push(','); sql.push('\''); esc(&mut sql, &e.target); sql.push('\'');
                    sql.push(','); sql.push('\''); esc(&mut sql, e.kind.as_str()); sql.push('\'');
                    sql_opt_str(&mut sql, &e.metadata.as_ref().map(|m| m.to_string()));
                    sql_opt_i32(&mut sql, e.line);
                    sql_opt_i32(&mut sql, e.column);
                    sql.push(',');
                    match &e.provenance {
                        Some(p) => { sql.push('\''); esc(&mut sql, p); sql.push('\''); }
                        None => { sql.push_str("NULL"); }
                    }
                    sql.push(')');
                }
                tx.execute_batch(&sql).map_err(|e| CitadelError::Database(e.to_string()))?;
            }
            t_edges = t0.elapsed().as_millis() as u64;
        }

        // ── Unresolved refs: multi-VALUES ──
        if !unresolved_refs.is_empty() {
            let mut sql = String::with_capacity(32 * 1024);
            for chunk in unresolved_refs.chunks(500) {
                sql.clear();
                sql.push_str("INSERT INTO unresolved_refs(from_node_id,reference_name,reference_kind,line,col,candidates,file_path,language) VALUES ");
                for (j, r) in chunk.iter().enumerate() {
                    if j > 0 { sql.push(','); }
                    sql.push('(');
                    sql.push('\''); esc(&mut sql, &r.from_node_id); sql.push('\'');
                    sql.push(','); sql.push('\''); esc(&mut sql, &r.reference_name); sql.push('\'');
                    sql.push(','); sql.push('\''); esc(&mut sql, &r.reference_kind); sql.push('\'');
                    sql_opt_i32(&mut sql, r.line);
                    sql_opt_i32(&mut sql, r.column);
                    sql_opt_str(&mut sql, &r.candidates.as_ref().map(|c| c.to_string()));
                    sql.push(','); sql.push('\''); esc(&mut sql, &r.file_path); sql.push('\'');
                    sql.push(','); sql.push('\''); esc(&mut sql, &r.language); sql.push('\'');
                    sql.push(')');
                }
                tx.execute_batch(&sql).map_err(|e| CitadelError::Database(e.to_string()))?;
            }
            t_refs = t0.elapsed().as_millis() as u64;
        }

        // ── Files: multi-VALUES ──
        if !files.is_empty() {
            let mut sql = String::with_capacity(32 * 1024);
            for chunk in files.chunks(500) {
                sql.clear();
                sql.push_str("INSERT INTO files VALUES ");
                for (j, f) in chunk.iter().enumerate() {
                    if j > 0 { sql.push(','); }
                    sql.push('(');
                    sql.push('\''); esc(&mut sql, &f.path); sql.push('\'');
                    sql.push(','); sql.push('\''); esc(&mut sql, &f.content_hash); sql.push('\'');
                    sql.push(','); sql.push('\''); esc(&mut sql, f.language.as_str()); sql.push('\'');
                    write!(sql, ",{},{},{},{}", f.size, f.modified_at, f.indexed_at, f.node_count).unwrap();
                    sql_opt_str(&mut sql, &f.errors.as_ref().map(|e| serde_json::to_string(e).unwrap_or_default()));
                    sql.push(')');
                }
                sql.push_str(" ON CONFLICT(path) DO UPDATE SET content_hash=excluded.content_hash,language=excluded.language,size=excluded.size,modified_at=excluded.modified_at,indexed_at=excluded.indexed_at,node_count=excluded.node_count,errors=excluded.errors");
                tx.execute_batch(&sql).map_err(|e| CitadelError::Database(e.to_string()))?;
            }
        }

        t_files = t0.elapsed().as_millis() as u64;
        tx.commit()?;

        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).create(true).open("/tmp/cg-profile.log") {
            use std::io::Write;
            let _ = writeln!(f, "DB-PROFILE: nodes={}ms edges={}ms refs={}ms files={}ms ({} nodes, {} edges, {} refs, {} files) nchunks={} echunks={}",
                t_nodes, t_edges - t_nodes, t_refs - t_edges, t_files - t_refs,
                nodes.len(), edges.len(), unresolved_refs.len(), files.len(),
                (nodes.len() + 499) / 500, (edges.len() + 499) / 500);
        }

        // Restore safe PRAGMAs after bulk insert
        conn.execute_batch("PRAGMA cache_size = -64000; PRAGMA synchronous = NORMAL;")
            .map_err(|e| CitadelError::Database(e.to_string()))?;

        Ok((nodes.len() as u32, edges.len() as u32))
    }

    fn batch_store_file_extractions(
        &self,
        paths_and_hashes: &[(&str, &str)],
        languages: &[&str],
        results: &[extraction::ExtractionResult],
        root_dir: &str,
    ) -> Result<(u32, u32, u32, Vec<String>), CitadelError> {
        let guard = self.lock()?;
        let conn = guard.conn.as_ref().ok_or_else(|| CitadelError::NotInitialized("database not opened".into()))?;

        let t0 = std::time::Instant::now();

        // Tune for bulk insert
        conn.execute_batch("PRAGMA cache_size = -512000; PRAGMA synchronous = OFF;")
            .map_err(|e| CitadelError::Database(e.to_string()))?;

        let tx = conn.unchecked_transaction()?;

        // Pre-allocate flat collections
        let est_nodes: usize = results.iter().map(|r| r.nodes.len()).sum();
        let est_edges: usize = results.iter().map(|r| r.edges.len()).sum();
        let mut all_nodes: Vec<&Node> = Vec::with_capacity(est_nodes);
        let mut all_edges: Vec<&Edge> = Vec::with_capacity(est_edges);
        let mut all_refs: Vec<&UnresolvedRef> = Vec::new();
        let mut all_files: Vec<FileRecord> = Vec::with_capacity(results.len());
        let mut files_skipped: u32 = 0;
        let mut files_indexed: u32 = 0;
        let errors: Vec<String> = Vec::new();

        for (i, &(path, content_hash)) in paths_and_hashes.iter().enumerate() {
            let result = &results[i];
            let language = languages[i];

            // Hash check (skip unchanged)
            let existing: Option<String> = tx.query_row(
                "SELECT content_hash FROM files WHERE path = ?1",
                params![path], |row| row.get(0)
            ).optional().map_err(|e| CitadelError::Database(e.to_string()))?;
            if existing.as_deref() == Some(content_hash) {
                files_skipped += 1;
                continue;
            }

            // Delete old data
            tx.execute("DELETE FROM nodes WHERE file_path = ?1", params![path])
                .map_err(|e| CitadelError::Database(e.to_string()))?;
            tx.execute("DELETE FROM files WHERE path = ?1", params![path])
                .map_err(|e| CitadelError::Database(e.to_string()))?;

            let valid_nodes: Vec<&Node> = result.nodes.iter()
                .filter(|n| !n.id.is_empty() && !n.name.is_empty() && !n.file_path.is_empty())
                .collect();

            if valid_nodes.is_empty() {
                let lang = Language::from_str(language).unwrap_or(Language::Unknown);
                let now = crate::util::now_ts();
                all_files.push(FileRecord {
                    path: path.to_string(), content_hash: content_hash.to_string(),
                    language: lang, size: 0, modified_at: 0, indexed_at: now,
                    node_count: 0,
                    errors: if result.errors.is_empty() { None } else { Some(result.errors.clone()) },
                });
                files_skipped += 1;
                continue;
            }

            let valid_ids: HashSet<&str> = valid_nodes.iter().map(|n| n.id.as_str()).collect();

            all_nodes.extend(valid_nodes);

            all_edges.extend(result.edges.iter()
                .filter(|e| valid_ids.contains(e.source.as_str()) && valid_ids.contains(e.target.as_str())));

            all_refs.extend(result.unresolved_references.iter()
                .filter(|r| valid_ids.contains(r.from_node_id.as_str())));

            // File metadata
            let lang = Language::from_str(language).unwrap_or(Language::Unknown);
            let full_path = format!("{}/{}", root_dir.trim_end_matches('/'), path);
            let (size, modified_at) = match std::fs::metadata(&full_path) {
                Ok(meta) => {
                    let mtime = meta.modified().ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as i64).unwrap_or(0);
                    (meta.len() as u64, mtime)
                }
                Err(_) => (0, 0),
            };
            let now = crate::util::now_ts();
            all_files.push(FileRecord {
                path: path.to_string(), content_hash: content_hash.to_string(),
                language: lang, size, modified_at, indexed_at: now,
                node_count: result.nodes.len() as u32,
                errors: result.errors.clone().into(),
            });

            files_indexed += 1;
        }

        let t_flatten = t0.elapsed().as_micros() as u64;
        let RawDb(db) = guard.db_ptr.ok_or_else(|| CitadelError::NotInitialized("db not opened".into()))?;

        let mut t_node_exec: u64 = 0;
        let mut t_edge_exec: u64 = 0;
        let mut t_ref_exec: u64 = 0;
        let mut t_file_exec: u64 = 0;
        let mut t_reindex: u64 = 0;

        // ── Drop secondary indexes before bulk insert ──

        if !all_nodes.is_empty() {
            ffi::drop_node_indexes(db)?;
            let tn = std::time::Instant::now();
            ffi::insert_nodes_multi_values(db, &all_nodes)?;
            t_node_exec = tn.elapsed().as_micros() as u64;
        }

        if !all_edges.is_empty() {
            let te = std::time::Instant::now();
            ffi::insert_edges_prepared(db, &all_edges)?;
            t_edge_exec = te.elapsed().as_micros() as u64;
        }

        if !all_refs.is_empty() {
            let tr = std::time::Instant::now();
            ffi::insert_refs_prepared(db, &all_refs)?;
            t_ref_exec = tr.elapsed().as_micros() as u64;
        }

        if !all_files.is_empty() {
            let tf = std::time::Instant::now();
            ffi::insert_files_prepared(db, &all_files)?;
            t_file_exec = tf.elapsed().as_micros() as u64;
        }

        // Recreate secondary indexes
        if !all_nodes.is_empty() {
            let tr = std::time::Instant::now();
            ffi::create_node_indexes(db)?;
            t_reindex = tr.elapsed().as_micros() as u64;
        }

        let t_commit = std::time::Instant::now();
        tx.commit()?;
        let t_commit_us = t_commit.elapsed().as_micros() as u64;

        // Restore safe PRAGMAs
        conn.execute_batch("PRAGMA cache_size = -64000; PRAGMA synchronous = NORMAL;")
            .map_err(|e| CitadelError::Database(e.to_string()))?;

        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).create(true).open("/tmp/cg-store-profile.log") {
            use std::io::Write;
            let _ = write!(f, "{} nodes, {} edges, {} refs, {} files | ",
                all_nodes.len(), all_edges.len(), all_refs.len(), all_files.len());
            let _ = writeln!(f, concat!("flatten={}ms ",
                "node_exec={}ms edge_exec={}ms ",
                "ref_exec={}ms file_exec={}ms ",
                "reindex={}ms commit={}ms"),
                t_flatten / 1000,
                t_node_exec / 1000, t_edge_exec / 1000,
                t_ref_exec / 1000, t_file_exec / 1000,
                t_reindex / 1000, t_commit_us / 1000);
        }

        Ok((files_indexed, 0, files_skipped, errors))
    }

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
        self.with(|conn| {
            let tx = conn.unchecked_transaction()?;

            // Skip if content hash matches cached data
            let existing: Option<String> = tx
                .query_row(
                    "SELECT content_hash FROM files WHERE path = ?1",
                    params![path],
                    |row| row.get(0),
                )
                .optional()?;
            if existing.as_deref() == Some(content_hash) {
                return Ok(false);
            }

            // Delete old data for this file
            tx.execute("DELETE FROM nodes WHERE file_path = ?1", params![path])?;
            tx.execute("DELETE FROM files WHERE path = ?1", params![path])?;

            // Filter nodes with valid IDs
            let valid_nodes: Vec<Node> = nodes.iter()
                .filter(|n| !n.id.is_empty() && !n.name.is_empty() && !n.file_path.is_empty())
                .cloned()
                .collect();

            if !valid_nodes.is_empty() {
                let valid_ids: HashSet<&str> = valid_nodes.iter().map(|n| n.id.as_str()).collect();

                {
                    let mut node_stmt = tx.prepare(
                        "INSERT OR REPLACE INTO nodes (
                            id, kind, name, qualified_name, file_path, language,
                            start_line, end_line, start_column, end_column,
                            docstring, signature, visibility,
                            is_exported, is_async, is_static, is_abstract,
                            decorators, type_parameters, updated_at
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                                  ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)"
                    )?;

                    for node in &valid_nodes {
                        node_stmt.execute(params![
                            node.id, node.kind.as_str(), node.name, node.qualified_name,
                            node.file_path, node.language.as_str(),
                            node.start_line, node.end_line, node.start_column, node.end_column,
                            node.docstring, node.signature, node.visibility,
                            node.is_exported as i32, node.is_async as i32,
                            node.is_static as i32, node.is_abstract as i32,
                            node.decorators.as_ref().map(|d| serde_json::to_string(d).unwrap_or_default()),
                            node.type_parameters.as_ref().map(|t| serde_json::to_string(t).unwrap_or_default()),
                            node.updated_at,
                        ])?;
                    }
                }

                {
                    let mut edge_stmt = tx.prepare(
                        "INSERT OR IGNORE INTO edges (source, target, kind, metadata, line, col, provenance)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
                    )?;

                    for edge in edges.iter().filter(|e| valid_ids.contains(e.source.as_str()) && valid_ids.contains(e.target.as_str())) {
                        edge_stmt.execute(params![
                            edge.source, edge.target, edge.kind.as_str(),
                            edge.metadata.as_ref().map(|m| m.to_string()),
                            edge.line, edge.column, edge.provenance,
                        ])?;
                    }
                }

                {
                    let mut ref_stmt = tx.prepare(
                        "INSERT INTO unresolved_refs (from_node_id, reference_name, reference_kind, line, col, candidates, file_path, language)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
                    )?;

                    for r in unresolved_refs.iter().filter(|r| valid_ids.contains(r.from_node_id.as_str())) {
                        ref_stmt.execute(params![
                            r.from_node_id, r.reference_name, r.reference_kind,
                            r.line, r.column,
                            r.candidates.as_ref().map(|c| c.to_string()),
                            r.file_path, r.language,
                        ])?;
                    }
                }
            }

            // File metadata
            let lang = parse_language(language);
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
                    (meta.len() as u64, mtime)
                }
                Err(_) => (0, 0),
            };
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);

            {
                let mut file_stmt = tx.prepare(
                    "INSERT INTO files (path, content_hash, language, size, modified_at, indexed_at, node_count, errors)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                     ON CONFLICT(path) DO UPDATE SET
                        content_hash=excluded.content_hash, language=excluded.language,
                        size=excluded.size, modified_at=excluded.modified_at,
                        indexed_at=excluded.indexed_at, node_count=excluded.node_count,
                        errors=excluded.errors"
                )?;

                file_stmt.execute(params![
                    path, content_hash, lang.as_str(),
                    size, modified_at, now, valid_nodes.len() as u32,
                    if extraction_errors.is_empty() { None } else { Some(serde_json::to_string(extraction_errors).unwrap_or_default()) },
                ])?;
            }

            tx.commit()?;
            Ok(true)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize_and_insert_node() {
        let dir = std::env::temp_dir().join(format!("cg_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let db_path = dir.join("test.db");
        let db_path_str = db_path.to_str().unwrap();

        let mut storage = SqliteStorage::new();
        storage.initialize(db_path_str).expect("should initialize");

        let node = Node {
            id: "test1".into(),
            kind: NodeKind::Function,
            name: "hello".into(),
            qualified_name: "hello".into(),
            file_path: "main.ts".into(),
            language: Language::TypeScript,
            start_line: 1,
            end_line: 5,
            start_column: 0,
            end_column: 0,
            docstring: None,
            signature: Some("fn hello()".into()),
            visibility: None,
            is_exported: true,
            is_async: false,
            is_static: false,
            is_abstract: false,
            decorators: None,
            type_parameters: None,
            updated_at: 12345,
        };

        storage.insert_node(&node).expect("should insert");
        let found = storage.get_node_by_id("test1").expect("should find");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "hello");

        let node2 = Node {
            id: "test2".into(),
            kind: NodeKind::Function,
            name: "world".into(),
            qualified_name: "world".into(),
            file_path: "main.ts".into(),
            language: Language::TypeScript,
            start_line: 10,
            end_line: 15,
            start_column: 0,
            end_column: 0,
            docstring: None,
            signature: Some("fn world()".into()),
            visibility: None,
            is_exported: false,
            is_async: false,
            is_static: false,
            is_abstract: false,
            decorators: None,
            type_parameters: None,
            updated_at: 12345,
        };
        storage.insert_node(&node2).expect("should insert node2");

        let edge = Edge {
            source: "test1".into(),
            target: "test2".into(),
            kind: EdgeKind::Calls,
            metadata: None,
            line: None,
            column: None,
            provenance: None,
        };
        storage.insert_edge(&edge).expect("should insert edge");

        let file = FileRecord {
            path: "main.ts".into(),
            content_hash: "abc123".into(),
            language: Language::TypeScript,
            size: 100,
            modified_at: 12345,
            indexed_at: 12345,
            node_count: 1,
            errors: None,
        };
        storage.upsert_file(&file).expect("should upsert file");

        let stats = storage.get_stats().expect("should get stats");
        assert_eq!(stats.node_count, 2);
        assert_eq!(stats.edge_count, 1);
        assert_eq!(stats.file_count, 1);

        storage.set_metadata("key1", "val1").expect("should set metadata");
        let val = storage.get_metadata("key1").expect("should get metadata");
        assert_eq!(val, Some("val1".into()));

        storage.close().expect("should close");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_sqlite_contract() {
        crate::storage::contract_tests::run_all(&|| Box::new(SqliteStorage::new()));
    }
}
