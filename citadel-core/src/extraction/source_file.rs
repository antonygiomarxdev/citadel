use sha2::{Digest, Sha256};

use std::time::UNIX_EPOCH;

/// A source file read from disk with its content, hash, and metadata.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub relative_path: String,
    pub content: String,
    pub sha256: String,
    pub size: u64,
    pub modified_at: i64,
    pub error: Option<String>,
}

impl SourceFile {
    pub fn new(relative_path: String, content: String, size: u64, modified_at: i64) -> Self {
        let sha256 = format!("{:x}", Sha256::digest(content.as_bytes()));
        SourceFile { relative_path, content, sha256, size, modified_at, error: None }
    }

    pub fn error(relative_path: String, message: String) -> Self {
        SourceFile {
            relative_path,
            content: String::new(),
            sha256: String::new(),
            size: 0,
            modified_at: 0,
            error: Some(message),
        }
    }

    pub fn modified_at_from_meta(meta: &std::fs::Metadata) -> i64 {
        meta.modified().ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }
}
