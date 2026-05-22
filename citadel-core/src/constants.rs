//! Named constants extracted from across the codebase.
//!
//! Every magic number and magic string that was previously hardcoded
//! inline now lives here with a descriptive name and rationale comment.

// ---- Schema & Migration ----

/// Current database schema version. Increment when table structure changes.
pub const SCHEMA_VERSION: i32 = 4;

/// Size in bytes of pre-allocated Vecs for node/edge bulk inserts.
/// Balances memory usage against reallocation overhead for typical projects (~2000 files).
pub const NODE_BULK_PREALLOC: usize = 4096;

/// Size in bytes of pre-allocated Vecs for file records.
pub const FILE_BULK_PREALLOC: usize = 256;

// ---- SQLite PRAGMAs ----

/// SQLite busy timeout in milliseconds. How long to wait if the database is locked
/// by another connection before returning SQLITE_BUSY.
/// 120 seconds is generous for write-heavy CI/deployment scenarios.
pub const SQLITE_BUSY_TIMEOUT_MS: u32 = 120_000;

/// SQLite cache size in kilobytes. Negative means kibibytes instead of pages.
/// -64000 = 64 MiB — large enough to hold the entire working set for repos < 50K files.
pub const SQLITE_CACHE_SIZE_KB: i32 = -64_000;

/// SQLite memory-mapped I/O size in bytes. 256 MiB — maps the database file to avoid
/// syscall overhead on reads. Only used if the OS supports mmap (Linux/macOS).
pub const SQLITE_MMAP_SIZE: u64 = 268_435_456;

// ---- Traversal Limits ----

/// Default maximum nodes returned by a graph traversal (BFS/DFS).
/// Prevents unbounded memory consumption on highly connected graphs.
pub const DEFAULT_TRAVERSAL_LIMIT: usize = 1000;

/// Default maximum depth for graph traversal operations.
/// 100 hops from root should cover any reasonable call chain.
pub const DEFAULT_MAX_DEPTH: usize = 100;

/// Smaller max depth for type hierarchy traversal (extends/implements chains).
/// Type hierarchies are rarely deeper than 20 levels in practice.
pub const TYPE_HIERARCHY_MAX_DEPTH: usize = 20;

/// Hardcoded max_iterations safeguard in get_ancestors() loop.
/// Prevents infinite loops on malformed data.
pub const ANCESTOR_LOOP_GUARD: usize = 100;

// ---- Search Defaults ----

/// Default number of search results to return.
pub const DEFAULT_SEARCH_LIMIT: usize = 50;

/// Minimum character length for CamelCase symbol splitting.
/// Shorter fragments are noise (e.g., "on", "in", "at").
pub const MIN_CAMELCASE_TOKEN_LEN: usize = 2;

/// Minimum character length for snake_case symbol splitting.
/// Uses 3 to avoid false positives on short names like "a_b".
pub const MIN_SNAKECASE_TOKEN_LEN: usize = 3;

// ---- Context Builder Defaults ----

/// Maximum number of nodes to include in a context window.
pub const CONTEXT_MAX_NODES: usize = 20;

/// Maximum number of source files to include in a context window.
pub const CONTEXT_MAX_FILES: usize = 10;

/// Number of top entry points to display in context summary.
pub const TOP_ENTRY_POINTS: usize = 10;

/// Lines of source code to include before and after a node in context blocks.
pub const CONTEXT_SURROUNDING_LINES: usize = 3;

// ---- Resolution Scoring ----

/// Score added when reference and candidate are in the same file.
pub const SAME_FILE_BONUS: i32 = 200;

/// Score added when reference and candidate share the same directory.
pub const SAME_DIRECTORY_BONUS: i32 = 100;

/// Score multiplier per shared directory segment for multi-hop resolution.
pub const SHARED_SEGMENT_MULTIPLIER: i32 = 15;

/// Score added when reference and candidate are in the same language.
pub const SAME_LANGUAGE_BONUS: i32 = 50;

/// Score deducted when reference and candidate are in different languages.
pub const DIFFERENT_LANGUAGE_PENALTY: i32 = 80;

/// Score added when the candidate is exported/public.
pub const EXPORTED_BONUS: i32 = 10;

/// Score for a candidate being the same kind (e.g., function calling function,
/// class instantiating class).
pub const SAME_KIND_BONUS: i32 = 25;

/// Confidence threshold for high-certainty matches.
pub const CONFIDENCE_HIGH: f64 = 0.95;

/// Confidence threshold for medium-certainty matches.
pub const CONFIDENCE_MEDIUM: f64 = 0.85;

/// Confidence threshold for low-certainty matches (below this is discarded).
pub const CONFIDENCE_LOW: f64 = 0.70;

/// Confidence for exact-name match.
pub const CONFIDENCE_EXACT_MATCH: f64 = 0.9;

/// Confidence for fuzzy/partial match.
pub const CONFIDENCE_FUZZY_MATCH: f64 = 0.5;

// ---- Extraction ----

/// Number of files to accumulate before flushing to the database.
/// Batch I/O amortizes transaction overhead.
pub const FILE_IO_BATCH_SIZE: usize = 10;

/// Timeout in milliseconds for a single file's tree-sitter parse.
/// Prevents malformed files from blocking the worker pool indefinitely.
pub const PARSE_TIMEOUT_MS: u64 = 10_000;

/// Number of files between worker recycling.
/// Recycles prevent memory accumulation from tree-sitter arena allocators.
pub const WORKER_RECYCLE_INTERVAL: usize = 250;

/// Maximum depth for tree-sitter AST walks. Prevents stack overflow
/// on deeply nested files (>500 depth is pathological even for minified code).
pub const MAX_AST_WALK_DEPTH: usize = 500;

// ---- Resolution ----

/// Default batch size for resolving unresolved references.
pub const RESOLUTION_BATCH_SIZE: usize = 5000;

/// Default maximum file size in bytes for indexing (1 MiB).
/// Larger files are typically generated/minified and not useful for code graph.
pub const MAX_FILE_SIZE: u64 = 1024 * 1024;

/// Maximum length of regex patterns for safety (pathological regex DoS prevention).
pub const MAX_REGEX_PATTERN_LENGTH: usize = 500;
