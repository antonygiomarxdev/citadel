use tree_sitter::TreeCursor;

use super::ast_visitor::AstVisitor;
use super::scope_tracker::ScopeTracker;

/// Iterative preorder AST walker.
///
/// Uses `TreeCursor` as a state machine to avoid stack overflow on deeply
/// nested ASTs (e.g. 140KB / 10K-line TypeScript files).
///
/// The TreeWalker handles ONLY traversal and scope exit.
/// Scope ENTRY is handled by the visitor's `on_*` hooks, which can call
/// `scope.enter(id)` when they create a scope-creating node.
pub struct TreeWalker;

impl TreeWalker {
    pub fn walk<V: AstVisitor>(
        cursor: &mut TreeCursor,
        source: &[u8],
        visitor: &mut V,
        scope: &mut ScopeTracker,
    ) {
        loop {
            let node = cursor.node();
            if node.is_named() {
                let kind = node.kind();
                match kind {
                    "function_declaration" | "function_expression" | "arrow_function" => {
                        visitor.on_function(&node, source);
                    }
                    "method_definition" => {
                        visitor.on_method(&node, source);
                    }
                    "class_declaration" => {
                        visitor.on_class(&node, source);
                    }
                    "interface_declaration" => {
                        visitor.on_interface(&node, source);
                    }
                    "type_alias_declaration" => {
                        visitor.on_type_alias(&node, source);
                    }
                    "enum_declaration" => {
                        visitor.on_enum(&node, source);
                    }
                    "variable_declaration" | "lexical_declaration" | "assignment_expression" => {
                        visitor.on_variable(&node, source);
                    }
                    "import_statement" | "import_declaration" => {
                        visitor.on_import(&node, source);
                    }
                    "call_expression" => {
                        visitor.on_call(&node, source);
                    }
                    "new_expression" => {
                        visitor.on_instantiation(&node, source);
                    }
                    "extends_clause" => {
                        visitor.on_extends(&node, source);
                    }
                    "type_annotation" | "type_reference" => {
                        visitor.on_type_ref(&node, source);
                    }
                    _ => {}
                }
            }

            if cursor.goto_first_child() {
                continue;
            }

            'ascend: loop {
                if visitor.is_scope(cursor.node().kind()) {
                    scope.exit();
                }

                if cursor.goto_next_sibling() {
                    break 'ascend;
                }

                if !cursor.goto_parent() {
                    return;
                }
            }
        }
    }
}
