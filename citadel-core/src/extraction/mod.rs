pub mod languages;

mod source_file;
mod file_reader;
mod scope_tracker;
mod ast_visitor;
mod tree_walker;
mod strategy;
mod pipeline;

pub use source_file::SourceFile;
pub use file_reader::FileReader;
pub use scope_tracker::ScopeTracker;
pub use ast_visitor::AstVisitor;
pub use tree_walker::TreeWalker;
pub use strategy::{ExtractionStrategy, SequentialExtractor, ParallelExtractor};
pub use pipeline::ExtractionPipeline;

use crate::types::*;

// ── Core extraction types (sin cambios) ──

#[derive(Debug, Clone)]
pub struct ExtractionResult {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub unresolved_references: Vec<UnresolvedRef>,
    pub errors: Vec<ExtractionError>,
}

impl ExtractionResult {
    pub fn default(file_path: &str, language: Language) -> Self {
        let file_id = generate_node_id(&NodeKind::File, file_path, file_path, 0);
        ExtractionResult {
            nodes: vec![Node {
                id: file_id, kind: NodeKind::File, name: file_path.to_string(),
                qualified_name: file_path.to_string(), file_path: file_path.to_string(), language,
                start_line: 0, end_line: 0, start_column: 0, end_column: 0,
                docstring: None, signature: None, visibility: None,
                is_exported: false, is_async: false, is_static: false, is_abstract: false,
                decorators: None, type_parameters: None, updated_at: crate::util::now_ts(),
            }],
            edges: Vec::new(),
            unresolved_references: Vec::new(),
            errors: Vec::new(),
        }
    }
}

pub fn extraction_error(msg: &str) -> ExtractionError {
    ExtractionError {
        message: msg.to_string(),
        kind: ExtractionErrorKind::Other,
        line: None,
        column: None,
    }
}

/// Trait for per-language extractors (backward compat).
pub trait LanguageExtractor: Send + Sync {
    fn extract(&self, source: &str, file_path: &str, language: Language, framework_names: &[String], parser: &mut tree_sitter::Parser) -> ExtractionResult;
}

// ── Language detection ──

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

// ── Node ID generation ──

pub fn generate_node_id(kind: &NodeKind, name: &str, file_path: &str, line: u32) -> String {
    use sha2::{Sha256, Digest};
    let raw = format!("{file_path}:{}:{name}:{line}", kind.as_str());
    let hash = Sha256::digest(raw.as_bytes());
    let hex = format!("{hash:x}");
    format!("{}:{}", kind.as_str(), &hex[..32])
}

pub fn make_empty_result(file_path: &str, language: Language) -> ExtractionResult {
    ExtractionResult::default(file_path, language)
}

// ── Helpers for AST traversal ──

pub mod tree_sitter_helpers {
    use tree_sitter::Node;

    pub fn get_node_text<'a>(node: &Node, source: &'a [u8]) -> &'a str {
        node.utf8_text(source).unwrap_or("")
    }

    pub fn get_child_by_field_name<'a>(node: &Node<'a>, field: &str) -> Option<Node<'a>> {
        node.child_by_field_name(field)
    }

    pub fn get_named_children<'a>(node: &Node<'a>) -> Vec<Node<'a>> {
        let mut cursor = node.walk();
        node.named_children(&mut cursor).collect()
    }

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

    pub fn extract_name<'a>(node: &Node<'a>, name_field: &str, source: &'a [u8]) -> String {
        if let Some(name_node) = get_child_by_field_name(node, name_field) {
            get_node_text(&name_node, source).to_string()
        } else {
            get_node_text(node, source).to_string()
        }
    }
}

// ── Backward compat: old public functions ──

/// Sequential extraction: un parser por file, un thread.
pub fn extract_files_from_disk(
    paths: &[String],
    root_dir: &str,
    framework_names: &[String],
) -> (Vec<ExtractionResult>, Vec<String>) {
    let pipeline = ExtractionPipeline::new(Box::new(SequentialExtractor));
    pipeline.run(paths, root_dir, framework_names)
}

/// Parallel extraction: un parser por chunk de files, distribuido en rayon.
pub fn extract_files_from_disk_parallel(
    paths: &[String],
    root_dir: &str,
    framework_names: &[String],
    num_workers: usize,
) -> (Vec<ExtractionResult>, Vec<String>) {
    let pipeline = ExtractionPipeline::new(Box::new(ParallelExtractor { num_workers }));
    pipeline.run(paths, root_dir, framework_names)
}

/// Extrae files pre-leídos (backward compat para napi `extract_files`).
pub fn extract_files_parallel(
    files: &[(String, String)],
    framework_names: &[String],
) -> Vec<ExtractionResult> {
    let sf: Vec<SourceFile> = files.iter().map(|(path, content)| {
        SourceFile::new(path.clone(), content.clone(), 0, 0)
    }).collect();
    let extractor = SequentialExtractor;
    extractor.execute(&sf, framework_names)
}

/// Extrae un solo file (backward compat para tests / llamadas directas).
pub fn extract_file(
    file_path: &str,
    source: &str,
    framework_names: &[String],
    parser: &mut tree_sitter::Parser,
) -> ExtractionResult {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    match catch_unwind(AssertUnwindSafe(|| {
        let language = detect_language(file_path);
        if let Some(extractor) = languages::get_extractor(&language) {
            extractor.extract(source, file_path, language, framework_names, parser)
        } else {
            make_empty_result(file_path, language)
        }
    })) {
        Ok(r) => r,
        Err(panic_payload) => {
            let mut r = make_empty_result(file_path, detect_language(file_path));
            let msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic".to_string()
            };
            r.errors.push(ExtractionError {
                message: format!("panic during extraction: {msg}"),
                kind: ExtractionErrorKind::FatalPanic,
                line: None,
                column: None,
            });
            r
        }
    }
}
