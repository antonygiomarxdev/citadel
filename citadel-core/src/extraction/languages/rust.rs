#![allow(clippy::ptr_arg, clippy::too_many_arguments)]
use crate::extraction::*;
use crate::extraction::languages::generic::{walk, ExtractorConfig};
use crate::types::*;

pub struct RustExtractor;

impl LanguageExtractor for RustExtractor {
    fn extract(&self, source: &str, file_path: &str, language: Language, _framework_names: &[String], parser: &mut tree_sitter::Parser) -> ExtractionResult {
        let mut result = make_empty_result(file_path, language.clone());

        if let Err(e) = parser.set_language(&tree_sitter_rust::LANGUAGE.into()) {
            result.errors.push(ExtractionError { message: format!("set_language: {e}"), kind: ExtractionErrorKind::TreeSitterError, line: None, column: None });
            return result;
        }

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => { result.errors.push(ExtractionError { message: "parse failed".into(), kind: ExtractionErrorKind::ParseError, line: None, column: None }); return result; }
        };

        let config = ExtractorConfig {
            function_kinds: vec!["function_item".into()],
            class_kinds: vec![],
            interface_kinds: vec!["trait_item".into()],
            struct_kinds: vec!["struct_item".into()],
            variable_kinds: vec!["let_declaration".into()],
            import_kinds: vec!["use_declaration".into()],
            call_kinds: vec!["call_expression".into(), "macro_invocation".into()],
            name_field: "name",
        };
        walk(&tree, source, file_path, language, &mut result, &config);
        result
    }
}
