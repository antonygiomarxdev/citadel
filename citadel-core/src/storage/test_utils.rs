use crate::types::*;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

pub fn make_node(id: &str, name: &str, kind: NodeKind, file_path: &str, language: Language) -> Node {
    Node {
        id: id.to_string(),
        kind,
        name: name.to_string(),
        qualified_name: format!("{file_path}::{name}"),
        file_path: file_path.to_string(),
        language,
        start_line: 0,
        end_line: 10,
        start_column: 0,
        end_column: 5,
        docstring: None,
        signature: None,
        visibility: None,
        is_exported: false,
        is_async: false,
        is_static: false,
        is_abstract: false,
        decorators: None,
        type_parameters: None,
        updated_at: now_ts(),
    }
}

pub fn make_edge(source: &str, target: &str, kind: EdgeKind) -> Edge {
    Edge {
        source: source.to_string(),
        target: target.to_string(),
        kind,
        metadata: None,
        line: Some(1),
        column: Some(0),
        provenance: None,
    }
}

pub fn make_file(path: &str, language: Language) -> FileRecord {
    FileRecord {
        path: path.to_string(),
        content_hash: format!("hash_{path}"),
        language,
        size: 100,
        modified_at: now_ts(),
        indexed_at: now_ts(),
        node_count: 0,
        errors: None,
    }
}

pub fn make_unresolved_ref(from_node_id: &str, name: &str, file_path: &str) -> UnresolvedRef {
    UnresolvedRef {
        from_node_id: from_node_id.to_string(),
        reference_name: name.to_string(),
        reference_kind: "import".to_string(),
        line: Some(5),
        column: Some(0),
        candidates: None,
        file_path: file_path.to_string(),
        language: "typescript".to_string(),
    }
}

pub fn make_hashmap(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for (k, v) in pairs {
        map.insert(k.to_string(), v.to_string());
    }
    map
}
