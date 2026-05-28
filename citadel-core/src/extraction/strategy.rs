use rayon::prelude::*;

use super::source_file::SourceFile;
use super::{detect_language, make_empty_result, extract_file, ExtractionResult};
use crate::types::*;

/// Strategy pattern: el pipeline delega la extracción a una estrategia
/// intercambiable (secuencial vs paralela).
pub trait ExtractionStrategy: Send + Sync {
    fn execute(&self, files: &[SourceFile], framework_names: &[String]) -> Vec<ExtractionResult>;
}

/// Extracción secuencial — un solo parser reusado.
/// Útil para repos pequeños o debug.
pub struct SequentialExtractor;

impl ExtractionStrategy for SequentialExtractor {
    fn execute(&self, files: &[SourceFile], fw: &[String]) -> Vec<ExtractionResult> {
        files.iter().map(|f| extract_single(f, fw)).collect()
    }
}

/// Extracción paralela vía rayon + un parser por chunk.
///
/// `par_chunks` distribuye batches de files entre threads.
/// Cada thread tiene su propio `tree_sitter::Parser` — thread-safe
/// porque cada parser se usa desde un solo thread.
pub struct ParallelExtractor {
    pub num_workers: usize,
}

impl ExtractionStrategy for ParallelExtractor {
    fn execute(&self, files: &[SourceFile], fw: &[String]) -> Vec<ExtractionResult> {
        // Si hay menos files que workers, degrada a secuencial
        if files.len() <= self.num_workers || self.num_workers <= 1 {
            return files.iter().map(|f| extract_single(f, fw)).collect();
        }

        let chunk = (files.len() + self.num_workers - 1) / self.num_workers;
        let chunk = chunk.max(1);

        files
            .par_chunks(chunk)
            .flat_map(|c| {
                // Un parser por chunk — el chunk corre en un solo thread
                let mut parser = tree_sitter::Parser::new();
                c.iter()
                    .map(|f| extract_one(f, &mut parser, fw))
                    .collect::<Vec<_>>()
            })
            .collect()
    }
}

// ── Helpers ──

fn extract_single(f: &SourceFile, fw: &[String]) -> ExtractionResult {
    if let Some(ref err) = f.error {
        return error_result(&f.relative_path, err);
    }
    let mut parser = tree_sitter::Parser::new();
    extract_one(f, &mut parser, fw)
}

fn extract_one(f: &SourceFile, parser: &mut tree_sitter::Parser, fw: &[String]) -> ExtractionResult {
    extract_file(&f.relative_path, &f.content, fw, parser)
}

fn error_result(path: &str, msg: &str) -> ExtractionResult {
    let lang = detect_language(path);
    let mut r = make_empty_result(path, lang);
    r.errors.push(ExtractionError {
        message: msg.to_string(),
        kind: ExtractionErrorKind::Other,
        line: None,
        column: None,
    });
    r
}
