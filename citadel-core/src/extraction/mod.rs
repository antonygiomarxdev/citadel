pub mod languages;

use crate::types::*;

/// Result of extracting symbols from a single source file.
#[derive(Debug, Clone)]
pub struct ExtractionResult {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub unresolved_references: Vec<UnresolvedRef>,
    pub errors: Vec<ExtractionError>,
}

/// Trait implemented by each language extractor.
/// Each language module exports one struct implementing this trait.
pub trait LanguageExtractor: Send + Sync {
    fn extract(&self, source: &str, file_path: &str, language: Language, framework_names: &[String]) -> ExtractionResult;
}

/// Maps file extension to Language.
/// Mirrors the TS EXTENSION_MAP in src/extraction/grammars.ts.
pub fn detect_language(file_path: &str) -> Language {
    let lower = file_path.to_lowercase();
    if lower.ends_with(".ts") { return Language::TypeScript; }
    if lower.ends_with(".tsx") { return Language::Tsx; }
    if lower.ends_with(".js") { return Language::JavaScript; }
    if lower.ends_with(".jsx") { return Language::Jsx; }
    if lower.ends_with(".py") { return Language::Python; }
    if lower.ends_with(".go") { return Language::Go; }
    if lower.ends_with(".rs") { return Language::Rust; }
    if lower.ends_with(".java") { return Language::Java; }
    if lower.ends_with(".c") || lower.ends_with(".h") { return Language::C; }
    if lower.ends_with(".cpp") || lower.ends_with(".cc") || lower.ends_with(".cxx") || lower.ends_with(".hpp") { return Language::Cpp; }
    if lower.ends_with(".cs") { return Language::CSharp; }
    if lower.ends_with(".php") { return Language::Php; }
    if lower.ends_with(".rb") { return Language::Ruby; }
    if lower.ends_with(".swift") { return Language::Swift; }
    if lower.ends_with(".kt") || lower.ends_with(".kts") { return Language::Kotlin; }
    if lower.ends_with(".dart") { return Language::Dart; }
    if lower.ends_with(".svelte") { return Language::Svelte; }
    if lower.ends_with(".vue") { return Language::Vue; }
    if lower.ends_with(".liquid") { return Language::Liquid; }
    if lower.ends_with(".pas") || lower.ends_with(".dfm") || lower.ends_with(".fmx") { return Language::Pascal; }
    if lower.ends_with(".scala") || lower.ends_with(".sc") { return Language::Scala; }
    if lower.ends_with(".lua") { return Language::Lua; }
    if lower.ends_with(".luau") { return Language::Luau; }
    Language::Unknown
}

/// Generates a deterministic node ID matching the TS implementation.
/// Format: `{kind}:{32-char-hex}` — SHA-256 of `filePath:kind:name:line`
pub fn generate_node_id(kind: &NodeKind, name: &str, file_path: &str, line: u32) -> String {
    use sha2::{Sha256, Digest};
    let raw = format!("{file_path}:{}:{name}:{line}", kind.as_str());
    let hash = Sha256::digest(raw.as_bytes());
    let hex = format!("{hash:x}");
    format!("{}:{}", kind.as_str(), &hex[..32])
}

/// Creates an empty ExtractionResult with just the file node.
pub fn make_empty_result(file_path: &str, language: Language) -> ExtractionResult {
    let file_id = generate_node_id(&NodeKind::File, file_path, file_path, 0);
    ExtractionResult {
        nodes: vec![Node {
            id: file_id, kind: NodeKind::File, name: file_path.to_string(),
            qualified_name: file_path.to_string(), file_path: file_path.to_string(), language,
            start_line: 0, end_line: 0, start_column: 0, end_column: 0,
            docstring: None, signature: None, visibility: None,
            is_exported: false, is_async: false, is_static: false, is_abstract: false,
            decorators: None, type_parameters: None, updated_at: crate::storage::test_utils::now_ts(),
        }],
        edges: Vec::new(),
        unresolved_references: Vec::new(),
        errors: Vec::new(),
    }
}

/// Helpers for tree-sitter AST traversal.
pub mod tree_sitter_helpers {
    use tree_sitter::Node;

    /// Get the text of a node from source.
    pub fn get_node_text<'a>(node: &Node, source: &'a [u8]) -> &'a str {
        node.utf8_text(source).unwrap_or("")
    }

    /// Find a direct child node by field name.
    pub fn get_child_by_field_name<'a>(node: &Node<'a>, field: &str) -> Option<Node<'a>> {
        let mut cursor = node.walk();
        node.named_children(&mut cursor).find(|&child| node.field_name_for_child(child.id() as u32) == Some(field))
    }

    /// Get all named children of a node.
    pub fn get_named_children<'a>(node: &Node<'a>) -> Vec<Node<'a>> {
        let mut cursor = node.walk();
        node.named_children(&mut cursor).collect()
    }

    /// Get the preceding docstring comment for a node.
    pub fn get_preceding_docstring<'a>(node: &Node<'a>, source: &'a [u8]) -> Option<String> {
        let mut prev = node.prev_sibling();
        while let Some(ref p) = prev {
            let text = get_node_text(p, source);
            if text.starts_with("///") || text.starts_with("/**") || text.starts_with("//") {
                return Some(text.to_string());
            }
            if text.starts_with('#') || text.starts_with("\"\"\"") {
                return Some(text.to_string());
            }
            prev = p.prev_sibling();
        }
        None
    }

    /// Extract the name from a node using the given field name.
    pub fn extract_name<'a>(node: &Node<'a>, name_field: &str, source: &'a [u8]) -> String {
        if let Some(name_node) = get_child_by_field_name(node, name_field) {
            get_node_text(&name_node, source).to_string()
        } else {
            get_node_text(node, source).to_string()
        }
    }
}

/// Parallel file extraction using rayon.
/// Each file is parsed independently — tree-sitter is CPU-bound,
/// so par_iter() gives near-linear speedup on multi-core machines.
///
/// Returns results in the same order as the input files.
pub fn extract_files_parallel(
    files: &[(String, String)],
    framework_names: &[String],
) -> Vec<ExtractionResult> {
    use rayon::prelude::*;

    files.par_iter().map(|(file_path, source)| {
        let language = detect_language(file_path);
        if let Some(extractor) = languages::get_extractor(&language) {
            extractor.extract(source, file_path, language.clone(), framework_names)
        } else {
            // Fallback: return empty result with file node only
            make_empty_result(file_path, language)
        }
    }).collect()
}
