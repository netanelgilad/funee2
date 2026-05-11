use super::{
    declaration::Declaration,
    get_inline_source_map::get_inline_source_map,
    get_references_from_declaration::rename_references_in_declaration,
    macro_runtime::{MacroClosure, MacroRuntime},
    source_graph::SourceGraph,
};
use crate::emit_module::emit_module;
use petgraph::{
    stable_graph::NodeIndex,
    visit::{DfsPostOrder, EdgeRef},
    Direction::Outgoing,
};
use std::collections::HashMap;
use swc_common::{Mark, GLOBALS};
use swc_ecma_ast::{CallExpr, Callee, EsVersion, Expr, Module, ModuleItem};
use swc_ecma_codegen::{text_writer::JsWriter, Emitter};
use swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax, TsSyntax};
use swc_ecma_transforms_base::resolver;
use swc_ecma_visit::{VisitMut, VisitMutWith};

impl SourceGraph {
    pub fn into_js_execution_code(mut self) -> String {
        // First, expand all macro calls in the graph
        self.expand_macros();
        
        // Collect all host modules used in the graph
        let mut host_namespaces: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (_, declaration) in self.graph.node_weights() {
            if let Declaration::HostModule(namespace, _) = declaration {
                host_namespaces.insert(namespace.clone());
            }
        }
        
        let mut module_items: Vec<ModuleItem> = vec![];
        let mut dfs = DfsPostOrder::new(&self.graph, self.root);
        while let Some(nx) = dfs.next(&self.graph) {
            let declaration = &self.graph[nx].1;
            
            // Skip macro function definitions - they're not needed at runtime
            if matches!(declaration, Declaration::Macro(_)) {
                continue;
            }
            
            // Skip closure values - they're internal representations
            if matches!(declaration, Declaration::ClosureValue(_)) {
                continue;
            }
            
            let edges = self.graph.edges_directed(nx, Outgoing);
            let to_replace: HashMap<String, String> = edges
                .into_iter()
                .map(|e| {
                    (
                        e.weight().into(),
                        format!("declaration_{}", e.target().index()),
                    )
                })
                .collect();
            let mut declaration = self.graph[nx].1.clone();
            rename_references_in_declaration(
                &mut declaration,
                to_replace,
                (&self.references_mark.globals, self.references_mark.mark),
            );
            module_items.push(
                declaration.into_module_item(format!("declaration_{}", nx.index())),
            );
        }
        let module = Module {
            body: module_items,
            shebang: None,
            span: Default::default(),
        };
        let (mut srcmap, buf) = emit_module(self.source_map.clone(), module);
        let code = String::from_utf8(buf).expect("failed to convert to utf8");
        let srcmap_str = get_inline_source_map(&self.source_map, &mut srcmap);
        
        // Generate host module preamble if any host modules are used
        let preamble = generate_host_module_preamble(&host_namespaces);
        
        format!("{}{}{}", preamble, code, srcmap_str)
    }

    /// Expand all macro calls in the graph before emitting
    fn expand_macros(&mut self) {
        // Build a map from edge labels (original identifier names) to their target node indices
        let mut edge_targets: HashMap<(NodeIndex, String), NodeIndex> = HashMap::new();
        
        for edge in self.graph.edge_references() {
            edge_targets.insert((edge.source(), edge.weight().clone()), edge.target());
        }


        // Collect all macro definitions (name -> code) for injection into runtime
        // This allows macros to call other macros
        let mut all_macros: Vec<(String, String)> = Vec::new();
        for edge in self.graph.edge_references() {
            if let Declaration::Macro(expr) = &self.graph[edge.target()].1 {
                let name = edge.weight().clone();
                let code = self.expr_to_code(expr);
                // Avoid duplicates
                if !all_macros.iter().any(|(n, _)| n == &name) {
                    all_macros.push((name, code));
                }
            }
        }

        // Collect nodes to process
        let nodes: Vec<NodeIndex> = self.graph.node_indices().collect();
        
        let mut runtime = MacroRuntime::new();
        
        for nx in nodes {
            let mut declaration = self.graph[nx].1.clone();

            match &mut declaration {
                Declaration::VarInit(expr) | Declaration::Expr(expr) => {
                    self.expand_macro_calls_in_expr(
                        nx,
                        expr,
                        &edge_targets,
                        &all_macros,
                        &mut runtime,
                    );
                    self.graph[nx].1 = declaration;
                }
                _ => {}
            }
        }
    }

    fn expand_macro_calls_in_expr(
        &mut self,
        source_node: NodeIndex,
        expr: &mut Expr,
        edge_targets: &HashMap<(NodeIndex, String), NodeIndex>,
        all_macros: &[(String, String)],
        runtime: &mut MacroRuntime,
    ) {
        struct MacroCallExpander<'a> {
            graph: &'a mut SourceGraph,
            source_node: NodeIndex,
            edge_targets: &'a HashMap<(NodeIndex, String), NodeIndex>,
            all_macros: &'a [(String, String)],
            runtime: &'a mut MacroRuntime,
        }

        impl VisitMut for MacroCallExpander<'_> {
            fn visit_mut_expr(&mut self, expr: &mut Expr) {
                expr.visit_mut_children_with(self);

                let Expr::Call(call_expr) = expr else {
                    return;
                };

                let Callee::Expr(callee_expr) = &call_expr.callee else {
                    return;
                };

                let Expr::Ident(ident) = callee_expr.as_ref() else {
                    return;
                };

                let callee_name = ident.sym.to_string();
                let Some(target_node) = self.edge_targets.get(&(self.source_node, callee_name)) else {
                    return;
                };

                if !matches!(&self.graph.graph[*target_node].1, Declaration::Macro(_)) {
                    return;
                }

                let Some((result_expr, macro_refs)) = self.graph.execute_macro_call(
                    self.source_node,
                    *target_node,
                    call_expr,
                    self.edge_targets,
                    self.all_macros,
                    self.runtime,
                ) else {
                    return;
                };

                for (local_name, (_uri, _export_name)) in macro_refs.iter() {
                    let mut found_target: Option<NodeIndex> = None;
                    for ((_src, _edge_name), tgt) in self.edge_targets.iter() {
                        let (node_uri, _decl) = &self.graph.graph[*tgt];
                        if node_uri.ends_with("funee-lib/index.ts")
                            || node_uri.ends_with("funee-lib/core.ts")
                            || node_uri == "funee"
                        {
                            found_target = Some(*tgt);
                            break;
                        }
                    }

                    if let Some(target_node) = found_target {
                        self.graph.graph.add_edge(self.source_node, target_node, local_name.clone());
                    }
                }

                let result_idents = self.graph.extract_identifiers(&result_expr);
                for ident_name in result_idents {
                    for ((_src, edge_name), tgt) in self.edge_targets.iter() {
                        if edge_name == &ident_name {
                            self.graph.graph.add_edge(self.source_node, *tgt, ident_name.clone());
                            break;
                        }
                    }
                }

                *expr = result_expr;
            }
        }

        expr.visit_mut_with(&mut MacroCallExpander {
            graph: self,
            source_node,
            edge_targets,
            all_macros,
            runtime,
        });
    }

    /// Execute a macro call and return the result expression plus any new references
    /// References are returned as (local_name, (uri, export_name))
    fn execute_macro_call(
        &self,
        source_node: NodeIndex,
        macro_node: NodeIndex,
        call_expr: &CallExpr,
        edge_targets: &HashMap<(NodeIndex, String), NodeIndex>,
        all_macros: &[(String, String)],
        runtime: &mut MacroRuntime,
    ) -> Option<(Expr, HashMap<String, (String, String)>)> {
        // Get the macro function
        let macro_fn = match &self.graph[macro_node].1 {
            Declaration::Macro(expr) => expr,
            _ => return None,
        };

        // Convert macro function to code string
        let macro_fn_code = self.expr_to_code(macro_fn);

        // Get the macro name from the callee for looking up ClosureValue nodes
        let macro_name = match &call_expr.callee {
            Callee::Expr(callee_expr) => {
                if let Expr::Ident(ident) = callee_expr.as_ref() {
                    ident.sym.to_string()
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        };

        // Build arguments as MacroClosures, using ClosureValue nodes for references
        let args: Vec<MacroClosure> = call_expr
            .args
            .iter()
            .enumerate()
            .map(|(arg_idx, arg)| {
                // Look for a ClosureValue node that was created for this argument
                let closure_edge_name = format!("{}_arg{}", macro_name, arg_idx);
                let references = if let Some(closure_node) = edge_targets.get(&(source_node, closure_edge_name)) {
                    // Found the ClosureValue node - get its references
                    if let Declaration::ClosureValue(closure) = &self.graph[*closure_node].1 {
                        closure
                            .references
                            .iter()
                            .map(|(name, id)| (name.clone(), (id.uri.clone(), id.name.clone())))
                            .collect()
                    } else {
                        HashMap::new()
                    }
                } else {
                    HashMap::new()
                };

                // Get the expression code
                let expr_code = if let Expr::Ident(ident) = &*arg.expr {
                    let ident_name = ident.sym.to_string();
                    // Look up what this identifier refers to
                    if let Some(target_node) = edge_targets.get(&(source_node, ident_name)) {
                        // Get the definition's expression
                        match &self.graph[*target_node].1 {
                            Declaration::VarInit(def_expr) => self.expr_to_code(def_expr),
                            Declaration::FnExpr(fn_expr) => self.expr_to_code(&Expr::Fn(fn_expr.clone())),
                            Declaration::Expr(def_expr) => self.expr_to_code(def_expr),
                            _ => self.expr_to_code(&arg.expr),
                        }
                    } else {
                        self.expr_to_code(&arg.expr)
                    }
                } else {
                    self.expr_to_code(&arg.expr)
                };
                MacroClosure {
                    expression: expr_code,
                    references,
                }
            })
            .collect();

        // Execute the macro with other macros available for recursive calls
        // Max iterations prevents infinite macro recursion
        const MAX_ITERATIONS: usize = 100;
        match runtime.execute_macro(&macro_fn_code, args, all_macros, MAX_ITERATIONS) {
            Ok(result) => {
                // Parse the result expression back to AST
                self.parse_expr(&result.expression).map(|expr| (expr, result.references))
            }
            Err(e) => {
                eprintln!("Macro execution failed: {}", e);
                None
            }
        }
    }

    /// Extract identifier names from an expression
    fn extract_identifiers(&self, expr: &Expr) -> Vec<String> {
        use swc_ecma_visit::{Visit, VisitWith};
        
        struct IdentCollector {
            idents: Vec<String>,
        }
        
        impl Visit for IdentCollector {
            fn visit_ident(&mut self, ident: &swc_ecma_ast::Ident) {
                self.idents.push(ident.sym.to_string());
            }
        }
        
        let mut collector = IdentCollector { idents: vec![] };
        expr.visit_with(&mut collector);
        collector.idents
    }

    /// Convert an expression AST to JavaScript code
    fn expr_to_code(&self, expr: &Expr) -> String {
        let mut buf = vec![];
        {
            let wr = JsWriter::new(self.source_map.clone(), "\n", &mut buf, None);
            let mut emitter = Emitter {
                cfg: swc_ecma_codegen::Config::default(),
                cm: self.source_map.clone(),
                comments: None,
                wr: Box::new(wr),
            };
            // Wrap in a module item to emit
            use swc_ecma_ast::{ExprStmt, Stmt};
            let stmt = Stmt::Expr(ExprStmt {
                span: Default::default(),
                expr: Box::new(expr.clone()),
            });
            let module = Module {
                body: vec![ModuleItem::Stmt(stmt)],
                shebang: None,
                span: Default::default(),
            };
            emitter.emit_module(&module).unwrap();
        }
        let code = String::from_utf8(buf).expect("Invalid UTF-8");
        // Remove trailing semicolon and newline
        code.trim().trim_end_matches(';').to_string()
    }

    /// Parse a JavaScript expression string back to AST
    /// Runs the resolver to apply unresolved_mark to identifiers
    fn parse_expr(&self, code: &str) -> Option<Expr> {
        let cm = self.source_map.clone();
        let fm = cm.new_source_file(
            swc_common::FileName::Anon.into(),
            code.to_string(),
        );
        
        let lexer = Lexer::new(
            Syntax::Typescript(TsSyntax::default()),
            EsVersion::latest(),
            StringInput::from(&*fm),
            None,
        );
        
        let mut parser = Parser::new_from(lexer);
        match parser.parse_expr() {
            Ok(mut expr) => {
                // Run the resolver to apply unresolved_mark to identifiers
                // This ensures they'll be renamed correctly during emission
                GLOBALS.set(&self.references_mark.globals, || {
                    let mut res = resolver(self.references_mark.mark, Mark::new(), true);
                    expr.visit_mut_with(&mut res);
                });
                Some(*expr)
            },
            Err(e) => {
                eprintln!("Failed to parse macro result '{}': {:?}", code, e);
                None
            }
        }
    }
}

/// Generate JavaScript code that defines host module objects
/// These are inlined in the bundle preamble for modules like host://fs, host://http, etc.
fn generate_host_module_preamble(namespaces: &std::collections::HashSet<String>) -> String {
    if namespaces.is_empty() {
        return String::new();
    }

    let mut preamble = String::new();

    for namespace in namespaces {
        let var_name = format!("__host_{}", namespace.replace('/', "_"));
        let module_code = get_host_module_code(namespace);
        preamble.push_str(&format!("var {} = {};\n", var_name, module_code));
    }

    preamble
}

/// Get the JavaScript object implementation for a host module namespace
fn get_host_module_code(namespace: &str) -> &'static str {
    match namespace {
        "fs" => r#"({
    readFile: (path) => Deno.core.ops.op_fsReadFile(path),
    readFileBinary: (path) => Deno.core.ops.op_fsReadFileBinary(path),
    writeFile: (path, content) => Deno.core.ops.op_fsWriteFile(path, content),
    writeFileBinary: (path, contentBase64) => Deno.core.ops.op_fsWriteFileBinary(path, contentBase64),
    isFile: (path) => Deno.core.ops.op_fsIsFile(path),
    exists: (path) => Deno.core.ops.op_fsExists(path),
    lstat: (path) => Deno.core.ops.op_fsLstat(path),
    mkdir: (path, recursive) => Deno.core.ops.op_fsMkdir(path, recursive ?? false),
    readdir: (path) => Deno.core.ops.op_fsReaddir(path),
    tmpdir: () => Deno.core.ops.op_tmpdir()
})"#,

        "http" => r#"({
    fetch: globalThis.fetch
})"#,

        "http/server" => r#"({
    serve: globalThis.serve,
    createResponse: (body, init) => new Response(body, init),
    createJsonResponse: (data, init) => Response.json(data, init)
})"#,

        "process" => r#"({
    spawn: globalThis.spawn,
    env: globalThis.env
})"#,

        "time" => r#"({
    setTimeout: globalThis.setTimeout,
    clearTimeout: globalThis.clearTimeout,
    setInterval: globalThis.setInterval,
    clearInterval: globalThis.clearInterval
})"#,

        "watch" => r#"({
    watchStart: (path, recursive) => Deno.core.ops.op_watchStart(path, recursive),
    watchPoll: (watcherId) => Deno.core.ops.op_watchPoll(watcherId),
    watchStop: (watcherId) => Deno.core.ops.op_watchStop(watcherId)
})"#,

        "crypto" => r#"({
    randomBytes: (length) => {
        const hex = Deno.core.ops.op_randomBytes(length);
        const bytes = new Uint8Array(length);
        for (let i = 0; i < length; i++) {
            bytes[i] = parseInt(hex.substr(i * 2, 2), 16);
        }
        return bytes;
    }
})"#,

        "console" => r#"({
    log: (...args) => console.log(...args),
    debug: (...args) => console.debug(...args)
})"#,

        _ => r#"({})"#,
    }
}
