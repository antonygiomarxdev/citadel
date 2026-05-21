use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
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
    EnumMember,
    TypeAlias,
    Namespace,
    Parameter,
    Import,
    Export,
    Route,
    Component,
}

impl NodeKind {
    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "file" => NodeKind::File,
            "module" => NodeKind::Module,
            "class" => NodeKind::Class,
            "struct" => NodeKind::Struct,
            "interface" => NodeKind::Interface,
            "trait" => NodeKind::Trait,
            "protocol" => NodeKind::Protocol,
            "function" => NodeKind::Function,
            "method" => NodeKind::Method,
            "property" => NodeKind::Property,
            "field" => NodeKind::Field,
            "variable" => NodeKind::Variable,
            "constant" => NodeKind::Constant,
            "enum" => NodeKind::Enum,
            "enum_member" => NodeKind::EnumMember,
            "type_alias" => NodeKind::TypeAlias,
            "namespace" => NodeKind::Namespace,
            "parameter" => NodeKind::Parameter,
            "import" => NodeKind::Import,
            "export" => NodeKind::Export,
            "route" => NodeKind::Route,
            "component" => NodeKind::Component,
            _ => return None,
        })
    }

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Contains,
    Calls,
    Imports,
    Exports,
    Extends,
    Implements,
    References,
    TypeOf,
    Returns,
    Instantiates,
    Overrides,
    Decorates,
}

impl EdgeKind {
    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "contains" => EdgeKind::Contains,
            "calls" => EdgeKind::Calls,
            "imports" => EdgeKind::Imports,
            "exports" => EdgeKind::Exports,
            "extends" => EdgeKind::Extends,
            "implements" => EdgeKind::Implements,
            "references" => EdgeKind::References,
            "type_of" => EdgeKind::TypeOf,
            "returns" => EdgeKind::Returns,
            "instantiates" => EdgeKind::Instantiates,
            "overrides" => EdgeKind::Overrides,
            "decorates" => EdgeKind::Decorates,
            _ => return None,
        })
    }

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

#[derive(Debug, Clone, PartialEq)]
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
    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "typescript" => Language::TypeScript,
            "javascript" => Language::JavaScript,
            "tsx" => Language::Tsx,
            "jsx" => Language::Jsx,
            "python" => Language::Python,
            "go" => Language::Go,
            "rust" => Language::Rust,
            "java" => Language::Java,
            "c" => Language::C,
            "cpp" => Language::Cpp,
            "csharp" => Language::CSharp,
            "php" => Language::Php,
            "ruby" => Language::Ruby,
            "swift" => Language::Swift,
            "kotlin" => Language::Kotlin,
            "dart" => Language::Dart,
            "svelte" => Language::Svelte,
            "vue" => Language::Vue,
            "liquid" => Language::Liquid,
            "pascal" => Language::Pascal,
            "scala" => Language::Scala,
            "lua" => Language::Lua,
            "luau" => Language::Luau,
            "unknown" => Language::Unknown,
            _ => return None,
        })
    }

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
}

impl Serialize for Language {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Language {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct LanguageVisitor;
        impl<'de> serde::de::Visitor<'de> for LanguageVisitor {
            type Value = Language;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a language string")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Language, E> {
                Language::from_str(v).ok_or_else(|| E::unknown_variant(v, &[
                    "typescript", "javascript", "tsx", "jsx", "python", "go", "rust",
                    "java", "c", "cpp", "csharp", "php", "ruby", "swift", "kotlin",
                    "dart", "svelte", "vue", "liquid", "pascal", "scala", "lua", "luau", "unknown",
                ]))
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize { 50 }

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
