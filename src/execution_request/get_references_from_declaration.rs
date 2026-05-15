use super::declaration::Declaration;
use std::collections::{HashMap, HashSet};
use swc_common::{Globals, Mark, GLOBALS};
use swc_ecma_ast::{ArrowExpr, BlockStmtOrExpr, CatchClause, Function, Ident, Pat, VarDeclarator};
use swc_ecma_transforms_base::resolver;
use swc_ecma_visit::{
    noop_visit_mut_type, noop_visit_type, Visit, VisitMut, VisitMutWith, VisitWith,
};

pub fn get_references_from_declaration(
    decl: &mut Declaration,
    unresolved_mark: (&Globals, Mark),
) -> HashSet<String> {
    match decl {
        Declaration::FnDecl(n) => get_references_from_ast(&mut n.function, unresolved_mark),
        Declaration::FnExpr(n) => get_references_from_ast(n, unresolved_mark),
        Declaration::Expr(n) => get_references_from_ast(n, unresolved_mark),
        Declaration::VarInit(n) => get_references_from_ast(n, unresolved_mark),
        Declaration::Macro(n) => get_references_from_ast(n, unresolved_mark),
        Declaration::ClosureValue(closure) => {
            // Closure already has its references captured
            // Return the reference names from the closure
            closure.references.keys().cloned().collect()
        }
        Declaration::FuneeIdentifier(_) => HashSet::new(),
        Declaration::HostFn(_) => HashSet::new(),
        Declaration::HostModule(_, _) => HashSet::new(),
    }
}

#[derive(Default)]
pub(super) struct ResolveReferences {
    pub unresolved_mark: Mark,
    pub references: HashSet<String>,
    scopes: Vec<HashSet<String>>,
}

impl ResolveReferences {
    fn enter_scope(&mut self) {
        self.scopes.push(HashSet::new());
    }

    fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    fn bind(&mut self, name: &str) {
        if self.scopes.is_empty() {
            self.enter_scope();
        }
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string());
        }
    }

    fn bind_pattern(&mut self, pat: &Pat) {
        match pat {
            Pat::Ident(ident) => self.bind(&ident.id.sym),
            Pat::Array(array) => {
                for elem in array.elems.iter().flatten() {
                    self.bind_pattern(elem);
                }
            }
            Pat::Rest(rest) => self.bind_pattern(&rest.arg),
            Pat::Object(object) => {
                for prop in &object.props {
                    match prop {
                        swc_ecma_ast::ObjectPatProp::KeyValue(kv) => self.bind_pattern(&kv.value),
                        swc_ecma_ast::ObjectPatProp::Assign(assign) => self.bind(&assign.key.sym),
                        swc_ecma_ast::ObjectPatProp::Rest(rest) => self.bind_pattern(&rest.arg),
                    }
                }
            }
            Pat::Assign(assign) => self.bind_pattern(&assign.left),
            Pat::Expr(_) | Pat::Invalid(_) => {}
        }
    }

    fn visit_pattern_defaults(&mut self, pat: &Pat) {
        match pat {
            Pat::Array(array) => {
                for elem in array.elems.iter().flatten() {
                    self.visit_pattern_defaults(elem);
                }
            }
            Pat::Rest(rest) => self.visit_pattern_defaults(&rest.arg),
            Pat::Object(object) => {
                for prop in &object.props {
                    match prop {
                        swc_ecma_ast::ObjectPatProp::KeyValue(kv) => {
                            self.visit_pattern_defaults(&kv.value);
                        }
                        swc_ecma_ast::ObjectPatProp::Assign(assign) => {
                            if let Some(value) = &assign.value {
                                value.visit_with(self);
                            }
                        }
                        swc_ecma_ast::ObjectPatProp::Rest(rest) => {
                            self.visit_pattern_defaults(&rest.arg);
                        }
                    }
                }
            }
            Pat::Assign(assign) => {
                assign.right.visit_with(self);
                self.visit_pattern_defaults(&assign.left);
            }
            Pat::Expr(expr) => expr.visit_with(self),
            Pat::Ident(_) | Pat::Invalid(_) => {}
        }
    }

    fn is_bound(&self, name: &str) -> bool {
        self.scopes.iter().rev().any(|scope| scope.contains(name))
    }
}

impl Visit for ResolveReferences {
    noop_visit_type!();

    fn visit_ident(&mut self, n: &Ident) {
        let name = n.sym.as_str();
        if n.ctxt.has_mark(self.unresolved_mark) && !self.is_bound(name) {
            self.references.insert(n.sym.as_str().to_string());
        }
    }

    fn visit_function(&mut self, n: &Function) {
        self.enter_scope();
        for param in &n.params {
            self.visit_pattern_defaults(&param.pat);
            self.bind_pattern(&param.pat);
        }
        if let Some(body) = &n.body {
            body.visit_with(self);
        }
        self.exit_scope();
    }

    fn visit_arrow_expr(&mut self, n: &ArrowExpr) {
        self.enter_scope();
        for param in &n.params {
            self.visit_pattern_defaults(param);
            self.bind_pattern(param);
        }
        match &*n.body {
            BlockStmtOrExpr::BlockStmt(block) => block.visit_with(self),
            BlockStmtOrExpr::Expr(expr) => expr.visit_with(self),
        }
        self.exit_scope();
    }

    fn visit_var_declarator(&mut self, n: &VarDeclarator) {
        if let Some(init) = &n.init {
            init.visit_with(self);
        }
        self.visit_pattern_defaults(&n.name);
        self.bind_pattern(&n.name);
    }

    fn visit_catch_clause(&mut self, n: &CatchClause) {
        self.enter_scope();
        if let Some(param) = &n.param {
            self.visit_pattern_defaults(param);
            self.bind_pattern(param);
        }
        n.body.visit_with(self);
        self.exit_scope();
    }
}

pub fn get_references_from_ast<T: Clone + VisitMutWith<dyn VisitMut> + VisitWith<ResolveReferences>>(
    ast: &mut T,
    unresolved_mark: (&Globals, Mark),
) -> HashSet<String> {
    GLOBALS.set(unresolved_mark.0, || {
        let resolver = &mut resolver(unresolved_mark.1, Mark::new(), true);
        ast.visit_mut_with(resolver);

        let mut definition_references = ResolveReferences {
            unresolved_mark: unresolved_mark.1,
            scopes: vec![HashSet::new()],
            ..Default::default()
        };
        ast.visit_with(&mut definition_references);

        definition_references.references
    })
}

pub fn rename_references_in_declaration(
    decl: &mut Declaration,
    to_replace: HashMap<String, String>,
    unresolved_mark: (&Globals, Mark),
) {
    match decl {
        Declaration::FnDecl(n) => {
            rename_references_in_ast(&mut n.function, to_replace, unresolved_mark)
        }
        Declaration::FnExpr(n) => rename_references_in_ast(n, to_replace, unresolved_mark),
        Declaration::Expr(n) => rename_references_in_ast(n, to_replace, unresolved_mark),
        Declaration::VarInit(n) => rename_references_in_ast(n, to_replace, unresolved_mark),
        Declaration::Macro(n) => rename_references_in_ast(n, to_replace, unresolved_mark),
        Declaration::ClosureValue(closure) => {
            // Rename references in the closure expression
            rename_references_in_ast(&mut closure.expression, to_replace.clone(), unresolved_mark);
            // The closure's reference map doesn't need updating - it maps local names to canonical identifiers
            // The AST transformation above already handles the renaming in the expression
        }
        Declaration::FuneeIdentifier(_) => {}
        Declaration::HostFn(_) => {}
        Declaration::HostModule(_, _) => {}
    };
}

fn rename_references_in_ast<
    T: Clone + VisitMutWith<dyn VisitMut> + VisitWith<ResolveReferences>,
>(
    ast: &mut T,
    to_replace: HashMap<String, String>,
    unresolved_mark: (&Globals, Mark),
) {
    GLOBALS.set(unresolved_mark.0, || {
        ast.visit_mut_with(&mut RenameReferences {
            unresolved_mark: unresolved_mark.1,
            to_replace,
        });
    });
}

struct RenameReferences {
    pub unresolved_mark: Mark,
    pub to_replace: HashMap<String, String>,
}

impl<'a> VisitMut for RenameReferences {
    noop_visit_mut_type!();

    fn visit_mut_ident(&mut self, n: &mut Ident) {
        if n.ctxt.has_mark(self.unresolved_mark) {
            let name = n.sym.as_str();
            if let Some(to_replace) = self.to_replace.get(name) {
                n.sym = to_replace.clone().into();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution_request::get_module_declarations::get_module_declarations;
    use swc_common::{FileName, SourceMap};
    use swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax, TsSyntax};

    #[test]
    fn function_parameters_and_object_property_keys_are_not_external_references() {
        let globals = Globals::default();
        let unresolved_mark = GLOBALS.set(&globals, || Mark::new());
        let cm = SourceMap::default();
        let fm = cm.new_source_file(
            FileName::Anon.into(),
            r#"
            function json(status: number, body: unknown): Response {
              return createJsonResponse(body, {
                status,
                headers: {
                  "cache-control": "no-store",
                },
              });
            }
            "#
            .to_string(),
        );
        let lexer = Lexer::new(
            Syntax::Typescript(TsSyntax::default()),
            Default::default(),
            StringInput::from(&*fm),
            None,
        );
        let mut parser = Parser::new_from(lexer);
        let module = parser.parse_module().expect("module should parse");
        let mut declarations = get_module_declarations(module);
        let declaration = &mut declarations
            .remove("json")
            .expect("json declaration should exist")
            .declaration;

        let references = get_references_from_declaration(declaration, (&globals, unresolved_mark));

        assert_eq!(
            references,
            HashSet::from(["createJsonResponse".to_string()])
        );
    }

    #[test]
    fn destructuring_default_values_are_external_references() {
        let globals = Globals::default();
        let unresolved_mark = GLOBALS.set(&globals, || Mark::new());
        let cm = SourceMap::default();
        let fm = cm.new_source_file(
            FileName::Anon.into(),
            r#"
            const run = (options = {}) => {
              const { logger = hostLog } = options;
              logger("ok");
            };
            "#
            .to_string(),
        );
        let lexer = Lexer::new(
            Syntax::Typescript(TsSyntax::default()),
            Default::default(),
            StringInput::from(&*fm),
            None,
        );
        let mut parser = Parser::new_from(lexer);
        let module = parser.parse_module().expect("module should parse");
        let mut declarations = get_module_declarations(module);
        let declaration = &mut declarations
            .remove("run")
            .expect("run declaration should exist")
            .declaration;

        let references = get_references_from_declaration(declaration, (&globals, unresolved_mark));

        assert_eq!(references, HashSet::from(["hostLog".to_string()]));
    }
}
