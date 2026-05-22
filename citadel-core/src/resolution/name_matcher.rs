use crate::resolution::{ResolvedRef, ResolutionContext};
use crate::types::*;

/// Match by exact name lookup.
/// Returns the best matching node with a confidence score.
pub fn match_name(r#ref: &UnresolvedRef, ctx: &dyn ResolutionContext) -> Option<ResolvedRef> {
    let name = &r#ref.reference_name;

    // Strategy 1: exact name match
    if let Ok(nodes) = ctx.get_nodes_by_name(name)
        && !nodes.is_empty() {
            return pick_best_node(r#ref, &nodes, "exact-match", 0.9);
        }

    // Strategy 2: qualified name match (contains :: or .)
    if name.contains("::") || name.contains('.') {
        let qn = name.replace("::", ".");
        if let Ok(nodes) = ctx.get_nodes_by_qualified_name(&qn)
            && !nodes.is_empty() {
                return pick_best_node(r#ref, &nodes, "qualified-name", 0.95);
            }
    }

    // Strategy 3: lower-case name match
    let lower = name.to_lowercase();
    if let Ok(nodes) = ctx.get_nodes_by_lower_name(&lower)
        && !nodes.is_empty() {
            return pick_best_node(r#ref, &nodes, "fuzzy", 0.5);
        }

    // Strategy 4: method call resolution (obj.method → class lookup)
    if let Some((receiver, method)) = split_method_call(name) {
        // Try direct class match: capitalize receiver
        let mut capitalized = receiver.to_string();
        if let Some(c) = capitalized.chars().next() {
            capitalized = c.to_uppercase().collect::<String>() + &capitalized[1..];
        }
        if let Ok(class_nodes) = ctx.get_nodes_by_name(&capitalized) {
            for class_node in &class_nodes {
                let children = ctx.get_nodes_in_file(&class_node.file_path).ok()?;
                for child in &children {
                    if child.name == method {
                        return Some(ResolvedRef {
                            from_node_id: r#ref.from_node_id.clone(),
                            reference_name: r#ref.reference_name.clone(),
                            reference_kind: r#ref.reference_kind.clone(),
                            target_node_id: child.id.clone(),
                            confidence: 0.85,
                            resolved_by: "instance-method".to_string(),
                        });
                    }
                }
            }
        }
    }

    None
}

/// Split "obj.method" or "Class::method" into (receiver, method).
fn split_method_call(name: &str) -> Option<(&str, &str)> {
    if let Some(pos) = name.rfind('.') {
        Some((&name[..pos], &name[pos+1..]))
    } else if let Some(pos) = name.rfind("::") {
        Some((&name[..pos], &name[pos+2..]))
    } else {
        None
    }
}

/// Pick the best node from a list based on scoring heuristics.
fn pick_best_node(r#ref: &UnresolvedRef, nodes: &[Node], strategy: &str, base_confidence: f64) -> Option<ResolvedRef> {
    if nodes.is_empty() {
        return None;
    }

    let from_file = &r#ref.file_path;
    let from_dir = std::path::Path::new(from_file)
        .parent()
        .map(|d| d.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut best_idx = 0usize;
    let mut best_score = 0i32;

    for (i, node) in nodes.iter().enumerate() {
        let mut score = 0i32;

        if &node.file_path == from_file {
            score += 100;
        } else {
            let node_dir = std::path::Path::new(&node.file_path)
                .parent()
                .map(|d| d.to_string_lossy().to_string())
                .unwrap_or_default();
            if node_dir == from_dir {
                score += 80;
            }
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

        if r#ref.reference_kind == "calls" && matches!(node.kind, NodeKind::Function | NodeKind::Method) {
            score += 25;
        }
        if r#ref.reference_kind == "instantiates" && matches!(node.kind, NodeKind::Class | NodeKind::Struct | NodeKind::Interface) {
            score += 25;
        }
        if node.is_exported {
            score += 10;
        }

        if score > best_score {
            best_score = score;
            best_idx = i;
        }
    }

    let confidence = if best_score >= 200 { 0.95 }
        else if best_score >= 100 { 0.85 }
        else if best_score >= 50 { 0.7 }
        else { base_confidence.min(0.5) };

    Some(ResolvedRef {
        from_node_id: r#ref.from_node_id.clone(),
        reference_name: r#ref.reference_name.clone(),
        reference_kind: r#ref.reference_kind.clone(),
        target_node_id: nodes[best_idx].id.clone(),
        confidence,
        resolved_by: strategy.to_string(),
    })
}
