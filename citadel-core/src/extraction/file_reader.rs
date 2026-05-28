use std::io::Read;
use rayon::prelude::*;

use super::source_file::SourceFile;

/// Reads source files from disk in parallel.
/// Each file's SHA256 and metadata (size, mtime) are collected during the read phase.
pub struct FileReader;

impl FileReader {
    pub fn read_all(paths: &[String], root_dir: &str) -> Vec<SourceFile> {
        paths.par_iter().map(|fp| {
            let full_path = if root_dir.is_empty() {
                fp.clone()
            } else {
                format!("{}/{}", root_dir.trim_end_matches('/'), fp)
            };
            match std::fs::File::open(&full_path) {
                Ok(mut file) => {
                    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
                    let modified_at = file.metadata().ok()
                        .map(|m| SourceFile::modified_at_from_meta(&m))
                        .unwrap_or(0);
                    let mut content = String::new();
                    match file.read_to_string(&mut content) {
                        Ok(_) => SourceFile::new(fp.clone(), content, size, modified_at),
                        Err(e) => SourceFile::error(fp.clone(), format!("Failed to read file: {e}")),
                    }
                }
                Err(e) => SourceFile::error(fp.clone(), format!("Failed to open file: {e}")),
            }
        }).collect()
    }
}
