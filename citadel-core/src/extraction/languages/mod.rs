// Each language extractor is implemented in its own module.
// Currently only TypeScript and Python have fully native Rust extractors.
// The remaining languages (go, rust, java, c, cpp, csharp, php, ruby, kotlin)
// are stubbed out; their grammars are in Cargo.toml but the extractors
// will be activated in a follow-up PR.

pub mod generic;
pub mod typescript;
pub mod python;
pub mod go;
pub mod rust;
pub mod java;

pub use typescript::TypeScriptExtractor;
pub use python::PythonExtractor;
pub use go::GoExtractor;
pub use rust::RustExtractor;
pub use java::JavaExtractor;

use super::LanguageExtractor;
use crate::types::Language;

/// Returns the extractor for a given language, or None if not yet implemented.
pub fn get_extractor(language: &Language) -> Option<Box<dyn LanguageExtractor>> {
    match language {
        Language::TypeScript | Language::Tsx => Some(Box::new(TypeScriptExtractor)),
        Language::JavaScript | Language::Jsx => Some(Box::new(TypeScriptExtractor)),
        Language::Python => Some(Box::new(PythonExtractor)),
        Language::Go => Some(Box::new(GoExtractor)),
        Language::Rust => Some(Box::new(RustExtractor)),
        Language::Java => Some(Box::new(JavaExtractor)),
        _ => None,
    }
}
