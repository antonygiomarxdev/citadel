#![allow(clippy::ptr_arg, clippy::too_many_arguments)]
use crate::extraction::*;
use crate::extraction::languages::generic::{walk, ExtractorConfig};
use crate::types::*;

pub struct GoExtractor;

impl LanguageExtractor for GoExtractor {
    fn extract(&self, source: &str, file_path: &str, language: Language, _framework_names: &[String], parser: &mut tree_sitter::Parser) -> ExtractionResult {
        let mut result = make_empty_result(file_path, language.clone());

        if let Err(e) = parser.set_language(&tree_sitter_go::LANGUAGE.into()) {
            result.errors.push(ExtractionError { message: format!("set_language: {e}"), kind: ExtractionErrorKind::TreeSitterError, line: None, column: None });
            return result;
        }

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => { result.errors.push(ExtractionError { message: "parse failed".into(), kind: ExtractionErrorKind::ParseError, line: None, column: None }); return result; }
        };

        let config = ExtractorConfig {
            function_kinds: vec!["function_declaration".into(), "method_declaration".into()],
            class_kinds: vec!["struct_type".into()],
            interface_kinds: vec!["interface_type".into()],
            struct_kinds: vec!["struct_type".into()],
            variable_kinds: vec!["var_declaration".into(), "const_declaration".into(), "short_var_declaration".into()],
            import_kinds: vec!["import_declaration".into()],
            call_kinds: vec!["call_expression".into()],
            name_field: "name",
        };
        walk(&tree, source, file_path, language, &mut result, &config);
        result
    }
}
