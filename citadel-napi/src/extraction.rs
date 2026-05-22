use citadel_core::extraction;
use citadel_core::types::{Edge, ExtractionError, Node, UnresolvedRef};

/// Result of extracting one file, returned to JS.
#[napi(object)]
#[derive(Debug, Clone)]
pub struct JsExtractionResult {
    pub nodes: Vec<JsNode>,
    pub edges: Vec<JsEdge>,
    pub unresolved_references: Vec<JsUnresolvedRef>,
    pub errors: Vec<JsExtractionError>,
}

#[napi(object)]
#[derive(Debug, Clone)]
pub struct JsNode {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub qualified_name: String,
    pub file_path: String,
    pub language: String,
    pub start_line: u32,
    pub end_line: u32,
    pub start_column: u32,
    pub end_column: u32,
    pub docstring: Option<String>,
    pub signature: Option<String>,
    pub visibility: Option<String>,
    pub is_exported: bool,
    pub is_async: bool,
    pub is_static: bool,
    pub is_abstract: bool,
    pub decorators: Option<Vec<String>>,
    pub type_parameters: Option<Vec<String>>,
}

#[napi(object)]
#[derive(Debug, Clone)]
pub struct JsEdge {
    pub source: String,
    pub target: String,
    pub edge_type: String,
    pub metadata: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub provenance: Option<String>,
}

#[napi(object)]
#[derive(Debug, Clone)]
pub struct JsUnresolvedRef {
    pub from_node_id: String,
    pub reference_name: String,
    pub reference_kind: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub candidates: Option<String>,
    pub file_path: String,
    pub language: String,
}

#[napi(object)]
#[derive(Debug, Clone)]
pub struct JsExtractionError {
    pub message: String,
    pub kind: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

fn node_to_js(node: &Node) -> JsNode {
    JsNode {
        id: node.id.clone(),
        kind: node.kind.as_str().to_string(),
        name: node.name.clone(),
        qualified_name: node.qualified_name.clone(),
        file_path: node.file_path.clone(),
        language: node.language.as_str().to_string(),
        start_line: node.start_line,
        end_line: node.end_line,
        start_column: node.start_column,
        end_column: node.end_column,
        docstring: node.docstring.clone(),
        signature: node.signature.clone(),
        visibility: node.visibility.clone(),
        is_exported: node.is_exported,
        is_async: node.is_async,
        is_static: node.is_static,
        is_abstract: node.is_abstract,
        decorators: node.decorators.clone(),
        type_parameters: node.type_parameters.clone(),
    }
}

fn edge_to_js(edge: &Edge) -> JsEdge {
    JsEdge {
        source: edge.source.clone(),
        target: edge.target.clone(),
        edge_type: edge.kind.as_str().to_string(),
        metadata: edge.metadata.as_ref().map(|v| v.to_string()),
        line: edge.line,
        column: edge.column,
        provenance: edge.provenance.clone(),
    }
}

fn unresolved_to_js(ur: &UnresolvedRef) -> JsUnresolvedRef {
    JsUnresolvedRef {
        from_node_id: ur.from_node_id.clone(),
        reference_name: ur.reference_name.clone(),
        reference_kind: ur.reference_kind.clone(),
        line: ur.line,
        column: ur.column,
        candidates: ur.candidates.as_ref().map(|v| v.to_string()),
        file_path: ur.file_path.clone(),
        language: ur.language.clone(),
    }
}

fn error_to_js(err: &ExtractionError) -> JsExtractionError {
    JsExtractionError {
        message: err.message.clone(),
        kind: err.kind.as_str().to_string(),
        line: err.line,
        column: err.column,
    }
}

/// Extracts symbols from multiple files in parallel using native Rust tree-sitter.
/// Each file is parsed independently — rayon par_iter() distributes across CPU cores.
/// This replaces the WASM-based tree-sitter extraction in TypeScript.
#[napi]
pub fn extract_files(
    files: Vec<Vec<String>>,
    framework_names: Vec<String>,
) -> Vec<JsExtractionResult> {
    let tuples: Vec<(String, String)> = files
        .into_iter()
        .filter(|f| f.len() >= 2)
        .map(|f| (f[0].clone(), f[1].clone()))
        .collect();

    extraction::extract_files_parallel(&tuples, &framework_names)
        .into_iter()
        .map(|r| JsExtractionResult {
            nodes: r.nodes.iter().map(node_to_js).collect(),
            edges: r.edges.iter().map(edge_to_js).collect(),
            unresolved_references: r.unresolved_references.iter().map(unresolved_to_js).collect(),
            errors: r.errors.iter().map(error_to_js).collect(),
        })
        .collect()
}
