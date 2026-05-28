/// Tracks the current container scope during AST traversal.
///
/// Container nodes (class, interface, struct) push their scope when entered
/// and pop it when exited. The current scope is used as the parent for
/// `contains` edges of child symbols.
///
/// Thread-local by construction — one per visitor.
pub struct ScopeTracker {
    stack: Vec<String>,
}

impl ScopeTracker {
    pub fn new(root_id: String) -> Self {
        ScopeTracker { stack: vec![root_id] }
    }

    pub fn enter(&mut self, id: String) {
        self.stack.push(id);
    }

    pub fn exit(&mut self) {
        if self.stack.len() > 1 {
            self.stack.pop();
        }
    }

    /// Returns the current (innermost) scope ID, or the root if empty.
    pub fn current(&self) -> &str {
        self.stack.last().map(|s| s.as_str()).unwrap_or("")
    }

    pub fn depth(&self) -> usize {
        self.stack.len()
    }
}
