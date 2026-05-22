use std::str::FromStr;

use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

use crate::constants::*;

#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, EnumString, Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum NodeKind {
    File,
    Module,
    Class,
    Struct,
    Interface,
    Trait,
    Protocol,
    Function,
    Method,
    Property,
    Field,
    Variable,
    Constant,
    Enum,
    #[strum(serialize = "enum_member")]
    EnumMember,
    #[strum(serialize = "type_alias")]
    TypeAlias,
    Namespace,
    Parameter,
    Import,
    Export,
    Route,
    Component,
}

impl NodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeKind::File => "file",
            NodeKind::Module => "module",
            NodeKind::Class => "class",
            NodeKind::Struct => "struct",
            NodeKind::Interface => "interface",
            NodeKind::Trait => "trait",
            NodeKind::Protocol => "protocol",
            NodeKind::Function => "function",
            NodeKind::Method => "method",
            NodeKind::Property => "property",
            NodeKind::Field => "field",
            NodeKind::Variable => "variable",
            NodeKind::Constant => "constant",
            NodeKind::Enum => "enum",
            NodeKind::EnumMember => "enum_member",
            NodeKind::TypeAlias => "type_alias",
            NodeKind::Namespace => "namespace",
            NodeKind::Parameter => "parameter",
            NodeKind::Import => "import",
            NodeKind::Export => "export",
            NodeKind::Route => "route",
            NodeKind::Component => "component",
        }
    }
}

#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, EnumString, Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum EdgeKind {
    Contains,
    Calls,
    Imports,
    Exports,
    Extends,
    Implements,
    References,
    #[strum(serialize = "type_of")]
    TypeOf,
    Returns,
    Instantiates,
    Overrides,
    Decorates,
}

impl EdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EdgeKind::Contains => "contains",
            EdgeKind::Calls => "calls",
            EdgeKind::Imports => "imports",
            EdgeKind::Exports => "exports",
            EdgeKind::Extends => "extends",
            EdgeKind::Implements => "implements",
            EdgeKind::References => "references",
            EdgeKind::TypeOf => "type_of",
            EdgeKind::Returns => "returns",
            EdgeKind::Instantiates => "instantiates",
            EdgeKind::Overrides => "overrides",
            EdgeKind::Decorates => "decorates",
        }
    }
}

#[derive(Debug, Clone, PartialEq, EnumString, Display)]
#[strum(serialize_all = "lowercase")]
pub enum Language {
    TypeScript,
    JavaScript,
    Tsx,
    Jsx,
    Python,
    Go,
    Rust,
    Java,
    C,
    Cpp,
    #[strum(serialize = "csharp")]
    CSharp,
    Php,
    Ruby,
    Swift,
    Kotlin,
    Dart,
    Svelte,
    Vue,
    Liquid,
    Pascal,
    Scala,
    Lua,
    Luau,
    Unknown,
}

impl Language {
    pub fn as_str(&self) -> &'static str {
        match self {
            Language::TypeScript => "typescript",
            Language::JavaScript => "javascript",
            Language::Tsx => "tsx",
            Language::Jsx => "jsx",
            Language::Python => "python",
            Language::Go => "go",
            Language::Rust => "rust",
            Language::Java => "java",
            Language::C => "c",
            Language::Cpp => "cpp",
            Language::CSharp => "csharp",
            Language::Php => "php",
            Language::Ruby => "ruby",
            Language::Swift => "swift",
            Language::Kotlin => "kotlin",
            Language::Dart => "dart",
            Language::Svelte => "svelte",
            Language::Vue => "vue",
            Language::Liquid => "liquid",
            Language::Pascal => "pascal",
            Language::Scala => "scala",
            Language::Lua => "lua",
            Language::Luau => "luau",
            Language::Unknown => "unknown",
        }
    }

    pub fn file_extensions(&self) -> &[&str] {
        match self {
            Language::TypeScript => &[".ts"],
            Language::JavaScript => &[".js", ".mjs", ".cjs"],
            Language::Tsx => &[".tsx"],
            Language::Jsx => &[".jsx"],
            Language::Python => &[".py", ".pyi", ".pyx"],
            Language::Go => &[".go"],
            Language::Rust => &[".rs"],
            Language::Java => &[".java"],
            Language::C => &[".c", ".h"],
            Language::Cpp => &[".cpp", ".cc", ".hpp", ".hxx"],
            Language::CSharp => &[".cs"],
            Language::Php => &[".php"],
            Language::Ruby => &[".rb"],
            Language::Swift => &[".swift"],
            Language::Kotlin => &[".kt", ".kts"],
            Language::Dart => &[".dart"],
            Language::Svelte => &[".svelte"],
            Language::Vue => &[".vue"],
            Language::Liquid => &[".liquid"],
            Language::Pascal => &[".pas", ".pp"],
            Language::Scala => &[".scala", ".sc"],
            Language::Lua => &[".lua"],
            Language::Luau => &[".luau"],
            Language::Unknown => &[],
        }
    }

    /// Detect language from a file extension (including leading dot).
    pub fn from_extension(ext: &str) -> Option<Language> {
        let ext_lower = ext.to_lowercase();
        for lang in Language::all() {
            if lang.file_extensions().contains(&ext_lower.as_str()) {
                return Some(lang.clone());
            }
        }
        None
    }

    /// All concrete languages (excludes Unknown).
    pub fn all() -> &'static [Language] {
        &[
            Language::TypeScript,
            Language::JavaScript,
            Language::Tsx,
            Language::Jsx,
            Language::Python,
            Language::Go,
            Language::Rust,
            Language::Java,
            Language::C,
            Language::Cpp,
            Language::CSharp,
            Language::Php,
            Language::Ruby,
            Language::Swift,
            Language::Kotlin,
            Language::Dart,
            Language::Svelte,
            Language::Vue,
            Language::Liquid,
            Language::Pascal,
            Language::Scala,
            Language::Lua,
            Language::Luau,
        ]
    }
}

impl Serialize for Language {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Language {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct LanguageVisitor;
        impl<'de> serde::de::Visitor<'de> for LanguageVisitor {
            type Value = Language;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a language string")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Language, E> {
                Language::from_str(v).map_err(|_| {
                    static VALID: &[&str] = &[
                        "typescript", "javascript", "tsx", "jsx", "python", "go", "rust",
                        "java", "c", "cpp", "csharp", "php", "ruby", "swift", "kotlin",
                        "dart", "svelte", "vue", "liquid", "pascal", "scala", "lua", "luau", "unknown",
                    ];
                    E::unknown_variant(v, VALID)
                })
            }
        }
        deserializer.deserialize_str(LanguageVisitor)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub name: String,
    pub qualified_name: String,
    pub file_path: String,
    pub language: Language,
    pub start_line: u32,
    pub end_line: u32,
    pub start_column: u32,
    pub end_column: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docstring: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
    #[serde(default)]
    pub is_exported: bool,
    #[serde(default)]
    pub is_async: bool,
    #[serde(default)]
    pub is_static: bool,
    #[serde(default)]
    pub is_abstract: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decorators: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<Vec<String>>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub kind: EdgeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub path: String,
    pub content_hash: String,
    pub language: Language,
    pub size: u64,
    pub modified_at: i64,
    pub indexed_at: i64,
    pub node_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<ExtractionError>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionError {
    pub message: String,
    pub kind: ExtractionErrorKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionErrorKind {
    ParseError,
    UnsupportedSyntax,
    FatalPanic,
    StackOverflow,
    InvalidSpan,
    TreeSitterError,
    Other,
}

impl ExtractionErrorKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ExtractionErrorKind::ParseError => "parse_error",
            ExtractionErrorKind::UnsupportedSyntax => "unsupported_syntax",
            ExtractionErrorKind::FatalPanic => "fatal_panic",
            ExtractionErrorKind::StackOverflow => "stack_overflow",
            ExtractionErrorKind::InvalidSpan => "invalid_span",
            ExtractionErrorKind::TreeSitterError => "tree_sitter_error",
            ExtractionErrorKind::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedRef {
    pub from_node_id: String,
    pub reference_name: String,
    pub reference_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidates: Option<serde_json::Value>,
    pub file_path: String,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub node: Node,
    pub score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlights: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kinds: Option<Vec<NodeKind>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub languages: Option<Vec<Language>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_patterns: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_patterns: Option<Vec<String>>,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

fn default_search_limit() -> usize {
    DEFAULT_SEARCH_LIMIT
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub node_count: u64,
    pub edge_count: u64,
    pub file_count: u64,
    pub nodes_by_kind: std::collections::HashMap<String, u64>,
    pub edges_by_kind: std::collections::HashMap<String, u64>,
    pub files_by_language: std::collections::HashMap<String, u64>,
    pub db_size_bytes: u64,
    pub last_updated: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaVersion {
    pub version: i32,
    pub applied_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TraversalDirection {
    Outgoing,
    Incoming,
    #[default]
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraversalOptions {
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
    #[serde(default)]
    pub edge_kinds: Vec<EdgeKind>,
    #[serde(default)]
    pub node_kinds: Vec<NodeKind>,
    #[serde(default)]
    pub direction: TraversalDirection,
    #[serde(default = "default_traversal_limit")]
    pub limit: usize,
    #[serde(default = "default_include_start")]
    pub include_start: bool,
}

fn default_max_depth() -> usize {
    DEFAULT_MAX_DEPTH
}
fn default_traversal_limit() -> usize {
    DEFAULT_TRAVERSAL_LIMIT
}
fn default_include_start() -> bool {
    true
}

impl Default for TraversalOptions {
    fn default() -> Self {
        TraversalOptions {
            max_depth: DEFAULT_MAX_DEPTH,
            edge_kinds: Vec::new(),
            node_kinds: Vec::new(),
            direction: TraversalDirection::Both,
            limit: DEFAULT_TRAVERSAL_LIMIT,
            include_start: true,
        }
    }
}
