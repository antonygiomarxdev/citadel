#![allow(clippy::ptr_arg, clippy::too_many_arguments)]
use crate::extraction::*;
use crate::extraction::tree_sitter_helpers::*;
use crate::types::*;
use tree_sitter::Node as TsNode;

pub struct PythonExtractor;

impl LanguageExtractor for PythonExtractor {
    fn extract(&self, source: &str, file_path: &str, language: Language, _framework_names: &[String], parser: &mut tree_sitter::Parser) -> ExtractionResult {
        let mut result = ExtractionResult {
            nodes: Vec::new(),
            edges: Vec::new(),
            unresolved_references: Vec::new(),
            errors: Vec::new(),
        };
        let source_bytes = source.as_bytes();

        parser.set_language(&tree_sitter_python::LANGUAGE.into())
            .map_err(|e| result.errors.push(ExtractionError { message: format!("set_language: {e}"), kind: ExtractionErrorKind::TreeSitterError, line: None, column: None }))
            .ok();

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => { result.errors.push(ExtractionError { message: "parse failed".into(), kind: ExtractionErrorKind::ParseError, line: None, column: None }); return result; }
        };

        let file_id = generate_node_id(&NodeKind::File, file_path, file_path, 0);
        result.nodes.push(Node {
            id: file_id.clone(), kind: NodeKind::File, name: file_path.to_string(),
            qualified_name: file_path.to_string(), file_path: file_path.to_string(), language: language.clone(),
            start_line: 0, end_line: 0, start_column: 0, end_column: 0,
            docstring: None, signature: None, visibility: None,
            is_exported: false, is_async: false, is_static: false, is_abstract: false,
            decorators: None, type_parameters: None, updated_at: crate::util::now_ts(),
        });
        let mut scope_stack: Vec<String> = vec![file_id.clone()];

        let root = tree.root_node();
        let mut cursor = root.walk();
        walk_python(&mut cursor, source_bytes, file_path, &language, &mut result, &mut scope_stack);
        result
    }
}

fn walk_python(
    cursor: &mut tree_sitter::TreeCursor,
    source: &[u8],
    file_path: &str,
    language: &Language,
    result: &mut ExtractionResult,
    scope_stack: &mut Vec<String>,
) {
    let node = cursor.node();
    if node.is_named() {
        let kind = node.kind();
        match kind {
            "function_definition" => extract_py_node(&node, NodeKind::Function, source, file_path, language.clone(), result, scope_stack),
            "class_definition" => {
                extract_py_node(&node, NodeKind::Class, source, file_path, language.clone(), result, scope_stack);
                // push scope for methods
                let name = extract_name(&node, "name", source);
                if !name.is_empty() {
                    let class_id = generate_node_id(&NodeKind::Class, &name, file_path, node.start_position().row as u32 + 1);
                    scope_stack.push(class_id);
                }
            }
            "import_statement" | "import_from_statement" => extract_py_import(&node, source, file_path, language.clone(), result, scope_stack),
            "call" => extract_py_call(&node, source, file_path, language.clone(), result, scope_stack),
            _ => {}
        }
    }

    if cursor.goto_first_child() {
        walk_python(cursor, source, file_path, language, result, scope_stack);
        cursor.goto_parent();
    }
    if cursor.goto_next_sibling() {
        walk_python(cursor, source, file_path, language, result, scope_stack);
    }
}

fn extract_py_node(
    node: &TsNode,
    kind: NodeKind,
    source: &[u8],
    file_path: &str,
    language: Language,
    result: &mut ExtractionResult,
    scope_stack: &mut Vec<String>,
) {
    let name = extract_name(node, "name", source);
    if name.is_empty() { return; }
    let parent_id = scope_stack.last().cloned().unwrap_or_default();
    let node_id = generate_node_id(&kind, &name, file_path, node.start_position().row as u32 + 1);

    result.nodes.push(Node {
        id: node_id.clone(), kind, name: name.clone(),
        qualified_name: format!("{file_path}::{name}"), file_path: file_path.to_string(), language: language.clone(),
        start_line: node.start_position().row as u32 + 1, end_line: node.end_position().row as u32 + 1,
        start_column: node.start_position().column as u32, end_column: node.end_position().column as u32,
        docstring: None, signature: Some(get_node_text(node, source).lines().next().unwrap_or("").to_string()),
        visibility: None, is_exported: false, is_async: extract_py_async(node), is_static: false, is_abstract: false,
        decorators: None, type_parameters: None, updated_at: crate::util::now_ts(),
    });
    result.edges.push(Edge {
        source: parent_id, target: node_id,
        kind: EdgeKind::Contains, metadata: None,
        line: Some(node.start_position().row as u32 + 1), column: Some(node.start_position().column as u32),
        provenance: Some("tree-sitter".into()),
    });
}

fn extract_py_import(
    node: &TsNode,
    source: &[u8],
    file_path: &str,
    language: Language,
    result: &mut ExtractionResult,
    scope_stack: &mut Vec<String>,
) {
    // Python: `from X import Y` or `import X`
    let text = get_node_text(node, source);
    let re = regex_lite::Regex::new(r"from\s+(\S+)\s+import\s+(.+)").expect("hardcoded regex");
    if let Some(caps) = re.captures(text) {
        let Some(names) = caps.get(2) else { return; };
        for name in names.as_str().split(',') {
            let name = name.trim();
            if name.is_empty() || name == "*" { continue; }
            result.unresolved_references.push(UnresolvedRef {
                from_node_id: scope_stack.last().cloned().unwrap_or_default(),
                reference_name: name.to_string(),
                reference_kind: "imports".to_string(),
                line: Some(node.start_position().row as u32 + 1),
                column: Some(node.start_position().column as u32),
                candidates: None, file_path: file_path.to_string(), language: language.as_str().to_string(),
            });
        }
    }
}

fn extract_py_call(
    node: &TsNode,
    source: &[u8],
    file_path: &str,
    language: Language,
    result: &mut ExtractionResult,
    scope_stack: &mut Vec<String>,
) {
    if let Some(func) = get_child_by_field_name(node, "function") {
        let name = get_node_text(&func, source);
        let from_node_id = scope_stack.last().cloned().unwrap_or_default();
        if !name.is_empty() && name.chars().next().is_some_and(|c| c.is_alphabetic()) {
            result.unresolved_references.push(UnresolvedRef {
                from_node_id, reference_name: name.to_string(), reference_kind: "calls".to_string(),
                line: Some(node.start_position().row as u32 + 1), column: Some(node.start_position().column as u32),
                candidates: None, file_path: file_path.to_string(), language: language.as_str().to_string(),
            });
        }
    }
}

// Python async: detect async def
fn extract_py_async(node: &TsNode) -> bool {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i)
            && child.kind() == "async"
        {
            return true;
        }
    }
    false
}
