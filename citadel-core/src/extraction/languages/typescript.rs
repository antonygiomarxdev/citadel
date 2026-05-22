#![allow(clippy::ptr_arg, clippy::too_many_arguments)]
use crate::extraction::*;
use crate::extraction::tree_sitter_helpers::*;
use crate::types::*;
use tree_sitter::{Parser, Node as TsNode};

pub struct TypeScriptExtractor;

impl LanguageExtractor for TypeScriptExtractor {
    fn extract(&self, source: &str, file_path: &str, language: Language, _framework_names: &[String]) -> ExtractionResult {
        let mut result = ExtractionResult {
            nodes: Vec::new(),
            edges: Vec::new(),
            unresolved_references: Vec::new(),
            errors: Vec::new(),
        };

        let source_bytes = source.as_bytes();
        let lang = if matches!(language, Language::Tsx | Language::Jsx) {
            tree_sitter_typescript::LANGUAGE_TSX
        } else {
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT
        };

        let mut parser = Parser::new();
        parser.set_language(&lang.into())
            .map_err(|e| result.errors.push(ExtractionError {
                message: format!("set_language: {e}"),
                kind: ExtractionErrorKind::TreeSitterError,
                line: None,
                column: None,
            }))
            .ok();

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => {
                result.errors.push(ExtractionError {
                    message: "parse returned None".into(),
                    kind: ExtractionErrorKind::ParseError,
                    line: None,
                    column: None,
                });
                return result;
            }
        };

        // Create file node
        let file_id = generate_node_id(&NodeKind::File, file_path, file_path, 0);
        result.nodes.push(Node {
            id: file_id.clone(),
            kind: NodeKind::File,
            name: file_path.to_string(),
            qualified_name: file_path.to_string(),
            file_path: file_path.to_string(),
            language: language.clone(),
            start_line: 0,
            end_line: 0,
            start_column: 0,
            end_column: 0,
            docstring: None,
            signature: None,
            visibility: None,
            is_exported: false,
            is_async: false,
            is_static: false,
            is_abstract: false,
            decorators: None,
            type_parameters: None,
            updated_at: crate::storage::test_utils::now_ts(),
        });

        let mut scope_stack: Vec<String> = vec![file_id.clone()];

        // Walk the AST
        let root = tree.root_node();
        let mut cursor = root.walk();
        walk_tree(
            &mut cursor,
            source_bytes,
            file_path,
            language,
            &mut result,
            &mut scope_stack,
        );

        // Add 'contains' edges from each parent to its children
        // The scope_stack tracks the current container; edges from parent→child
        // are added as nodes are created.

        result
    }
}

fn walk_tree(
    cursor: &mut tree_sitter::TreeCursor,
    source: &[u8],
    file_path: &str,
    language: Language,
    result: &mut ExtractionResult,
    scope_stack: &mut Vec<String>,
) {
    let node = cursor.node();

    if node.is_named() {
        let kind = node.kind();
        let _text = get_node_text(&node, source);

        match kind {
            "function_declaration" | "function_expression" | "arrow_function" => {
                extract_function(&node, source, file_path, language.clone(), result, scope_stack);
            }
            "method_definition" => {
                extract_method(&node, source, file_path, language.clone(), result, scope_stack);
            }
            "class_declaration" => {
                extract_class(&node, source, file_path, language.clone(), result, scope_stack);
            }
            "interface_declaration" => {
                extract_interface(&node, source, file_path, language.clone(), result, scope_stack);
            }
            "type_alias_declaration" => {
                extract_type_alias(&node, source, file_path, language.clone(), result, scope_stack);
            }
            "enum_declaration" => {
                extract_enum(&node, source, file_path, language.clone(), result, scope_stack);
            }
            "variable_declaration" | "lexical_declaration" | "assignment_expression" => {
                extract_variable(&node, source, file_path, language.clone(), result, scope_stack);
            }
            "import_statement" | "import_declaration" => {
                extract_import(&node, source, file_path, language.clone(), result, scope_stack);
            }
            "call_expression" => {
                extract_call(&node, source, file_path, language.clone(), result, scope_stack);
            }
            "new_expression" => {
                extract_instantiation(&node, source, file_path, language.clone(), result, scope_stack);
            }
            "extends_clause" => {
                extract_inheritance(&node, source, file_path, language.clone(), result, scope_stack);
            }
            "type_annotation" | "type_reference" => {
                extract_type_ref(&node, source, file_path, language.clone(), result, scope_stack);
            }
            _ => {}
        }
    }

    // Recurse into children
    if cursor.goto_first_child() {
        walk_tree(cursor, source, file_path, language.clone(), result, scope_stack);
        cursor.goto_parent();
    }
    if cursor.goto_next_sibling() {
        walk_tree(cursor, source, file_path, language.clone(), result, scope_stack);
    }
}

fn extract_function(
    node: &TsNode,
    source: &[u8],
    file_path: &str,
    language: Language,
    result: &mut ExtractionResult,
    scope_stack: &mut Vec<String>,
) {
    let name = extract_name(node, "name", source);
    if name.is_empty() || name == "function" { return; }

    let parent_id = scope_stack.last().cloned().unwrap_or_default();
    let node_id = generate_node_id(&NodeKind::Function, &name, file_path, node.start_position().row as u32 + 1);
    let signature = get_node_text(node, source).lines().next().unwrap_or("").to_string();

    result.nodes.push(build_node(
        &node_id, NodeKind::Function, &name, file_path, language.clone(),
        node, signature, None,
    ));
    result.edges.push(Edge {
        source: parent_id,
        target: node_id.clone(),
        kind: EdgeKind::Contains,
        metadata: None,
        line: Some(node.start_position().row as u32 + 1),
        column: Some(node.start_position().column as u32),
        provenance: Some("tree-sitter".into()),
    });
}

fn extract_method(
    node: &TsNode,
    source: &[u8],
    file_path: &str,
    language: Language,
    result: &mut ExtractionResult,
    scope_stack: &mut Vec<String>,
) {
    let name = extract_name(node, "name", source);
    if name.is_empty() { return; }
    let parent_id = scope_stack.last().cloned().unwrap_or_default();
    let node_id = generate_node_id(&NodeKind::Method, &name, file_path, node.start_position().row as u32 + 1);
    let signature = get_node_text(node, source).lines().next().unwrap_or("").to_string();

    result.nodes.push(build_node(
        &node_id, NodeKind::Method, &name, file_path, language.clone(),
        node, signature, None,
    ));
    result.edges.push(Edge {
        source: parent_id,
        target: node_id.clone(),
        kind: EdgeKind::Contains,
        metadata: None,
        line: Some(node.start_position().row as u32 + 1),
        column: Some(node.start_position().column as u32),
        provenance: Some("tree-sitter".into()),
    });
}

fn extract_class(
    node: &TsNode,
    source: &[u8],
    file_path: &str,
    language: Language,
    result: &mut ExtractionResult,
    scope_stack: &mut Vec<String>,
) {
    let name = extract_name(node, "name", source);
    if name.is_empty() { return; }
    let parent_id = scope_stack.last().cloned().unwrap_or_default();
    let node_id = generate_node_id(&NodeKind::Class, &name, file_path, node.start_position().row as u32 + 1);

    result.nodes.push(build_node(
        &node_id, NodeKind::Class, &name, file_path, language.clone(),
        node, String::new(), None,
    ));
    result.edges.push(Edge {
        source: parent_id,
        target: node_id.clone(),
        kind: EdgeKind::Contains,
        metadata: None,
        line: Some(node.start_position().row as u32 + 1),
        column: Some(node.start_position().column as u32),
        provenance: Some("tree-sitter".into()),
    });

    // Push class as scope for methods
    scope_stack.push(node_id);
}

fn extract_interface(
    node: &TsNode,
    source: &[u8],
    file_path: &str,
    language: Language,
    result: &mut ExtractionResult,
    scope_stack: &mut Vec<String>,
) {
    let name = extract_name(node, "name", source);
    if name.is_empty() { return; }
    let parent_id = scope_stack.last().cloned().unwrap_or_default();
    let node_id = generate_node_id(&NodeKind::Interface, &name, file_path, node.start_position().row as u32 + 1);

    result.nodes.push(build_node(
        &node_id, NodeKind::Interface, &name, file_path, language.clone(),
        node, String::new(), None,
    ));
    result.edges.push(Edge {
        source: parent_id,
        target: node_id.clone(),
        kind: EdgeKind::Contains,
        metadata: None,
        line: Some(node.start_position().row as u32 + 1),
        column: Some(node.start_position().column as u32),
        provenance: Some("tree-sitter".into()),
    });
}

fn extract_type_alias(
    node: &TsNode,
    source: &[u8],
    file_path: &str,
    language: Language,
    result: &mut ExtractionResult,
    scope_stack: &mut Vec<String>,
) {
    let name = extract_name(node, "name", source);
    if name.is_empty() { return; }
    let parent_id = scope_stack.last().cloned().unwrap_or_default();
    let node_id = generate_node_id(&NodeKind::TypeAlias, &name, file_path, node.start_position().row as u32 + 1);

    result.nodes.push(build_node(
        &node_id, NodeKind::TypeAlias, &name, file_path, language.clone(),
        node, String::new(), None,
    ));
    result.edges.push(Edge {
        source: parent_id,
        target: node_id.clone(),
        kind: EdgeKind::Contains,
        metadata: None,
        line: Some(node.start_position().row as u32 + 1),
        column: Some(node.start_position().column as u32),
        provenance: Some("tree-sitter".into()),
    });
}

fn extract_enum(
    node: &TsNode,
    source: &[u8],
    file_path: &str,
    language: Language,
    result: &mut ExtractionResult,
    scope_stack: &mut Vec<String>,
) {
    let name = extract_name(node, "name", source);
    if name.is_empty() { return; }
    let parent_id = scope_stack.last().cloned().unwrap_or_default();
    let node_id = generate_node_id(&NodeKind::Enum, &name, file_path, node.start_position().row as u32 + 1);

    result.nodes.push(build_node(
        &node_id, NodeKind::Enum, &name, file_path, language.clone(),
        node, String::new(), None,
    ));
    result.edges.push(Edge {
        source: parent_id,
        target: node_id.clone(),
        kind: EdgeKind::Contains,
        metadata: None,
        line: Some(node.start_position().row as u32 + 1),
        column: Some(node.start_position().column as u32),
        provenance: Some("tree-sitter".into()),
    });
}

fn extract_variable(
    node: &TsNode,
    source: &[u8],
    file_path: &str,
    language: Language,
    result: &mut ExtractionResult,
    scope_stack: &mut Vec<String>,
) {
    let name = extract_name(node, "name", source);
    if name.is_empty() { return; }
    let parent_id = scope_stack.last().cloned().unwrap_or_default();
    let node_id = generate_node_id(&NodeKind::Variable, &name, file_path, node.start_position().row as u32 + 1);

    result.nodes.push(build_node(
        &node_id, NodeKind::Variable, &name, file_path, language.clone(),
        node, String::new(), None,
    ));
    result.edges.push(Edge {
        source: parent_id,
        target: node_id.clone(),
        kind: EdgeKind::Contains,
        metadata: None,
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
    _scope_stack: &mut Vec<String>,
) {
    // For import statements, we create unresolved references.
    // The actual resolution happens in the ReferenceResolver.
    let text = get_node_text(node, source);
    let re = regex_lite::Regex::new(r#"import\s+(?:\{[^}]+}\s+)?(?:(\w+)\s+)?from\s+['"]([^'"]+)['"]"#).unwrap();
    if let Some(caps) = re.captures(text) {
        let _module_path = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        // Each named import becomes an unresolved ref
        let named_re = regex_lite::Regex::new(r"\{([^}]+)\}").unwrap();
        if let Some(named_caps) = named_re.captures(text) {
            let names = named_caps.get(1).unwrap().as_str();
            for name in names.split(',') {
                let name = name.trim();
                if name.is_empty() { continue; }
                let from_node_id = generate_node_id(&NodeKind::Import, name, file_path, node.start_position().row as u32 + 1);
                result.unresolved_references.push(UnresolvedRef {
                    from_node_id,
                    reference_name: name.to_string(),
                    reference_kind: "imports".to_string(),
                    line: Some(node.start_position().row as u32 + 1),
                    column: Some(node.start_position().column as u32),
                    candidates: None,
                    file_path: file_path.to_string(),
                    language: language.as_str().to_string(),
                });
            }
        }
    }
}

fn extract_call(
    node: &TsNode,
    source: &[u8],
    file_path: &str,
    language: Language,
    result: &mut ExtractionResult,
    scope_stack: &mut Vec<String>,
) {
    // Find the function/method being called
    if let Some(func) = get_child_by_field_name(node, "function") {
        let name = get_node_text(&func, source);
        let from_node_id = scope_stack.last().cloned().unwrap_or_default();
        if !name.is_empty() && !name.contains('.') && name.chars().next().is_some_and(|c| c.is_alphabetic()) {
            result.unresolved_references.push(UnresolvedRef {
                from_node_id,
                reference_name: name.to_string(),
                reference_kind: "calls".to_string(),
                line: Some(node.start_position().row as u32 + 1),
                column: Some(node.start_position().column as u32),
                candidates: None,
                file_path: file_path.to_string(),
                language: language.as_str().to_string(),
            });
        }
    }
}

fn extract_instantiation(
    node: &TsNode,
    source: &[u8],
    file_path: &str,
    language: Language,
    result: &mut ExtractionResult,
    scope_stack: &mut Vec<String>,
) {
    if let Some(constructor) = get_child_by_field_name(node, "constructor") {
        let name = get_node_text(&constructor, source);
        let from_node_id = scope_stack.last().cloned().unwrap_or_default();
        if !name.is_empty() {
            result.unresolved_references.push(UnresolvedRef {
                from_node_id,
                reference_name: name.to_string(),
                reference_kind: "instantiates".to_string(),
                line: Some(node.start_position().row as u32 + 1),
                column: Some(node.start_position().column as u32),
                candidates: None,
                file_path: file_path.to_string(),
                language: language.as_str().to_string(),
            });
        }
    }
}

fn extract_inheritance(
    node: &TsNode,
    source: &[u8],
    file_path: &str,
    language: Language,
    result: &mut ExtractionResult,
    scope_stack: &mut Vec<String>,
) {
    let parent_id = scope_stack.last().cloned().unwrap_or_default();
    let named_children = get_named_children(node);
    for child in named_children {
        let name = get_node_text(&child, source);
        if !name.is_empty() && name.chars().next().is_some_and(|c| c.is_uppercase()) {
            result.unresolved_references.push(UnresolvedRef {
                from_node_id: parent_id.clone(),
                reference_name: name.to_string(),
                reference_kind: "extends".to_string(),
                line: Some(node.start_position().row as u32 + 1),
                column: Some(node.start_position().column as u32),
                candidates: None,
                file_path: file_path.to_string(),
                language: language.as_str().to_string(),
            });
        }
    }
}

fn extract_type_ref(
    node: &TsNode,
    source: &[u8],
    file_path: &str,
    language: Language,
    result: &mut ExtractionResult,
    scope_stack: &mut Vec<String>,
) {
    let text = get_node_text(node, source);
    let from_node_id = scope_stack.last().cloned().unwrap_or_default();
    if !text.is_empty() && text.chars().next().is_some_and(|c| c.is_uppercase()) {
        result.unresolved_references.push(UnresolvedRef {
            from_node_id,
            reference_name: text.to_string(),
            reference_kind: "type_of".to_string(),
            line: Some(node.start_position().row as u32 + 1),
            column: Some(node.start_position().column as u32),
            candidates: None,
            file_path: file_path.to_string(),
            language: language.as_str().to_string(),
        });
    }
}

fn build_node(
    id: &str,
    kind: NodeKind,
    name: &str,
    file_path: &str,
    language: Language,
    node: &TsNode,
    signature: String,
    visibility: Option<String>,
) -> Node {
    Node {
        id: id.to_string(),
        kind,
        name: name.to_string(),
        qualified_name: format!("{file_path}::{name}"),
        file_path: file_path.to_string(),
        language,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_column: node.start_position().column as u32,
        end_column: node.end_position().column as u32,
        docstring: None,
        signature: if signature.is_empty() { None } else { Some(signature) },
        visibility,
        is_exported: false,
        is_async: false,
        is_static: false,
        is_abstract: false,
        decorators: None,
        type_parameters: None,
        updated_at: crate::storage::test_utils::now_ts(),
    }
}
