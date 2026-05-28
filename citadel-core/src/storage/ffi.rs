#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::ffi::CStr;
use std::ffi::CString;
use std::ptr;

use rusqlite::ffi;

use crate::error::CitadelError;
use crate::types::{Edge, FileRecord, Node, UnresolvedRef};

macro_rules! chk {
    ($db:expr, $rc:expr) => {{
        let __rc = $rc;
        if __rc != ffi::SQLITE_OK && __rc != ffi::SQLITE_DONE {
            let err = ffi::sqlite3_errmsg($db);
            let msg = CStr::from_ptr(err).to_string_lossy().into_owned();
            return Err(CitadelError::Database(msg));
        }
    }};
}

macro_rules! bind_opt_str {
    ($stmt:expr, $idx:expr, $val:expr) => {{
        match $val {
            Some(v) => { ffi::sqlite3_bind_text($stmt, $idx, v.as_ptr() as *const std::os::raw::c_char, v.len() as std::os::raw::c_int, ffi::SQLITE_STATIC()); }
            None => { ffi::sqlite3_bind_null($stmt, $idx); }
        }
    }};
}

/// Execute a raw SQL string. Panics if SQL contains embedded NUL.
pub fn exec_raw(db: *mut ffi::sqlite3, sql: &str) -> Result<(), CitadelError> {
    let c = CString::new(sql).map_err(|_| CitadelError::Database("embedded NUL in SQL".into()))?;
    unsafe {
        let rc = ffi::sqlite3_exec(db, c.as_ptr(), None, ptr::null_mut(), ptr::null_mut());
        chk!(db, rc);
    }
    Ok(())
}

// ── Node indexes ──

pub fn drop_node_indexes(db: *mut ffi::sqlite3) -> Result<(), CitadelError> {
    exec_raw(db, concat!(
        "DROP INDEX IF EXISTS idx_nodes_kind;",
        "DROP INDEX IF EXISTS idx_nodes_name;",
        "DROP INDEX IF EXISTS idx_nodes_qualified_name;",
        "DROP INDEX IF EXISTS idx_nodes_language;",
        "DROP INDEX IF EXISTS idx_nodes_file_line;",
        "DROP INDEX IF EXISTS idx_nodes_lower_name;"
    ))
}

pub fn create_node_indexes(db: *mut ffi::sqlite3) -> Result<(), CitadelError> {
    exec_raw(db, concat!(
        "CREATE INDEX IF NOT EXISTS idx_nodes_kind ON nodes(kind);",
        "CREATE INDEX IF NOT EXISTS idx_nodes_name ON nodes(name);",
        "CREATE INDEX IF NOT EXISTS idx_nodes_qualified_name ON nodes(qualified_name);",
        "CREATE INDEX IF NOT EXISTS idx_nodes_language ON nodes(language);",
        "CREATE INDEX IF NOT EXISTS idx_nodes_file_line ON nodes(file_path, start_line);",
        "CREATE INDEX IF NOT EXISTS idx_nodes_lower_name ON nodes(lower(name));"
    ))
}

// ── Node insert (multi-VALUES, batch via sqlite3_exec) ──

/// Build one row of a multi-VALUES SQL fragment from a Node.
/// Appends `(v1,v2,...,v20)` to `sql`.
fn append_node_row(sql: &mut String, n: &Node) {
    sql.push('(');
    esc_val(sql, &n.id); sql.push(',');
    esc_val(sql, n.kind.as_str()); sql.push(',');
    esc_val(sql, &n.name); sql.push(',');
    esc_val(sql, &n.qualified_name); sql.push(',');
    esc_val(sql, &n.file_path); sql.push(',');
    esc_val(sql, n.language.as_str()); sql.push(',');
    sql.push_str(&n.start_line.to_string()); sql.push(',');
    sql.push_str(&n.end_line.to_string()); sql.push(',');
    sql.push_str(&n.start_column.to_string()); sql.push(',');
    sql.push_str(&n.end_column.to_string()); sql.push(',');
    esc_opt(sql, n.docstring.as_deref()); sql.push(',');
    esc_opt(sql, n.signature.as_deref()); sql.push(',');
    esc_opt(sql, n.visibility.as_deref()); sql.push(',');
    sql.push_str(&(n.is_exported as i64).to_string()); sql.push(',');
    sql.push_str(&(n.is_async as i64).to_string()); sql.push(',');
    sql.push_str(&(n.is_static as i64).to_string()); sql.push(',');
    sql.push_str(&(n.is_abstract as i64).to_string()); sql.push(',');
    let dec = n.decorators.as_ref().and_then(|d| serde_json::to_string(d).ok());
    esc_opt(sql, dec.as_deref()); sql.push(',');
    let tp = n.type_parameters.as_ref().and_then(|t| serde_json::to_string(t).ok());
    esc_opt(sql, tp.as_deref()); sql.push(',');
    sql.push_str(&n.updated_at.to_string());
    sql.push(')');
}

fn esc_val(sql: &mut String, val: &str) {
    sql.push('\'');
    for c in val.chars() {
        if c == '\'' { sql.push_str("''"); } else { sql.push(c); }
    }
    sql.push('\'');
}

fn esc_opt(sql: &mut String, val: Option<&str>) {
    match val {
        Some(s) => esc_val(sql, s),
        None => sql.push_str("NULL"),
    }
}

/// Insert nodes using multi-VALUES (sqlite3_exec).
/// Batches of 500 rows per SQL statement for optimal balance
/// between SQL parse cost and per-row overhead.
pub fn insert_nodes_multi_values(db: *mut ffi::sqlite3, nodes: &[&Node]) -> Result<u64, CitadelError> {
    let start = std::time::Instant::now();

    const BATCH: usize = 500;
    let sql_prefix = "INSERT OR REPLACE INTO nodes(id,kind,name,qualified_name,file_path,language,start_line,end_line,start_column,end_column,docstring,signature,visibility,is_exported,is_async,is_static,is_abstract,decorators,type_parameters,updated_at) VALUES ";

    for chunk in nodes.chunks(BATCH) {
        let mut sql = String::with_capacity(chunk.len() * 200 + 200);
        sql.push_str(sql_prefix);
        for (j, n) in chunk.iter().enumerate() {
            if j > 0 { sql.push(','); }
            append_node_row(&mut sql, n);
        }
        exec_raw(db, &sql)?;
    }

    let elapsed = std::time::Instant::now().duration_since(start).as_micros() as u64;
    Ok(elapsed)
}

// ── Edge insert (prepared statement via bind/step/reset) ──

pub fn insert_edges_prepared(db: *mut ffi::sqlite3, edges: &[&Edge]) -> Result<(), CitadelError> {
    let sql = CString::new(concat!(
        "INSERT OR IGNORE INTO edges(source,target,kind,metadata,line,col,provenance) ",
        "VALUES(?1,?2,?3,?4,?5,?6,?7)"
    )).unwrap();
    let mut stmt: *mut ffi::sqlite3_stmt = ptr::null_mut();
    unsafe {
        chk!(db, ffi::sqlite3_prepare_v2(db, sql.as_ptr(), -1, &mut stmt, ptr::null_mut()));
        for e in edges {
            bind_opt_str!(stmt, 1, Some(e.source.as_str()));
            bind_opt_str!(stmt, 2, Some(e.target.as_str()));
            bind_opt_str!(stmt, 3, Some(e.kind.as_str()));
            let meta = e.metadata.as_ref().and_then(|m| m.as_str());
            bind_opt_str!(stmt, 4, meta);
            match e.line { Some(v) => { ffi::sqlite3_bind_int64(stmt, 5, v as i64); } None => { ffi::sqlite3_bind_null(stmt, 5); } }
            match e.column { Some(v) => { ffi::sqlite3_bind_int64(stmt, 6, v as i64); } None => { ffi::sqlite3_bind_null(stmt, 6); } }
            bind_opt_str!(stmt, 7, e.provenance.as_deref());
            chk!(db, ffi::sqlite3_step(stmt));
            ffi::sqlite3_reset(stmt);
        }
        ffi::sqlite3_finalize(stmt);
    }
    Ok(())
}

// ── Unresolved ref insert (prepared statement) ──

pub fn insert_refs_prepared(db: *mut ffi::sqlite3, refs: &[&UnresolvedRef]) -> Result<(), CitadelError> {
    let sql = CString::new(concat!(
        "INSERT INTO unresolved_refs(from_node_id,reference_name,reference_kind,",
        "line,col,candidates,file_path,language) ",
        "VALUES(?1,?2,?3,?4,?5,?6,?7,?8)"
    )).unwrap();
    let mut stmt: *mut ffi::sqlite3_stmt = ptr::null_mut();
    unsafe {
        chk!(db, ffi::sqlite3_prepare_v2(db, sql.as_ptr(), -1, &mut stmt, ptr::null_mut()));
        for r in refs {
            bind_opt_str!(stmt, 1, Some(r.from_node_id.as_str()));
            bind_opt_str!(stmt, 2, Some(r.reference_name.as_str()));
            bind_opt_str!(stmt, 3, Some(r.reference_kind.as_str()));
            match r.line { Some(v) => { ffi::sqlite3_bind_int64(stmt, 4, v as i64); } None => { ffi::sqlite3_bind_null(stmt, 4); } }
            match r.column { Some(v) => { ffi::sqlite3_bind_int64(stmt, 5, v as i64); } None => { ffi::sqlite3_bind_null(stmt, 5); } }
            let candidates = r.candidates.as_ref().and_then(|c| c.as_str());
            bind_opt_str!(stmt, 6, candidates);
            bind_opt_str!(stmt, 7, Some(r.file_path.as_str()));
            bind_opt_str!(stmt, 8, Some(r.language.as_str()));
            chk!(db, ffi::sqlite3_step(stmt));
            ffi::sqlite3_reset(stmt);
        }
        ffi::sqlite3_finalize(stmt);
    }
    Ok(())
}

// ── File insert (prepared statement) ──

pub fn insert_files_prepared(db: *mut ffi::sqlite3, files: &[FileRecord]) -> Result<(), CitadelError> {
    let sql = CString::new(concat!(
        "INSERT INTO files(path,content_hash,language,size,modified_at,",
        "indexed_at,node_count,errors) ",
        "VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ",
        "ON CONFLICT(path) DO UPDATE SET ",
        "content_hash=excluded.content_hash,",
        "language=excluded.language,",
        "size=excluded.size,",
        "modified_at=excluded.modified_at,",
        "indexed_at=excluded.indexed_at,",
        "node_count=excluded.node_count,",
        "errors=excluded.errors"
    )).unwrap();
    let mut stmt: *mut ffi::sqlite3_stmt = ptr::null_mut();
    unsafe {
        chk!(db, ffi::sqlite3_prepare_v2(db, sql.as_ptr(), -1, &mut stmt, ptr::null_mut()));
        for f in files {
            bind_opt_str!(stmt, 1, Some(f.path.as_str()));
            bind_opt_str!(stmt, 2, Some(f.content_hash.as_str()));
            bind_opt_str!(stmt, 3, Some(f.language.to_string().as_str()));
            ffi::sqlite3_bind_int64(stmt, 4, f.size as i64);
            ffi::sqlite3_bind_int64(stmt, 5, f.modified_at);
            ffi::sqlite3_bind_int64(stmt, 6, f.indexed_at);
            ffi::sqlite3_bind_int64(stmt, 7, f.node_count as i64);
            let errs = f.errors.as_ref().and_then(|e| serde_json::to_string(e).ok());
            bind_opt_str!(stmt, 8, errs.as_deref());
            chk!(db, ffi::sqlite3_step(stmt));
            ffi::sqlite3_reset(stmt);
        }
        ffi::sqlite3_finalize(stmt);
    }
    Ok(())
}
