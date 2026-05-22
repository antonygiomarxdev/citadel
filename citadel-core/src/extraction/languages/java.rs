#![allow(clippy::ptr_arg, clippy::too_many_arguments)]
use crate::extraction::*;
use crate::extraction::languages::generic::{walk, ExtractorConfig};
use crate::types::*;
use tree_sitter::Parser;

pub struct JavaExtractor;

impl LanguageExtractor for JavaExtractor {
    fn extract(&self, source: &str, file_path: &str, language: Language, _framework_names: &[String]) -> ExtractionResult {
        let mut result = make_empty_result(file_path, language.clone());

        let mut parser = Parser::new();
        if let Err(e) = parser.set_language(&tree_sitter_java::LANGUAGE.into()) {
            result.errors.push(ExtractionError { message: format!("set_language: {e}"), kind: ExtractionErrorKind::TreeSitterError, line: None, column: None });
            return result;
        }

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => { result.errors.push(ExtractionError { message: "parse failed".into(), kind: ExtractionErrorKind::ParseError, line: None, column: None }); return result; }
        };

        let config = ExtractorConfig {
            function_kinds: vec!["method_declaration".into(), "constructor_declaration".into()],
            class_kinds: vec!["class_declaration".into()],
            interface_kinds: vec!["interface_declaration".into()],
            struct_kinds: vec![],
            variable_kinds: vec!["field_declaration".into(), "variable_declarator".into()],
            import_kinds: vec!["import_declaration".into()],
            call_kinds: vec!["method_invocation".into(), "object_creation_expression".into()],
            name_field: "name",
        };
        walk(&tree, source, file_path, language, &mut result, &config);
        result
    }
}
