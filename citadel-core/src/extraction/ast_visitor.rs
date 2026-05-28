use tree_sitter::Node;

use super::scope_tracker::ScopeTracker;
use super::ExtractionResult;

/// Interface Segregation: todas las visitas tienen default empty.
/// Cada lenguaje solo implementa los hooks que necesita.
pub trait AstVisitor {
    fn scope_mut(&mut self) -> &mut ScopeTracker;
    fn result_mut(&mut self) -> &mut ExtractionResult;

    fn on_function(&mut self, _node: &Node, _source: &[u8]) {}
    fn on_method(&mut self, _node: &Node, _source: &[u8]) {}
    fn on_class(&mut self, _node: &Node, _source: &[u8]) {}
    fn on_interface(&mut self, _node: &Node, _source: &[u8]) {}
    fn on_type_alias(&mut self, _node: &Node, _source: &[u8]) {}
    fn on_enum(&mut self, _node: &Node, _source: &[u8]) {}
    fn on_variable(&mut self, _node: &Node, _source: &[u8]) {}
    fn on_import(&mut self, _node: &Node, _source: &[u8]) {}
    fn on_call(&mut self, _node: &Node, _source: &[u8]) {}
    fn on_instantiation(&mut self, _node: &Node, _source: &[u8]) {}
    fn on_extends(&mut self, _node: &Node, _source: &[u8]) {}
    fn on_type_ref(&mut self, _node: &Node, _source: &[u8]) {}

    /// Kinds that trigger automatic scope push/pop.
    /// Override for languages with different container node kinds.
    fn scope_kinds(&self) -> &[&str] {
        &["class_declaration", "interface_declaration"]
    }

    fn is_scope(&self, kind: &str) -> bool {
        self.scope_kinds().contains(&kind)
    }
}
