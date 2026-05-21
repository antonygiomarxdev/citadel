#![allow(clippy::ptr_arg, clippy::too_many_arguments)]
use crate::extraction::*;
use crate::extraction::tree_sitter_helpers::*;
use crate::types::*;
use tree_sitter::{Tree, Node as TsNode};

#[derive(Clone)]
pub struct ExtractorConfig {
    pub function_kinds: Vec<String>,
    pub class_kinds: Vec<String>,
    pub interface_kinds: Vec<String>,
    pub struct_kinds: Vec<String>,
    pub variable_kinds: Vec<String>,
    pub import_kinds: Vec<String>,
    pub call_kinds: Vec<String>,
    pub name_field: &'static str,
    pub body_field: &'static str,
}

pub fn walk(
    tree: &Tree,
    source: &str,
    file_path: &str,
    language: Language,
    result: &mut ExtractionResult,
    config: &ExtractorConfig,
) {
    let source_bytes = source.as_bytes();
    let root = tree.root_node();
    let mut cursor = root.walk();
    let mut scope_stack: Vec<String> = vec![result.nodes.first().map(|n| n.id.clone()).unwrap_or_default()];
    walk_node(&mut cursor, source_bytes, file_path, &language, result, &mut scope_stack, config);
}

fn walk_node(
    cursor: &mut tree_sitter::TreeCursor,
    source: &[u8],
    file_path: &str,
    language: &Language,
    result: &mut ExtractionResult,
    scope_stack: &mut Vec<String>,
    config: &ExtractorConfig,
) {
    let node = cursor.node();
    if node.is_named() {
        let kind = node.kind();
        if config.function_kinds.iter().any(|k| k == kind) {
            extract_node(&node, NodeKind::Function, source, file_path, language.clone(), result, scope_stack, config);
        } else if config.class_kinds.iter().any(|k| k == kind) {
            extract_node(&node, NodeKind::Class, source, file_path, language.clone(), result, scope_stack, config);
        } else if config.interface_kinds.iter().any(|k| k == kind) {
            extract_node(&node, NodeKind::Interface, source, file_path, language.clone(), result, scope_stack, config);
        } else if config.struct_kinds.iter().any(|k| k == kind) {
            extract_node(&node, NodeKind::Struct, source, file_path, language.clone(), result, scope_stack, config);
        } else if config.variable_kinds.iter().any(|k| k == kind) {
            extract_node(&node, NodeKind::Variable, source, file_path, language.clone(), result, scope_stack, config);
        } else if config.import_kinds.iter().any(|k| k == kind) {
            extract_import(&node, source, file_path, language.clone(), result, config);
        } else if config.call_kinds.iter().any(|k| k == kind) {
            extract_call(&node, source, file_path, language.clone(), result, scope_stack);
        }
    }
    if cursor.goto_first_child() {
        walk_node(cursor, source, file_path, language, result, scope_stack, config);
        cursor.goto_parent();
    }
    if cursor.goto_next_sibling() {
        walk_node(cursor, source, file_path, language, result, scope_stack, config);
    }
}

fn extract_node(
    node: &TsNode,
    kind: NodeKind,
    source: &[u8],
    file_path: &str,
    language: Language,
    result: &mut ExtractionResult,
    scope_stack: &mut Vec<String>,
    config: &ExtractorConfig,
) {
    let name = extract_name(node, config.name_field, source);
    if name.is_empty() { return; }
    let parent_id = scope_stack.last().cloned().unwrap_or_default();
    let node_id = generate_node_id(&kind, &name, file_path, node.start_position().row as u32 + 1);
    let qname = format!("{file_path}::{name}");
    let sig = get_node_text(node, source).lines().next().unwrap_or("").to_string();
    result.nodes.push(Node {
        id: node_id.clone(), kind, name,
        qualified_name: qname,
        file_path: file_path.to_string(), language,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_column: node.start_position().column as u32,
        end_column: node.end_position().column as u32,
        docstring: None, signature: if sig.is_empty() { None } else { Some(sig) },
        visibility: None, is_exported: false, is_async: false, is_static: false, is_abstract: false,
        decorators: None, type_parameters: None, updated_at: crate::storage::test_utils::now_ts(),
    });
    result.edges.push(Edge {
        source: parent_id, target: node_id,
        kind: EdgeKind::Contains, metadata: None,
        line: Some(node.start_position().row as u32 + 1),
        column: Some(node.start_position().column as u32),
        provenance: Some("tree-sitter".into()),
    });
}

fn extract_import(
    node: &TsNode,
    source: &[u8],
    file_path: &str,
    language: Language,
    result: &mut ExtractionResult,
    _config: &ExtractorConfig,
) {
    let text = get_node_text(node, source);
    if text.is_empty() { return; }
    result.unresolved_references.push(UnresolvedRef {
        from_node_id: "file".to_string(),
        reference_name: text.lines().next().unwrap_or("").to_string(),
        reference_kind: "imports".to_string(),
        line: Some(node.start_position().row as u32 + 1),
        column: Some(node.start_position().column as u32),
        candidates: None, file_path: file_path.to_string(), language: language.as_str().to_string(),
    });
}

fn extract_call(
    node: &TsNode,
    source: &[u8],
    file_path: &str,
    language: Language,
    result: &mut ExtractionResult,
    scope_stack: &mut Vec<String>,
) {
    let text = get_node_text(node, source);
    let from_node_id = scope_stack.last().cloned().unwrap_or_default();
    if let Some(name) = text.split('(').next() {
        let name = name.trim();
        if !name.is_empty() && name.chars().next().is_some_and(|c| c.is_alphabetic()) {
            result.unresolved_references.push(UnresolvedRef {
                from_node_id, reference_name: name.to_string(), reference_kind: "calls".to_string(),
                line: Some(node.start_position().row as u32 + 1), column: Some(node.start_position().column as u32),
                candidates: None, file_path: file_path.to_string(), language: language.as_str().to_string(),
            });
        }
    }
}
