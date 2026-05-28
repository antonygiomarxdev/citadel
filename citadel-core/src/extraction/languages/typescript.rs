#![allow(clippy::ptr_arg, clippy::too_many_arguments)]
use crate::extraction::tree_sitter_helpers::*;
use crate::extraction::*;
use crate::types::*;
use tree_sitter::Node as TsNode;

// ── Extractor (adapta el pipeline nuevo al trait viejo para compat) ──

pub struct TypeScriptExtractor;

impl LanguageExtractor for TypeScriptExtractor {
    fn extract(
        &self,
        source: &str,
        file_path: &str,
        language: Language,
        _framework_names: &[String],
        parser: &mut tree_sitter::Parser,
    ) -> ExtractionResult {
        let source_bytes = source.as_bytes();
        let lang = if matches!(language, Language::Tsx | Language::Jsx) {
            tree_sitter_typescript::LANGUAGE_TSX
        } else {
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT
        };

        if let Err(e) = parser.set_language(&lang.into()) {
            let mut result = ExtractionResult::default(file_path, language.clone());
            result.errors.push(extraction_error(&format!("set_language: {e}")));
            return result;
        }

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => {
                let mut result = ExtractionResult::default(file_path, language.clone());
                result.errors.push(extraction_error("parse returned None"));
                return result;
            }
        };

        // Nuevo pipeline: TreeWalker + TypeScriptVisitor
        let mut visitor = TypeScriptVisitor::new(source_bytes, file_path, language);
        let mut scope = ScopeTracker::new(
            visitor.result.nodes.first()
                .map(|n| n.id.clone())
                .unwrap_or_default()
        );

        let root = tree.root_node();
        let mut cursor = root.walk();
        TreeWalker::walk(&mut cursor, source_bytes, &mut visitor, &mut scope);

        visitor.result
    }
}

// ── Visitor ──

pub struct TypeScriptVisitor<'a> {
    result: ExtractionResult,
    source: &'a [u8],
    file_path: &'a str,
    language: Language,
    scope: ScopeTracker,
}

impl<'a> TypeScriptVisitor<'a> {
    pub fn new(source: &'a [u8], file_path: &'a str, language: Language) -> Self {
        let file_id = generate_node_id(&NodeKind::File, file_path, file_path, 0);
        let result = ExtractionResult {
            nodes: vec![Node {
                id: file_id.clone(),
                kind: NodeKind::File,
                name: file_path.to_string(),
                qualified_name: file_path.to_string(),
                file_path: file_path.to_string(),
                language: language.clone(),
                start_line: 0, end_line: 0, start_column: 0, end_column: 0,
                docstring: None, signature: None, visibility: None,
                is_exported: false, is_async: false, is_static: false, is_abstract: false,
                decorators: None, type_parameters: None, updated_at: crate::util::now_ts(),
            }],
            edges: Vec::new(),
            unresolved_references: Vec::new(),
            errors: Vec::new(),
        };
        TypeScriptVisitor {
            result,
            source,
            file_path,
            language,
            scope: ScopeTracker::new(file_id),
        }
    }
}

impl AstVisitor for TypeScriptVisitor<'_> {
    fn scope_mut(&mut self) -> &mut ScopeTracker {
        &mut self.scope
    }

    fn result_mut(&mut self) -> &mut ExtractionResult {
        &mut self.result
    }

    fn on_function(&mut self, node: &TsNode, _source: &[u8]) {
        let name = extract_name(node, "name", self.source);
        if name.is_empty() || name == "function" { return; }
        self.emit_node(NodeKind::Function, name, node);
    }

    fn on_method(&mut self, node: &TsNode, _source: &[u8]) {
        let name = extract_name(node, "name", self.source);
        if name.is_empty() { return; }
        self.emit_node(NodeKind::Method, name, node);
    }

    fn on_class(&mut self, node: &TsNode, _source: &[u8]) {
        let name = extract_name(node, "name", self.source);
        if name.is_empty() { return; }
        let id = self.emit_node(NodeKind::Class, name, node);
        self.scope.enter(id);
    }

    fn on_interface(&mut self, node: &TsNode, _source: &[u8]) {
        let name = extract_name(node, "name", self.source);
        if name.is_empty() { return; }
        let id = self.emit_node(NodeKind::Interface, name, node);
        self.scope.enter(id);
    }

    fn on_type_alias(&mut self, node: &TsNode, _source: &[u8]) {
        let name = extract_name(node, "name", self.source);
        if name.is_empty() { return; }
        self.emit_node(NodeKind::TypeAlias, name, node);
    }

    fn on_enum(&mut self, node: &TsNode, _source: &[u8]) {
        let name = extract_name(node, "name", self.source);
        if name.is_empty() { return; }
        self.emit_node(NodeKind::Enum, name, node);
    }

    fn on_variable(&mut self, node: &TsNode, _source: &[u8]) {
        let name = extract_name(node, "name", self.source);
        if name.is_empty() { return; }
        self.emit_node(NodeKind::Variable, name, node);
    }

    fn on_import(&mut self, node: &TsNode, _source: &[u8]) {
        extract_import_inner(node, self.source, self.file_path, self.language.clone(), &mut self.result);
    }

    fn on_call(&mut self, node: &TsNode, _source: &[u8]) {
        if let Some(func) = get_child_by_field_name(node, "function") {
            let name = get_node_text(&func, self.source);
            if !name.is_empty() && !name.contains('.') && name.chars().next().is_some_and(|c| c.is_alphabetic()) {
                self.result.unresolved_references.push(UnresolvedRef {
                    from_node_id: self.scope.current().to_string(),
                    reference_name: name.to_string(),
                    reference_kind: "calls".to_string(),
                    line: Some(node.start_position().row as u32 + 1),
                    column: Some(node.start_position().column as u32),
                    candidates: None,
                    file_path: self.file_path.to_string(),
                    language: self.language.as_str().to_string(),
                });
            }
        }
    }

    fn on_instantiation(&mut self, node: &TsNode, _source: &[u8]) {
        if let Some(constructor) = get_child_by_field_name(node, "constructor") {
            let name = get_node_text(&constructor, self.source);
            if !name.is_empty() {
                self.result.unresolved_references.push(UnresolvedRef {
                    from_node_id: self.scope.current().to_string(),
                    reference_name: name.to_string(),
                    reference_kind: "instantiates".to_string(),
                    line: Some(node.start_position().row as u32 + 1),
                    column: Some(node.start_position().column as u32),
                    candidates: None,
                    file_path: self.file_path.to_string(),
                    language: self.language.as_str().to_string(),
                });
            }
        }
    }

    fn on_extends(&mut self, node: &TsNode, _source: &[u8]) {
        let parent_id = self.scope.current().to_string();
        for child in get_named_children(node) {
            let name = get_node_text(&child, self.source);
            if !name.is_empty() && name.chars().next().is_some_and(|c| c.is_uppercase()) {
                self.result.unresolved_references.push(UnresolvedRef {
                    from_node_id: parent_id.clone(),
                    reference_name: name.to_string(),
                    reference_kind: "extends".to_string(),
                    line: Some(node.start_position().row as u32 + 1),
                    column: Some(node.start_position().column as u32),
                    candidates: None,
                    file_path: self.file_path.to_string(),
                    language: self.language.as_str().to_string(),
                });
            }
        }
    }

    fn on_type_ref(&mut self, node: &TsNode, _source: &[u8]) {
        let text = get_node_text(node, self.source);
        if !text.is_empty() && text.chars().next().is_some_and(|c| c.is_uppercase()) {
            self.result.unresolved_references.push(UnresolvedRef {
                from_node_id: self.scope.current().to_string(),
                reference_name: text.to_string(),
                reference_kind: "type_of".to_string(),
                line: Some(node.start_position().row as u32 + 1),
                column: Some(node.start_position().column as u32),
                candidates: None,
                file_path: self.file_path.to_string(),
                language: self.language.as_str().to_string(),
            });
        }
    }

    fn scope_kinds(&self) -> &[&str] {
        &["class_declaration", "interface_declaration"]
    }
}

// ── Helpers internos ──

impl TypeScriptVisitor<'_> {
    fn emit_node(&mut self, kind: NodeKind, name: String, node: &TsNode) -> String {
        let parent_id = self.scope.current().to_string();
        let node_id = generate_node_id(&kind, &name, self.file_path, node.start_position().row as u32 + 1);
        let signature = get_node_text(node, self.source)
            .lines().next().unwrap_or("").to_string();
        let qname = format!("{}::{}", self.file_path, name);

        self.result.nodes.push(Node {
            id: node_id.clone(),
            kind,
            name,
            qualified_name: qname,
            file_path: self.file_path.to_string(),
            language: self.language.clone(),
            start_line: node.start_position().row as u32 + 1,
            end_line: node.end_position().row as u32 + 1,
            start_column: node.start_position().column as u32,
            end_column: node.end_position().column as u32,
            docstring: None,
            signature: if signature.is_empty() { None } else { Some(signature) },
            visibility: None,
            is_exported: false,
            is_async: false,
            is_static: false,
            is_abstract: false,
            decorators: None,
            type_parameters: None,
            updated_at: crate::util::now_ts(),
        });

        self.result.edges.push(Edge {
            source: parent_id,
            target: node_id.clone(),
            kind: EdgeKind::Contains,
            metadata: None,
            line: Some(node.start_position().row as u32 + 1),
            column: Some(node.start_position().column as u32),
            provenance: Some("tree-sitter".into()),
        });

        node_id
    }
}

fn extract_import_inner(
    node: &TsNode,
    source: &[u8],
    file_path: &str,
    language: Language,
    result: &mut ExtractionResult,
) {
    let text = get_node_text(node, source);
    let re = regex_lite::Regex::new(r#"import\s+(?:\{[^}]+}\s+)?(?:(\w+)\s+)?from\s+['"]([^'"]+)['"]"#).expect("hardcoded regex");
    if let Some(caps) = re.captures(text) {
        let _module_path = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        let named_re = regex_lite::Regex::new(r"\{([^}]+)\}").expect("hardcoded regex");
        if let Some(named_caps) = named_re.captures(text) {
            let Some(names) = named_caps.get(1) else { return; };
            for raw_name in names.as_str().split(',') {
                let name = raw_name.trim();
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
