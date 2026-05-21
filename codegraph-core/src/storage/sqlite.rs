use crate::storage::error::StorageError;
use crate::types::*;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use rusqlite::types::ToSql;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use super::Storage;

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

struct StorageInner {
    conn: Option<Connection>,
    path: Option<String>,
}

pub struct SqliteStorage {
    inner: Mutex<StorageInner>,
}

impl SqliteStorage {
    pub fn new() -> Self {
        SqliteStorage {
            inner: Mutex::new(StorageInner { conn: None, path: None }),
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, StorageInner>, StorageError> {
        self.inner.lock().map_err(|e| StorageError::Database(e.to_string()))
    }

    fn with<F, T>(&self, f: F) -> Result<T, StorageError>
    where
        F: FnOnce(&Connection) -> Result<T, StorageError>,
    {
        let guard = self.lock()?;
        let conn = guard.conn.as_ref().ok_or_else(|| StorageError::NotInitialized("database not opened".into()))?;
        f(conn)
    }

    fn apply_pragmas(conn: &Connection, _is_wal_allowed: bool) -> Result<(), StorageError> {
        for (pragma, _is_wal) in PRAGMAS {
            conn.execute_batch(pragma)?;
        }
        Ok(())
    }

    fn get_schema_version(conn: &Connection) -> Result<i32, StorageError> {
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='schema_versions'",
                [],
                |row| row.get(0),
            )
            .map_err(|e: rusqlite::Error| StorageError::Database(e.to_string()))?;

        if count == 0 {
            return Ok(0);
        }

        conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_versions",
            [],
            |row| row.get(0),
        )
        .map_err(|e| StorageError::Database(e.to_string()))
    }

    fn run_schema(conn: &Connection) -> Result<(), StorageError> {
        conn.execute_batch(SCHEMA_SQL)
            .map_err(|e| StorageError::Database(e.to_string()))?;
        conn.execute(
            "UPDATE schema_versions SET version = 4, description = 'schema (current)' WHERE version = 1",
            [],
        )
        .map_err(|e| StorageError::Database(e.to_string()))?;
        Ok(())
    }

    fn run_migrations(conn: &Connection, from_version: i32) -> Result<(), StorageError> {
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
            .map_err(|e| StorageError::Migration(e.to_string()))?;
            conn.execute(
                "INSERT INTO schema_versions (version, description) VALUES (2, 'add project_metadata + provenance')",
                [],
            )
            .map_err(|e| StorageError::Migration(e.to_string()))?;
        }
        if from_version < 3 {
            conn.execute_batch(
                "CREATE INDEX IF NOT EXISTS idx_nodes_lower_name ON nodes(lower(name));",
            )
            .map_err(|e| StorageError::Migration(e.to_string()))?;
            conn.execute(
                "INSERT INTO schema_versions (version, description) VALUES (3, 'add lower(name) index')",
                [],
            )
            .map_err(|e| StorageError::Migration(e.to_string()))?;
        }
        if from_version < 4 {
            conn.execute_batch(
                "DROP INDEX IF EXISTS idx_edges_source;
                 DROP INDEX IF EXISTS idx_edges_target;",
            )
            .map_err(|e| StorageError::Migration(e.to_string()))?;
            conn.execute(
                "INSERT INTO schema_versions (version, description) VALUES (4, 'drop redundant edge indexes')",
                [],
            )
            .map_err(|e| StorageError::Migration(e.to_string()))?;
        }
        Ok(())
    }

    fn insert_node_tx(tx: &Transaction, node: &Node) -> Result<(), StorageError> {
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
        .map_err(|e| StorageError::Database(e.to_string()))?;
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

impl Storage for SqliteStorage {
    fn initialize(&mut self, db_path: &str) -> Result<(), StorageError> {
        if let Some(parent) = Path::new(db_path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        Self::apply_pragmas(&conn, true)?;
        Self::run_schema(&conn)?;
        let mut guard = self.inner.lock().map_err(|e| StorageError::Database(e.to_string()))?;
        guard.conn = Some(conn);
        guard.path = Some(db_path.to_string());
        Ok(())
    }

    fn open(&mut self, db_path: &str) -> Result<(), StorageError> {
        if !Path::new(db_path).exists() {
            return Err(StorageError::NotFound(format!("database file not found: {}", db_path)));
        }
        let conn = Connection::open(db_path)?;
        Self::apply_pragmas(&conn, true)?;
        let current_version = Self::get_schema_version(&conn)?;
        if current_version > 0 && current_version < 4 {
            Self::run_migrations(&conn, current_version)?;
        }
        let mut guard = self.inner.lock().map_err(|e| StorageError::Database(e.to_string()))?;
        guard.conn = Some(conn);
        guard.path = Some(db_path.to_string());
        Ok(())
    }

    fn close(&mut self) -> Result<(), StorageError> {
        let mut guard = self.inner.lock().map_err(|e| StorageError::Database(e.to_string()))?;
        guard.conn = None;
        guard.path = None;
        Ok(())
    }

    fn get_path(&self) -> Option<String> {
        self.lock().ok().and_then(|g| g.path.clone())
    }

    // ---- Nodes ----

    fn insert_node(&self, node: &Node) -> Result<(), StorageError> {
        self.with(|conn| {
            let tx = conn.unchecked_transaction()?;
            Self::insert_node_tx(&tx, node)?;
            tx.commit()?;
            Ok(())
        })
    }

    fn insert_nodes(&self, nodes: &[Node]) -> Result<(), StorageError> {
        self.with(|conn| {
            let tx = conn.unchecked_transaction()?;
            for node in nodes {
                Self::insert_node_tx(&tx, node)?;
            }
            tx.commit()?;
            Ok(())
        })
    }

    fn update_node(&self, node: &Node) -> Result<(), StorageError> {
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

    fn delete_node(&self, id: &str) -> Result<(), StorageError> {
        self.with(|conn| {
            conn.execute("DELETE FROM nodes WHERE id = ?1", params![id])?;
            Ok(())
        })
    }

    fn delete_nodes_by_file(&self, file_path: &str) -> Result<(), StorageError> {
        self.with(|conn| {
            conn.execute("DELETE FROM nodes WHERE file_path = ?1", params![file_path])?;
            Ok(())
        })
    }

    fn get_node_by_id(&self, id: &str) -> Result<Option<Node>, StorageError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM nodes WHERE id = ?1")?;
            let result = stmt
                .query_row(params![id], |row| Self::row_to_node(row))
                .optional()?;
            Ok(result)
        })
    }

    fn get_nodes_by_file(&self, file_path: &str) -> Result<Vec<Node>, StorageError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM nodes WHERE file_path = ?1 ORDER BY start_line")?;
            let rows = stmt.query_map(params![file_path], |row| Self::row_to_node(row))?;
            let mut nodes = Vec::new();
            for row in rows {
                nodes.push(row?);
            }
            Ok(nodes)
        })
    }

    fn get_nodes_by_kind(&self, kind: &NodeKind) -> Result<Vec<Node>, StorageError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM nodes WHERE kind = ?1")?;
            let rows = stmt.query_map(params![kind.as_str()], |row| Self::row_to_node(row))?;
            let mut nodes = Vec::new();
            for row in rows {
                nodes.push(row?);
            }
            Ok(nodes)
        })
    }

    fn get_all_nodes(&self) -> Result<Vec<Node>, StorageError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM nodes")?;
            let rows = stmt.query_map([], |row| Self::row_to_node(row))?;
            let mut nodes = Vec::new();
            for row in rows {
                nodes.push(row?);
            }
            Ok(nodes)
        })
    }

    fn get_nodes_by_name(&self, name: &str) -> Result<Vec<Node>, StorageError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM nodes WHERE name = ?1")?;
            let rows = stmt.query_map(params![name], |row| Self::row_to_node(row))?;
            let mut nodes = Vec::new();
            for row in rows {
                nodes.push(row?);
            }
            Ok(nodes)
        })
    }

    fn get_nodes_by_qualified_name(&self, qn: &str) -> Result<Vec<Node>, StorageError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM nodes WHERE qualified_name = ?1")?;
            let rows = stmt.query_map(params![qn], |row| Self::row_to_node(row))?;
            let mut nodes = Vec::new();
            for row in rows {
                nodes.push(row?);
            }
            Ok(nodes)
        })
    }

    fn get_nodes_by_lower_name(&self, name: &str) -> Result<Vec<Node>, StorageError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM nodes WHERE lower(name) = lower(?1)")?;
            let rows = stmt.query_map(params![name], |row| Self::row_to_node(row))?;
            let mut nodes = Vec::new();
            for row in rows {
                nodes.push(row?);
            }
            Ok(nodes)
        })
    }

    // ---- Search ----

    fn search_nodes(&self, query: &str, options: &SearchOptions) -> Result<Vec<SearchResult>, StorageError> {
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

    // ---- Edges ----

    fn insert_edge(&self, edge: &Edge) -> Result<(), StorageError> {
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

    fn insert_edges(&self, edges: &[Edge]) -> Result<(), StorageError> {
        self.with(|conn| {
            let tx = conn.unchecked_transaction()?;
            for edge in edges {
                tx.execute(
                    "INSERT OR IGNORE INTO edges (source, target, kind, metadata, line, col, provenance)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        edge.source, edge.target, edge.kind.as_str(),
                        edge.metadata.as_ref().map(|m| m.to_string()),
                        edge.line, edge.column, edge.provenance,
                    ],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
    }

    fn delete_edges_by_source(&self, source_id: &str) -> Result<(), StorageError> {
        self.with(|conn| {
            conn.execute("DELETE FROM edges WHERE source = ?1", params![source_id])?;
            Ok(())
        })
    }

    fn get_outgoing_edges(&self, source_id: &str, kinds: Option<&[EdgeKind]>, provenance: Option<&str>) -> Result<Vec<Edge>, StorageError> {
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
            let rows = stmt.query_map(param_refs.as_slice(), |row| Self::row_to_edge(row))?;
            let mut edges = Vec::new();
            for row in rows {
                edges.push(row?);
            }
            Ok(edges)
        })
    }

    fn get_incoming_edges(&self, target_id: &str, kinds: Option<&[EdgeKind]>) -> Result<Vec<Edge>, StorageError> {
        self.with(|conn| {
            let mut sql = String::from("SELECT * FROM edges WHERE target = ?");
            let mut param_values: Vec<Box<dyn ToSql>> = vec![Box::new(target_id.to_string())];

            if let Some(k) = kinds.and_then(|k| k.first().map(|k| k.as_str())) {
                sql.push_str(" AND kind = ?");
                param_values.push(Box::new(k.to_string()));
            }

            let param_refs: Vec<&dyn ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(param_refs.as_slice(), |row| Self::row_to_edge(row))?;
            let mut edges = Vec::new();
            for row in rows {
                edges.push(row?);
            }
            Ok(edges)
        })
    }

    fn find_edges_between_nodes(&self, node_ids: &[String], kinds: Option<&[EdgeKind]>) -> Result<Vec<Edge>, StorageError> {
        self.with(|conn| {
            let ids_json = serde_json::to_string(node_ids)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;

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
            let rows = stmt.query_map(param_refs.as_slice(), |row| Self::row_to_edge(row))?;
            let mut edges = Vec::new();
            for row in rows {
                edges.push(row?);
            }
            Ok(edges)
        })
    }

    // ---- Files ----

    fn upsert_file(&self, file: &FileRecord) -> Result<(), StorageError> {
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

    fn delete_file(&self, path: &str) -> Result<(), StorageError> {
        self.with(|conn| {
            let tx = conn.unchecked_transaction()?;
            tx.execute("DELETE FROM nodes WHERE file_path = ?1", params![path])?;
            tx.execute("DELETE FROM files WHERE path = ?1", params![path])?;
            tx.commit()?;
            Ok(())
        })
    }

    fn get_file_by_path(&self, path: &str) -> Result<Option<FileRecord>, StorageError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM files WHERE path = ?1")?;
            let result = stmt
                .query_row(params![path], |row| Self::row_to_file(row))
                .optional()?;
            Ok(result)
        })
    }

    fn get_all_files(&self) -> Result<Vec<FileRecord>, StorageError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM files ORDER BY path")?;
            let rows = stmt.query_map([], |row| Self::row_to_file(row))?;
            let mut files = Vec::new();
            for row in rows {
                files.push(row?);
            }
            Ok(files)
        })
    }

    fn get_stale_files(&self, current_hashes: &HashMap<String, String>) -> Result<Vec<FileRecord>, StorageError> {
        let all = self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM files ORDER BY path")?;
            let rows = stmt.query_map([], |row| Self::row_to_file(row))?;
            let mut files = Vec::new();
            for row in rows {
                files.push(row?);
            }
            Ok(files)
        })?;
        Ok(all
            .into_iter()
            .filter(|f| current_hashes.get(&f.path).map_or(true, |h| *h != f.content_hash))
            .collect())
    }

    fn get_all_file_paths(&self) -> Result<Vec<String>, StorageError> {
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

    fn get_all_node_names(&self) -> Result<Vec<String>, StorageError> {
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

    // ---- Unresolved refs ----

    fn insert_unresolved_ref(&self, r#ref: &UnresolvedRef) -> Result<(), StorageError> {
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

    fn insert_unresolved_refs_batch(&self, refs: &[UnresolvedRef]) -> Result<(), StorageError> {
        self.with(|conn| {
            let tx = conn.unchecked_transaction()?;
            for r in refs {
                tx.execute(
                    "INSERT INTO unresolved_refs (from_node_id, reference_name, reference_kind, line, col, candidates, file_path, language)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        r.from_node_id, r.reference_name, r.reference_kind,
                        r.line, r.column,
                        r.candidates.as_ref().map(|c| c.to_string()),
                        r.file_path, r.language,
                    ],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
    }

    fn delete_unresolved_by_node(&self, node_id: &str) -> Result<(), StorageError> {
        self.with(|conn| {
            conn.execute("DELETE FROM unresolved_refs WHERE from_node_id = ?1", params![node_id])?;
            Ok(())
        })
    }

    fn get_unresolved_by_name(&self, name: &str) -> Result<Vec<UnresolvedRef>, StorageError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM unresolved_refs WHERE reference_name = ?1")?;
            let rows = stmt.query_map(params![name], |row| Self::row_to_unresolved_ref(row))?;
            let mut refs = Vec::new();
            for row in rows {
                refs.push(row?);
            }
            Ok(refs)
        })
    }

    fn get_all_unresolved_refs(&self) -> Result<Vec<UnresolvedRef>, StorageError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM unresolved_refs")?;
            let rows = stmt.query_map([], |row| Self::row_to_unresolved_ref(row))?;
            let mut refs = Vec::new();
            for row in rows {
                refs.push(row?);
            }
            Ok(refs)
        })
    }

    fn get_unresolved_refs_count(&self) -> Result<u64, StorageError> {
        self.with(|conn| {
            let count: u64 = conn.query_row("SELECT COUNT(*) FROM unresolved_refs", [], |row| row.get(0))?;
            Ok(count)
        })
    }

    fn get_unresolved_refs_batch(&self, offset: u64, limit: u64) -> Result<Vec<UnresolvedRef>, StorageError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM unresolved_refs LIMIT ?1 OFFSET ?2")?;
            let rows = stmt.query_map(params![limit, offset], |row| Self::row_to_unresolved_ref(row))?;
            let mut refs = Vec::new();
            for row in rows {
                refs.push(row?);
            }
            Ok(refs)
        })
    }

    fn get_unresolved_refs_by_files(&self, file_paths: &[String]) -> Result<Vec<UnresolvedRef>, StorageError> {
        self.with(|conn| {
            let paths_json = serde_json::to_string(file_paths)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;
            let mut stmt = conn.prepare(
                "SELECT * FROM unresolved_refs WHERE file_path IN (SELECT value FROM json_each(?1))"
            )?;
            let rows = stmt.query_map(params![paths_json], |row| Self::row_to_unresolved_ref(row))?;
            let mut refs = Vec::new();
            for row in rows {
                refs.push(row?);
            }
            Ok(refs)
        })
    }

    fn clear_unresolved_refs(&self) -> Result<(), StorageError> {
        self.with(|conn| {
            conn.execute("DELETE FROM unresolved_refs", [])?;
            Ok(())
        })
    }

    fn delete_resolved_refs(&self, from_node_ids: &[String]) -> Result<(), StorageError> {
        self.with(|conn| {
            let ids_json = serde_json::to_string(from_node_ids)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;
            conn.execute(
                "DELETE FROM unresolved_refs WHERE from_node_id IN (SELECT value FROM json_each(?1))",
                params![ids_json],
            )?;
            Ok(())
        })
    }

    fn delete_specific_resolved_refs(&self, refs: &[UnresolvedRef]) -> Result<(), StorageError> {
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

    // ---- Stats & Metadata ----

    fn get_stats(&self) -> Result<GraphStats, StorageError> {
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

    fn get_metadata(&self, key: &str) -> Result<Option<String>, StorageError> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT value FROM project_metadata WHERE key = ?1")?;
            let result = stmt.query_row(params![key], |row| row.get(0)).optional()?;
            Ok(result)
        })
    }

    fn set_metadata(&self, key: &str, value: &str) -> Result<(), StorageError> {
        self.with(|conn| {
            conn.execute(
                "INSERT INTO project_metadata (key, value, updated_at) VALUES (?1, ?2, strftime('%s','now'))
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=excluded.updated_at",
                params![key, value],
            )?;
            Ok(())
        })
    }

    fn get_all_metadata(&self) -> Result<HashMap<String, String>, StorageError> {
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

    fn clear(&self) -> Result<(), StorageError> {
        self.with(|conn| {
            conn.execute_batch(
                "DELETE FROM nodes; DELETE FROM edges; DELETE FROM files; DELETE FROM unresolved_refs;",
            )?;
            Ok(())
        })
    }
}

impl SqliteStorage {
    fn count_map(conn: &Connection, sql: &str) -> Result<HashMap<String, u64>, StorageError> {
        let mut stmt = conn.prepare(sql).map_err(|e| StorageError::Database(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
            })
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let mut map = HashMap::new();
        for row in rows {
            let (k, v) = row.map_err(|e| StorageError::Database(e.to_string()))?;
            map.insert(k, v);
        }
        Ok(map)
    }

    fn fts5_search(&self, conn: &Connection, query: &str, options: &SearchOptions) -> Result<Vec<SearchResult>, StorageError> {
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
        let mut stmt = conn.prepare(&sql).map_err(|e| StorageError::Database(e.to_string()))?;

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
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| StorageError::Database(e.to_string()))?);
        }
        Ok(results)
    }

    fn like_search(&self, conn: &Connection, query: &str, options: &SearchOptions) -> Result<Vec<SearchResult>, StorageError> {
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
        let mut stmt = conn.prepare(&sql).map_err(|e| StorageError::Database(e.to_string()))?;

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
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| StorageError::Database(e.to_string()))?);
        }
        Ok(results)
    }

    fn filter_search(&self, conn: &Connection, options: &SearchOptions) -> Result<Vec<SearchResult>, StorageError> {
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
        let mut stmt = conn.prepare(&sql).map_err(|e| StorageError::Database(e.to_string()))?;

        let rows = stmt
            .query_map(param_refs.as_slice(), |row| {
                let node = Self::row_to_node(row)?;
                Ok(SearchResult {
                    node,
                    score: 1.0,
                    highlights: None,
                })
            })
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| StorageError::Database(e.to_string()))?);
        }
        Ok(results)
    }
}

fn parse_node_kind(s: &str) -> NodeKind {
    match s {
        "file" => NodeKind::File,
        "module" => NodeKind::Module,
        "class" => NodeKind::Class,
        "struct" => NodeKind::Struct,
        "interface" => NodeKind::Interface,
        "trait" => NodeKind::Trait,
        "protocol" => NodeKind::Protocol,
        "function" => NodeKind::Function,
        "method" => NodeKind::Method,
        "property" => NodeKind::Property,
        "field" => NodeKind::Field,
        "variable" => NodeKind::Variable,
        "constant" => NodeKind::Constant,
        "enum" => NodeKind::Enum,
        "enum_member" => NodeKind::EnumMember,
        "type_alias" => NodeKind::TypeAlias,
        "namespace" => NodeKind::Namespace,
        "parameter" => NodeKind::Parameter,
        "import" => NodeKind::Import,
        "export" => NodeKind::Export,
        "route" => NodeKind::Route,
        "component" => NodeKind::Component,
        _ => NodeKind::Variable,
    }
}

fn parse_edge_kind(s: &str) -> EdgeKind {
    match s {
        "contains" => EdgeKind::Contains,
        "calls" => EdgeKind::Calls,
        "imports" => EdgeKind::Imports,
        "exports" => EdgeKind::Exports,
        "extends" => EdgeKind::Extends,
        "implements" => EdgeKind::Implements,
        "references" => EdgeKind::References,
        "type_of" => EdgeKind::TypeOf,
        "returns" => EdgeKind::Returns,
        "instantiates" => EdgeKind::Instantiates,
        "overrides" => EdgeKind::Overrides,
        "decorates" => EdgeKind::Decorates,
        _ => EdgeKind::References,
    }
}

fn parse_language(s: &str) -> Language {
    match s {
        "typescript" => Language::TypeScript,
        "javascript" => Language::JavaScript,
        "tsx" => Language::Tsx,
        "jsx" => Language::Jsx,
        "python" => Language::Python,
        "go" => Language::Go,
        "rust" => Language::Rust,
        "java" => Language::Java,
        "c" => Language::C,
        "cpp" => Language::Cpp,
        "csharp" => Language::CSharp,
        "php" => Language::Php,
        "ruby" => Language::Ruby,
        "swift" => Language::Swift,
        "kotlin" => Language::Kotlin,
        "dart" => Language::Dart,
        "svelte" => Language::Svelte,
        "vue" => Language::Vue,
        "liquid" => Language::Liquid,
        "pascal" => Language::Pascal,
        "scala" => Language::Scala,
        "lua" => Language::Lua,
        "luau" => Language::Luau,
        _ => Language::Unknown,
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
}
