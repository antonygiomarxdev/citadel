/// Path alias system for import resolution.
/// Parses tsconfig.json paths and provides alias expansion.

#[derive(Debug, Clone)]
pub struct AliasPattern {
    pub prefix: String,
    pub suffix: String,
    pub has_wildcard: bool,
    pub replacements: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AliasMap {
    pub base_url: String,
    pub patterns: Vec<AliasPattern>,
}

impl AliasMap {
    /// Apply alias patterns to an import path, returning candidate filesystem paths.
    pub fn apply(&self, import_path: &str) -> Vec<String> {
        let mut candidates = Vec::new();

        for pattern in &self.patterns {
            if !import_path.starts_with(&pattern.prefix) {
                continue;
            }

            let remainder = &import_path[pattern.prefix.len()..];
            for replacement in &pattern.replacements {
                let mut candidate = replacement.clone();
                if pattern.has_wildcard {
                    candidate = candidate.replace('*', remainder);
                } else {
                    candidate.push_str(remainder);
                }
                if !candidate.starts_with('/') {
                    candidate = format!("{}/{}", self.base_url, candidate);
                }
                candidates.push(candidate.trim_start_matches('/').to_string());
            }
        }

        candidates
    }

    /// Hard-coded fallback aliases for projects without tsconfig paths.
    pub fn fallback_aliases(import_path: &str) -> Vec<String> {
        let path = import_path.trim_start_matches('/');
        let mut candidates = Vec::new();

        if let Some(stripped) = path.strip_prefix('@') {
            candidates.push(format!("src/{stripped}"));
        }
        if let Some(stripped) = path.strip_prefix('~') {
            candidates.push(format!("src/{stripped}"));
        }

        candidates.push(path.to_string());
        candidates
    }
}

impl Default for AliasMap {
    fn default() -> Self {
        AliasMap {
            base_url: ".".to_string(),
            patterns: Vec::new(),
        }
    }
}
