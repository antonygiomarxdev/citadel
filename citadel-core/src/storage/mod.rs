pub mod error;
pub mod ffi;
pub mod sqlite;
#[cfg(test)]
pub mod test_utils;
pub mod traits;

#[cfg(test)]
mod contract_tests;

use crate::types::*;
use serde::{Deserialize, Serialize};

// ahash is used for HashSet/HashMap in BFS/DFS traversal in graph/mod.rs.
// It is 2-3x faster than SipHash (the default std hasher) because
// it is not cryptographically secure — only DoS-resistant against
// HashDoS attacks via its randomized seed. Since our keys (node IDs,
// edge kinds) are not user-controlled input for hashing purposes,
// this is a safe tradeoff.
//
// Note: we cannot use Default::default() because ahash::RandomState
// does not impl Default when default-features are disabled.
// Instead we construct with RandomState::new() which generates
// fresh random keys per instance.
#[allow(dead_code)]
use ahash::RandomState;
#[allow(dead_code)]
type FastSet<K> = std::collections::HashSet<K, RandomState>;
#[allow(dead_code)]
type FastMap<K, V> = std::collections::HashMap<K, V, RandomState>;

#[allow(dead_code)]
fn fast_set<K: std::cmp::Eq + std::hash::Hash>() -> FastSet<K> {
    FastSet::with_hasher(RandomState::new())
}

#[allow(dead_code)]
fn fast_map<K: std::cmp::Eq + std::hash::Hash, V>() -> FastMap<K, V> {
    FastMap::with_hasher(RandomState::new())
}

// Re-export FullStore as Storage for backward compatibility.
// New code should use the individual traits (NodeStore, EdgeStore, etc.)
// or FullStore. This alias will be removed in citadel-core 0.4+.
pub use traits::FullStore as Storage;
pub use traits::{BatchOps, EdgeStore, FileStore, FullStore, Lifecycle, MetadataStore, NodeStore, StatsProvider, UnresolvedRefStore};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetrics {
    pub incoming_edge_count: u32,
    pub outgoing_edge_count: u32,
    pub call_count: u32,
    pub caller_count: u32,
    pub child_count: u32,
    pub depth: u32,
}

#[allow(dead_code)]
fn filter_kinds(kinds: &[EdgeKind]) -> Option<&[EdgeKind]> {
    if kinds.is_empty() { None } else { Some(kinds) }
}

#[allow(dead_code)]
fn picomatch_like(file_path: &str, pattern: &str) -> bool {
    if pattern.contains('*') {
        let re = pattern
            .replace("**", "___DOUBLESTAR___")
            .replace('.', "\\.")
            .replace('*', "[^/]*")
            .replace("___DOUBLESTAR___", ".*");
        if let Ok(regex) = regex_lite::Regex::new(&format!("^{}$", re)) {
            return regex.is_match(file_path);
        }
    }
    file_path.contains(pattern)
}
