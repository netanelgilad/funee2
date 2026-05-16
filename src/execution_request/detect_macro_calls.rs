use crate::funee_identifier::FuneeIdentifier;
use std::collections::{HashMap, HashSet};
#[cfg_attr(not(test), allow(unused_imports))]
use swc_ecma_ast::{CallExpr, Callee, Expr, ExprOrSpread, Ident};
use swc_ecma_visit::{noop_visit_type, Visit, VisitWith};

/// Information about a macro call found in an expression
#[derive(Debug, Clone)]
pub struct MacroCall {
    /// The name of the macro function being called (local name in current scope)
    pub macro_name: String,
    /// Stable identifier for this call site within its source file.
    pub call_id: String,
    /// The arguments passed to the macro
    pub arguments: Vec<Expr>,
}

/// Visitor that finds all macro calls in an AST
pub struct MacroCallFinder<'a> {
    /// Set of macro function identifiers
    pub macro_functions: &'a HashSet<FuneeIdentifier>,
    /// Map from local names to FuneeIdentifiers in the current scope
    pub scope_references: &'a HashMap<String, FuneeIdentifier>,
    /// Collected macro calls
    pub macro_calls: Vec<MacroCall>,
}

impl<'a> Visit for MacroCallFinder<'a> {
    noop_visit_type!();

    fn visit_call_expr(&mut self, call: &CallExpr) {
        // Check if the callee is an identifier
        if let Callee::Expr(expr) = &call.callee {
            if let Expr::Ident(Ident { sym, .. }) = &**expr {
                let name = sym.as_ref();
                
                // Check if this identifier resolves to a macro function
                if let Some(identifier) = self.scope_references.get(name) {
                    if self.macro_functions.contains(identifier) {
                        // This is a macro call!
                        let arguments: Vec<Expr> = call
                            .args
                            .iter()
                            .map(|arg| (*arg.expr).clone())
                            .collect();
                        
                        self.macro_calls.push(MacroCall {
                            macro_name: name.to_string(),
                            call_id: format!("{}:{}", call.span.lo.0, call.span.hi.0),
                            arguments,
                        });
                    }
                }
            }
        }
        
        // Continue visiting children
        call.visit_children_with(self);
    }
}

/// Find all macro calls in an expression
pub fn find_macro_calls(
    expr: &Expr,
    macro_functions: &HashSet<FuneeIdentifier>,
    scope_references: &HashMap<String, FuneeIdentifier>,
) -> Vec<MacroCall> {
    let mut finder = MacroCallFinder {
        macro_functions,
        scope_references,
        macro_calls: Vec::new(),
    };
    
    expr.visit_with(&mut finder);
    finder.macro_calls
}

#[cfg(test)]
mod tests {
    use super::*;
    use swc_common::SyntaxContext;

    fn ident(name: &str) -> Ident {
        Ident::new(name.into(), Default::default(), SyntaxContext::empty())
    }

    #[test]
    fn test_find_macro_call() {
        // Create expression: closure(add)
        let expr = Expr::Call(CallExpr {
            span: Default::default(),
            ctxt: SyntaxContext::empty(),
            callee: Callee::Expr(Box::new(Expr::Ident(ident("closure")))),
            args: vec![ExprOrSpread {
                spread: None,
                expr: Box::new(Expr::Ident(ident("add"))),
            }],
            type_args: None,
        });

        // Setup scope: closure is a macro
        let mut macro_functions = HashSet::new();
        let closure_id = FuneeIdentifier {
            name: "closure".to_string(),
            uri: "/test/macros.ts".to_string(),
        };
        macro_functions.insert(closure_id.clone());

        let mut scope_references = HashMap::new();
        scope_references.insert("closure".to_string(), closure_id);

        // Find macro calls
        let calls = find_macro_calls(&expr, &macro_functions, &scope_references);

        assert_eq!(calls.len(), 1, "Should find one macro call");
        assert_eq!(calls[0].macro_name, "closure");
        assert_eq!(calls[0].arguments.len(), 1, "Should have one argument");
    }

    #[test]
    fn test_no_macro_calls() {
        // Regular function call: foo(bar)
        let expr = Expr::Call(CallExpr {
            span: Default::default(),
            ctxt: SyntaxContext::empty(),
            callee: Callee::Expr(Box::new(Expr::Ident(ident("foo")))),
            args: vec![ExprOrSpread {
                spread: None,
                expr: Box::new(Expr::Ident(ident("bar"))),
            }],
            type_args: None,
        });

        let macro_functions = HashSet::new();
        let scope_references = HashMap::new();

        let calls = find_macro_calls(&expr, &macro_functions, &scope_references);

        assert_eq!(calls.len(), 0, "Should find no macro calls");
    }
}
