use crate::resolution::{ResolvedRef, ResolutionContext};
use crate::types::*;

/// Extension resolution order by language.
/// Mirrors the TS EXTENSION_RESOLUTION map.
#[allow(dead_code)]
fn extension_order(language: &str) -> &[&str] {
    match language {
        "typescript" => &[".ts", ".tsx", ".d.ts", ".js", ".jsx", "/index.ts", "/index.tsx", "/index.js"],
        "javascript" => &[".js", ".jsx", ".mjs", ".cjs", "/index.js", "/index.jsx"],
        "tsx" => &[".tsx", ".ts", ".d.ts", ".js", ".jsx", "/index.tsx", "/index.ts", "/index.js"],
        "jsx" => &[".jsx", ".js", "/index.jsx", "/index.js"],
        "python" => &[".py", "/__init__.py"],
        "go" => &[".go"],
        "rust" => &[".rs", "/mod.rs"],
        "java" => &[".java"],
        "csharp" => &[".cs"],
        "php" => &[".php"],
        "ruby" => &[".rb"],
        _ => &[],
    }
}

/// Try to resolve via import path matching.
/// Returns None if the reference can't be resolved via imports.
pub fn resolve_import(r#ref: &UnresolvedRef, ctx: &dyn ResolutionContext) -> Option<ResolvedRef> {
    // Only handle 'imports' references for import resolution
    if r#ref.reference_kind != "imports" {
        return None;
    }

    let import_name = &r#ref.reference_name;
    let file_path = &r#ref.file_path;

    // Look for nodes with this name that match known import patterns
    let nodes = ctx.get_nodes_by_name(import_name).ok()?;
    if nodes.is_empty() {
        return None;
    }

    // Score candidates by file path proximity
    let from_dir = match std::path::Path::new(file_path).parent() {
        Some(d) => d.to_string_lossy().to_string(),
        None => String::new(),
    };

    let mut best_node: Option<&Node> = None;
    let mut best_score = 0i32;

    for node in &nodes {
        let mut score = 0i32;
        let node_dir = match std::path::Path::new(&node.file_path).parent() {
            Some(d) => d.to_string_lossy().to_string(),
            None => continue,
        };

        if &node.file_path == file_path {
            score += 200;
        } else if node_dir == from_dir {
            score += 100;
        } else {
            // Shared directory segments
            let from_parts: Vec<&str> = from_dir.split('/').collect();
            let node_parts: Vec<&str> = node_dir.split('/').collect();
            let shared = from_parts.iter().zip(node_parts.iter()).take_while(|(a, b)| a == b).count();
            score += shared as i32 * 15;
        }

        if node.language.as_str() == r#ref.language {
            score += 50;
        } else {
            score -= 80;
        }

        if node.is_exported {
            score += 10;
        }

        if score > best_score {
            best_score = score;
            best_node = Some(node);
        }
    }

    best_node.map(|node| {
        let confidence = if best_score >= 200 { 0.95 } else if best_score >= 100 { 0.85 } else { 0.7 };
        ResolvedRef {
            from_node_id: r#ref.from_node_id.clone(),
            reference_name: r#ref.reference_name.clone(),
            reference_kind: r#ref.reference_kind.clone(),
            target_node_id: node.id.clone(),
            confidence,
            resolved_by: "import".to_string(),
        }
    })
}

/// Check if an import path is external (npm package, stdlib, etc.)
pub fn is_external_import(import_path: &str, language: &str, _ctx: &dyn ResolutionContext) -> bool {
    if import_path.starts_with('.') {
        return false;
    }

    match language {
        "typescript" | "javascript" | "tsx" | "jsx" => {
            let builtins = ["fs", "path", "os", "crypto", "http", "https", "url", "util", "events", "stream", "child_process", "buffer"];
            if builtins.contains(&import_path) {
                return true;
            }
            if !import_path.starts_with('@') && !import_path.starts_with("src/") {
                return true;
            }
        }
        "python" => {
            let stdlibs = ["os", "sys", "json", "re", "math", "datetime", "collections", "typing", "pathlib", "logging"];
            if stdlibs.contains(&import_path.split('.').next().unwrap_or("")) {
                return true;
            }
        }
        "go"
            if !import_path.starts_with('.') && !import_path.contains("/internal/") => {
                return true;
            }
        _ => {}
    }
    false
}
