use super::{
    capture_closure::capture_closure,
    declaration::Declaration, 
    detect_macro_calls::find_macro_calls,
    get_module_declarations::get_module_declarations,
    get_references_from_declaration::get_references_from_declaration,
    load_module_declaration::load_declaration,
};
use crate::{emit_module::emit_module, funee_identifier::FuneeIdentifier, load_module::load_module};
use petgraph::{
    graph::EdgeIndex,
    stable_graph::NodeIndex,
    visit::{Dfs, EdgeRef, VisitMap},
    Graph,
};
use relative_path::RelativePath;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    rc::Rc,
};
use swc_common::{FileLoader, FilePathMapping, Globals, Mark, SourceMap, GLOBALS};
use swc_ecma_ast::{ArrayLit, Callee, Expr, ExprOrSpread, Module, ModuleDecl, ModuleItem};
use swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax, TsSyntax};
use swc_ecma_transforms_base::resolver;
use swc_ecma_visit::VisitMutWith;
use url::Url;

/// JavaScript globals provided by the runtime - skip during bundling
fn is_js_global(name: &str) -> bool {
    matches!(
        name,
        // Core globals
        "globalThis" | "undefined" | "NaN" | "Infinity"
        // Constructors / built-in objects
        | "Object" | "Function" | "Boolean" | "Symbol"
        | "Number" | "BigInt" | "Math" | "Date"
        | "String" | "RegExp"
        | "Array" | "Int8Array" | "Uint8Array" | "Uint8ClampedArray"
        | "Int16Array" | "Uint16Array" | "Int32Array" | "Uint32Array"
        | "Float32Array" | "Float64Array" | "BigInt64Array" | "BigUint64Array"
        | "Map" | "Set" | "WeakMap" | "WeakSet" | "WeakRef" | "FinalizationRegistry"
        | "ArrayBuffer" | "SharedArrayBuffer" | "DataView"
        | "Promise" | "Proxy" | "Reflect"
        | "Error" | "AggregateError" | "EvalError" | "RangeError"
        | "ReferenceError" | "SyntaxError" | "TypeError" | "URIError"
        | "JSON" | "Intl" | "Atomics"
        // Functions
        | "eval" | "isFinite" | "isNaN" | "parseFloat" | "parseInt"
        | "decodeURI" | "decodeURIComponent" | "encodeURI" | "encodeURIComponent"
        // Timer functions
        | "setTimeout" | "setInterval" | "clearTimeout" | "clearInterval"
        | "setImmediate" | "clearImmediate"
        | "queueMicrotask"
        // Console
        | "console"
        // Web APIs commonly available
        | "fetch" | "Request" | "Response" | "Headers" | "URL" | "URLSearchParams"
        | "FormData" | "Blob" | "File" | "FileReader"
        | "TextEncoder" | "TextDecoder"
        | "AbortController" | "AbortSignal"
        | "Event" | "EventTarget" | "CustomEvent"
        | "crypto" | "Crypto" | "CryptoKey" | "SubtleCrypto"
        | "atob" | "btoa"
        | "structuredClone"
    )
}

/// Check if a URI is an HTTP/HTTPS URL
fn is_http_uri(uri: &str) -> bool {
    uri.starts_with("http://") || uri.starts_with("https://")
}

/// Check if a URI is a host:// URL (built-in host modules)
fn is_host_uri(uri: &str) -> bool {
    uri.starts_with("host://")
}

/// Resolve an import URI against the current module's URI
/// 
/// Handles:
/// - "funee" -> funee-lib path
/// - "host://*" -> host module URIs (returned as-is)
/// - HTTP URLs (absolute) -> used as-is
/// - Relative paths from HTTP URLs -> resolved against base URL
/// - Absolute paths (/) from HTTP URLs -> resolved against HTTP server root
/// - Relative paths from file paths -> resolved against base path
/// - Absolute file paths -> used as-is
fn resolve_import_uri(import_uri: &str, base_uri: &str, funee_lib_path: &Option<String>) -> String {
    // Handle bare "funee" specifier
    if import_uri == "funee" {
        return funee_lib_path.clone().unwrap_or_else(|| {
            eprintln!("error: Cannot resolve 'funee' - no funee_lib_path configured");
            std::process::exit(1);
        });
    }

    // Handle host:// URIs - return as-is
    if is_host_uri(import_uri) {
        return import_uri.to_string();
    }

    // If import is already an absolute HTTP URL, use it directly
    if is_http_uri(import_uri) {
        return import_uri.to_string();
    }

    // If import starts with '/', behavior depends on base URI type
    if import_uri.starts_with('/') {
        if is_http_uri(base_uri) {
            // Base is HTTP URL - resolve absolute path against server root
            // e.g., "/lodash-es@4.17.21/add.mjs" from "https://esm.sh/lodash-es"
            //       -> "https://esm.sh/lodash-es@4.17.21/add.mjs"
            match Url::parse(base_uri) {
                Ok(base_url) => {
                    match base_url.join(import_uri) {
                        Ok(resolved) => return resolved.to_string(),
                        Err(e) => {
                            eprintln!("error: Failed to resolve '{}' from '{}': {}", import_uri, base_uri, e);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("error: Invalid base URL '{}': {}", base_uri, e);
                    std::process::exit(1);
                }
            }
        } else {
            // Base is file path - treat as filesystem absolute path
            return import_uri.to_string();
        }
    }

    // Relative import - resolve against base
    if is_http_uri(base_uri) {
        // Base is HTTP URL - resolve relative URL
        match Url::parse(base_uri) {
            Ok(base_url) => {
                match base_url.join(import_uri) {
                    Ok(resolved) => resolved.to_string(),
                    Err(e) => {
                        eprintln!("error: Failed to resolve '{}' from '{}': {}", import_uri, base_uri, e);
                        std::process::exit(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("error: Invalid base URL '{}': {}", base_uri, e);
                std::process::exit(1);
            }
        }
    } else {
        // Base is file path - resolve relative path
        let relative_path = RelativePath::new(import_uri);
        let current_dir = Path::new(base_uri)
            .parent()
            .unwrap_or(Path::new(""))
            .to_str()
            .unwrap_or("");
        relative_path
            .to_logical_path(current_dir)
            .to_str()
            .unwrap_or(import_uri)
            .to_string()
    }
}

pub struct ReferencesMark {
    pub mark: Mark,
    pub globals: Globals,
}

pub struct SourceGraph {
    pub graph: Graph<(String, Declaration), String>,
    pub root: NodeIndex,
    pub source_map: Rc<SourceMap>,
    pub references_mark: ReferencesMark,
    pub funee_lib_path: Option<String>,
    /// Set of FuneeIdentifiers that are macro functions (created via createMacro)
    pub macro_functions: HashSet<FuneeIdentifier>,
}

pub struct LoadParams {
    pub scope: String,
    pub expression: Expr,
    pub host_functions: HashSet<FuneeIdentifier>,
    pub file_loader: Box<dyn FileLoader + Sync + Send>,
    /// Path to the funee standard library (funee-lib/index.ts)
    pub funee_lib_path: Option<String>,
    pub replacement_paths: Vec<String>,
}

struct GraphReplacement {
    target: FuneeIdentifier,
    implementation: Expr,
    source_uri: String,
    dependencies: HashMap<String, FuneeIdentifier>,
}

impl SourceGraph {
    pub fn load(params: LoadParams) -> Self {
        let globals = Globals::default();
        let cm = Rc::new(SourceMap::with_file_loader(
            params.file_loader,
            FilePathMapping::empty(),
        ));
        let unresolved_mark = GLOBALS.set(&globals, || Mark::new());
        
        // Resolve the root expression so its identifiers get the unresolved_mark
        // This is necessary because the expression comes in unresolved from main.rs
        let mut root_expr = params.expression;
        GLOBALS.set(&globals, || {
            let resolver_pass = &mut resolver(unresolved_mark, Mark::new(), true);
            root_expr.visit_mut_with(resolver_pass);
        });
        
        let mut definitions_index = HashMap::new();
        let mut graph = Graph::new();
        let mut macro_functions: HashSet<FuneeIdentifier> = HashSet::new();
        let root_node = graph.add_node((params.scope, Declaration::Expr(root_expr)));
        let mut dfs = Dfs::new(&graph, root_node);
        while let Some(nx) = dfs.next(&graph) {
            let (t, declaration) = &mut graph[nx];
            let source_uri = t.clone(); // Clone early for error messages
            let references = match declaration {
                Declaration::FuneeIdentifier(identifier) => {
                    HashMap::from([(t.clone(), identifier.clone())])
                }
                _ => get_references_from_declaration(declaration, (&globals, unresolved_mark))
                    .into_iter()
                    .map(|x| {
                        (
                            x.clone(),
                            FuneeIdentifier {
                                name: x.clone(),
                                uri: t.clone(),
                            },
                        )
                    })
                    .collect(),
            };

            for reference in references {
                // Skip JavaScript globals unless the current module explicitly imports/declares
                // that name. Imported host bindings like `host://http` fetch must win over
                // same-named runtime globals so macros can capture their canonical target.
                if is_js_global(&reference.0) && load_declaration(&cm, &reference.1).is_none() {
                    continue;
                }

                // Resolve the reference to a declaration and track the final URI
                // This is important for import chains: entry.ts -> a.ts -> b.ts
                // When we resolve levelOne from entry.ts, we follow the import to a.ts
                // The node should have a.ts as its URI so references within levelOne resolve correctly
                let (declaration, resolved_uri) = if params.host_functions.contains(&reference.1) {
                    (
                        Declaration::HostFn(
                            params
                                .host_functions
                                .get(&reference.1)
                                .unwrap()
                                .name
                                .clone(),
                        ),
                        reference.1.uri.clone(), // Host functions don't need real URI
                    )
                } else {
                    let mut current_identifier = reference.1.clone();
                    loop {
                        // Check for host:// URIs - these are built-in host modules
                        if is_host_uri(&current_identifier.uri) {
                            let namespace = current_identifier.uri
                                .strip_prefix("host://")
                                .unwrap()
                                .to_string();
                            break (
                                Declaration::HostModule(namespace, current_identifier.name.clone()),
                                current_identifier.uri.clone(),
                            );
                        }

                        let err_source = source_uri.clone();
                        let err_name = current_identifier.name.clone();
                        let err_module = current_identifier.uri.clone();
                        let declaration = load_declaration(&cm, &current_identifier)
                            .unwrap_or_else(|| {
                                eprintln!("error: Cannot find '{}' in module '{}'", 
                                    err_name, err_module);
                                eprintln!("  --> Referenced from: {}", err_source);
                                std::process::exit(1);
                            })
                            .declaration;

                        if let Declaration::FuneeIdentifier(i) = declaration {
                            if params.host_functions.contains(&i) {
                                break (
                                    Declaration::HostFn(
                                        params.host_functions.get(&i).unwrap().name.clone(),
                                    ),
                                    current_identifier.uri.clone(),
                                );
                            }
                            // Resolve the import URI
                            let resolved_uri = resolve_import_uri(
                                &i.uri, 
                                &current_identifier.uri,
                                &params.funee_lib_path
                            );
                            current_identifier = FuneeIdentifier {
                                name: i.name,
                                uri: resolved_uri,
                            };
                        } else {
                            break (declaration, current_identifier.uri.clone());
                        }
                    }
                };

                if !definitions_index.contains_key(&reference.1) {
                    // Track macro functions for later macro expansion
                    // Use the resolved_uri, not the original reference URI
                    if matches!(&declaration, Declaration::Macro(_)) {
                        macro_functions.insert(FuneeIdentifier {
                            name: reference.1.name.clone(),
                            uri: resolved_uri.clone(),
                        });
                    }
                    
                    let node_index = graph.add_node((resolved_uri, declaration));
                    graph.add_edge(nx, node_index, reference.0);
                    definitions_index.insert(reference.1, node_index);

                    if !dfs.discovered.is_visited(&node_index) {
                        dfs.discovered.grow(graph.node_count());
                        dfs.stack.push(node_index);
                    }
                } else {
                    let node_index = definitions_index.get(&reference.1).unwrap();
                    graph.add_edge(nx, *node_index, reference.0);
                }
            }
        }

        let mut instance = Self {
            graph,
            source_map: cm,
            references_mark: ReferencesMark {
                mark: unresolved_mark,
                globals,
            },
            funee_lib_path: params.funee_lib_path,
            root: root_node,
            macro_functions,
        };

        // Step 2: Process macro calls now that the graph is fully built
        instance.process_macro_calls(&mut definitions_index, &mut dfs);

        let replacements = params
            .replacement_paths
            .iter()
            .flat_map(|path| instance.load_replacements(path))
            .collect::<Vec<_>>();
        instance.apply_replacements(replacements);

        instance
    }

    fn load_replacements(&self, replacement_path: &str) -> Vec<GraphReplacement> {
        let module = load_module(&self.source_map, replacement_path.into());
        let module_declarations = get_module_declarations(module.clone());
        let imports = module_declarations
            .iter()
            .filter_map(|(local_name, declaration)| match &declaration.declaration {
                Declaration::FuneeIdentifier(identifier) => Some((
                    local_name.clone(),
                    FuneeIdentifier {
                        name: identifier.name.clone(),
                        uri: resolve_import_uri(
                            &identifier.uri,
                            replacement_path,
                            &self.funee_lib_path,
                        ),
                    },
                )),
                _ => None,
            })
            .collect::<HashMap<_, _>>();

        let mut replacements = Vec::new();
        for item in module.body {
            let ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultExpr(default_expr)) = item else {
                continue;
            };

            match *default_expr.expr {
                Expr::Array(ArrayLit { elems, .. }) => {
                    for elem in elems.into_iter().flatten() {
                        if let Some(replacement) = self.replacement_from_expr(
                            replacement_path,
                            &imports,
                            elem,
                        ) {
                            replacements.push(replacement);
                        }
                    }
                }
                Expr::Call(call) => {
                    if let Some(replacement) = self.in_memory_host_replacement_from_call(
                        replacement_path,
                        &imports,
                        call,
                    ) {
                        replacements.push(replacement);
                    }
                }
                _ => {}
            }
        }

        replacements
    }

    fn in_memory_host_replacement_from_call(
        &self,
        replacement_path: &str,
        imports: &HashMap<String, FuneeIdentifier>,
        call: swc_ecma_ast::CallExpr,
    ) -> Option<GraphReplacement> {
        let Callee::Expr(callee) = &call.callee else {
            return None;
        };

        if !matches!(callee.as_ref(), Expr::Ident(ident) if ident.sym.as_ref() == "createInMemoryHost") {
            return None;
        }

        let config_arg = call.args.first()?;
        let config_code = self.replacement_expr_to_code(&config_arg.expr);
        let implementation_code = format!(
            r#"async (input, init) => {{
  const __inMemoryHostConfig = {config_code};
  const __servers = __inMemoryHostConfig.http?.servers ?? [];
  const request = typeof input === "object" && input && "url" in input
    ? input
    : {{ url: String(input), method: init?.method ?? "GET", headers: init?.headers, body: init?.body }};
  const requestOrigin = new URL(request.url).origin;
  const server = __servers.find((server) => server.origin === requestOrigin);

  if (!server) {{
    return new Response(`No in-memory HTTP server for ${{requestOrigin}}`, {{ status: 502 }});
  }}

  return await server.handler(request);
}}"#,
        );
        let implementation = self.parse_replacement_expr(&implementation_code)?;

        let mut implementation_for_refs = implementation.clone();
        GLOBALS.set(&self.references_mark.globals, || {
            let resolver_pass = &mut resolver(self.references_mark.mark, Mark::new(), true);
            implementation_for_refs.visit_mut_with(resolver_pass);
        });

        let mut implementation_declaration = Declaration::VarInit(implementation_for_refs.clone());
        let dependency_names = get_references_from_declaration(
            &mut implementation_declaration,
            (&self.references_mark.globals, self.references_mark.mark),
        );
        let dependencies = dependency_names
            .into_iter()
            .filter(|name| !is_js_global(name))
            .map(|name| {
                let identifier = imports.get(&name).cloned().unwrap_or_else(|| FuneeIdentifier {
                    name: name.clone(),
                    uri: replacement_path.to_string(),
                });
                (name, identifier)
            })
            .collect();

        Some(GraphReplacement {
            target: FuneeIdentifier {
                name: "fetch".to_string(),
                uri: "host://http".to_string(),
            },
            implementation: implementation_for_refs,
            source_uri: replacement_path.to_string(),
            dependencies,
        })
    }

    fn replacement_from_expr(
        &self,
        replacement_path: &str,
        imports: &HashMap<String, FuneeIdentifier>,
        elem: ExprOrSpread,
    ) -> Option<GraphReplacement> {
        let Expr::Call(call) = *elem.expr else {
            return None;
        };

        let Callee::Expr(callee) = &call.callee else {
            return None;
        };

        if !matches!(callee.as_ref(), Expr::Ident(ident) if ident.sym.as_ref() == "replacement") {
            return None;
        }

        let target_arg = call.args.first()?;
        let implementation_arg = call.args.get(1)?;
        let Expr::Ident(target_ident) = target_arg.expr.as_ref() else {
            return None;
        };
        let target = imports.get(target_ident.sym.as_ref())?.clone();

        let mut implementation = (*implementation_arg.expr).clone();
        GLOBALS.set(&self.references_mark.globals, || {
            let resolver_pass = &mut resolver(self.references_mark.mark, Mark::new(), true);
            implementation.visit_mut_with(resolver_pass);
        });

        let mut implementation_declaration = Declaration::VarInit(implementation.clone());
        let dependency_names = get_references_from_declaration(
            &mut implementation_declaration,
            (&self.references_mark.globals, self.references_mark.mark),
        );
        let dependencies = dependency_names
            .into_iter()
            .filter(|name| !is_js_global(name))
            .map(|name| {
                let identifier = imports.get(&name).cloned().unwrap_or_else(|| FuneeIdentifier {
                    name: name.clone(),
                    uri: replacement_path.to_string(),
                });
                (name, identifier)
            })
            .collect();

        Some(GraphReplacement {
            target,
            implementation,
            source_uri: replacement_path.to_string(),
            dependencies,
        })
    }

    fn apply_replacements(&mut self, replacements: Vec<GraphReplacement>) {
        for replacement in replacements {
            let Some(target_node) = self.find_replacement_target(&replacement.target) else {
                continue;
            };
            let replacement_node = self.graph.add_node((
                replacement.source_uri.clone(),
                Declaration::VarInit(replacement.implementation),
            ));

            for (local_name, identifier) in replacement.dependencies {
                let dependency_node = self.add_replacement_dependency(identifier);
                self.graph.add_edge(replacement_node, dependency_node, local_name);
            }

            let edges_to_redirect = self
                .graph
                .edge_references()
                .filter(|edge| edge.target() == target_node)
                .map(|edge| (edge.id(), edge.source(), edge.weight().clone()))
                .collect::<Vec<(EdgeIndex, NodeIndex, String)>>();

            for (edge_id, source, weight) in edges_to_redirect {
                self.graph.remove_edge(edge_id);
                self.graph.add_edge(source, replacement_node, weight);
            }
        }
    }

    fn find_replacement_target(&self, target: &FuneeIdentifier) -> Option<NodeIndex> {
        for node in self.graph.node_indices() {
            let (_, declaration) = &self.graph[node];
            match declaration {
                Declaration::HostModule(namespace, export_name)
                    if target.uri == format!("host://{}", namespace) && target.name == *export_name =>
                {
                    return Some(node);
                }
                _ => {}
            }
        }

        None
    }

    fn replacement_expr_to_code(&self, expr: &Expr) -> String {
        let module = Module {
            body: vec![ModuleItem::Stmt(swc_ecma_ast::Stmt::Expr(
                swc_ecma_ast::ExprStmt {
                    span: Default::default(),
                    expr: Box::new(expr.clone()),
                },
            ))],
            shebang: None,
            span: Default::default(),
        };
        let (_srcmap, buf) = emit_module(self.source_map.clone(), module);
        String::from_utf8(buf)
            .expect("failed to convert expression to utf8")
            .trim()
            .trim_end_matches(';')
            .to_string()
    }

    fn parse_replacement_expr(&self, code: &str) -> Option<Expr> {
        let fm = self.source_map.new_source_file(
            swc_common::FileName::Anon.into(),
            code.to_string(),
        );
        let lexer = Lexer::new(
            Syntax::Typescript(TsSyntax::default()),
            swc_ecma_ast::EsVersion::latest(),
            StringInput::from(&*fm),
            None,
        );
        let mut parser = Parser::new_from(lexer);
        parser.parse_expr().ok().map(|expr| *expr)
    }

    fn add_replacement_dependency(&mut self, identifier: FuneeIdentifier) -> NodeIndex {
        let (declaration, resolved_uri) = self.resolve_replacement_dependency(identifier);
        let dependency_node = self.graph.add_node((resolved_uri.clone(), declaration.clone()));

        let mut dependency_declaration = declaration.clone();
        let dependencies = get_references_from_declaration(
            &mut dependency_declaration,
            (&self.references_mark.globals, self.references_mark.mark),
        );
        for local_name in dependencies {
            if is_js_global(&local_name) {
                continue;
            }

            let child_identifier = FuneeIdentifier {
                name: local_name.clone(),
                uri: resolved_uri.clone(),
            };
            let child_node = self.add_replacement_dependency(child_identifier);
            self.graph.add_edge(dependency_node, child_node, local_name);
        }

        dependency_node
    }

    fn resolve_replacement_dependency(&self, identifier: FuneeIdentifier) -> (Declaration, String) {
        let mut current_identifier = identifier;
        loop {
            if is_host_uri(&current_identifier.uri) {
                let namespace = current_identifier
                    .uri
                    .strip_prefix("host://")
                    .unwrap()
                    .to_string();
                return (
                    Declaration::HostModule(namespace, current_identifier.name.clone()),
                    current_identifier.uri,
                );
            }

            let declaration = load_declaration(&self.source_map, &current_identifier)
                .unwrap_or_else(|| {
                    eprintln!(
                        "error: Cannot find '{}' in module '{}'",
                        current_identifier.name, current_identifier.uri,
                    );
                    std::process::exit(1);
                })
                .declaration;

            if let Declaration::FuneeIdentifier(next_identifier) = declaration {
                let resolved_uri = resolve_import_uri(
                    &next_identifier.uri,
                    &current_identifier.uri,
                    &self.funee_lib_path,
                );
                current_identifier = FuneeIdentifier {
                    name: next_identifier.name,
                    uri: resolved_uri,
                };
                continue;
            }

            return (declaration, current_identifier.uri);
        }
    }


    /// Process macro calls in the graph after it's fully constructed
    /// This needs to be a second pass because macros might be defined later in the module tree
    fn process_macro_calls(
        &mut self,
        definitions_index: &mut HashMap<FuneeIdentifier, NodeIndex>,
        dfs: &mut Dfs<NodeIndex, <petgraph::stable_graph::StableGraph<(String, Declaration), String> as petgraph::visit::Visitable>::Map>,
    ) {
        let globals = &self.references_mark.globals;
        let unresolved_mark = self.references_mark.mark;

        // Collect nodes to process (to avoid borrow issues)
        let nodes_to_process: Vec<_> = self.graph.node_indices().collect();

        for nx in nodes_to_process {
            // Clone the data we need
            let (source_uri, mut declaration_clone) = {
                let (t, declaration) = &self.graph[nx];
                (t.clone(), declaration.clone())
            };

            // Check if this declaration contains macro calls
            let expr_to_check = match &declaration_clone {
                Declaration::Expr(e) => Some(e.clone()),
                Declaration::VarInit(e) => Some(e.clone()),
                _ => None,
            };

            if let Some(expr) = expr_to_check {
                // Build current scope references map from the graph edges
                // Each outgoing edge from this node represents a resolved reference
                // The edge label is the local name, the target node contains the resolved URI
                let mut current_scope_refs: HashMap<String, FuneeIdentifier> = HashMap::new();
                for edge in self.graph.edges(nx) {
                    let local_name = edge.weight().clone();
                    let target_node = edge.target();
                    let (target_uri, target_decl) = &self.graph[target_node];
                    
                    // Get the export name - for most declarations it's the same as local name
                    // but we extract it from the target node's declaration
                    let export_name = match target_decl {
                        Declaration::FnDecl(fn_decl) => fn_decl.ident.sym.to_string(),
                        Declaration::HostFn(name) => name.clone(),
                        _ => local_name.clone(),
                    };
                    
                    current_scope_refs.insert(
                        local_name,
                        FuneeIdentifier {
                            name: export_name,
                            uri: target_uri.clone(),
                        },
                    );
                }

                if !is_host_uri(&source_uri) {
                    let module = load_module(&self.source_map, source_uri.clone().into());
                    for (local_name, declaration) in get_module_declarations(module) {
                        let Declaration::FuneeIdentifier(identifier) = declaration.declaration else {
                            continue;
                        };

                        current_scope_refs.entry(local_name).or_insert_with(|| FuneeIdentifier {
                            name: identifier.name,
                            uri: resolve_import_uri(
                                &identifier.uri,
                                &source_uri,
                                &self.funee_lib_path,
                            ),
                        });
                    }
                }

                // Find all macro calls in this expression
                let macro_calls = find_macro_calls(&expr, &self.macro_functions, &current_scope_refs);

                // For each macro call, capture its arguments as Closures
                for macro_call in macro_calls {
                    for (arg_idx, arg_expr) in macro_call.arguments.iter().enumerate() {
                        // Capture this argument as a Closure
                        let closure = capture_closure(
                            arg_expr.clone(),
                            &current_scope_refs,
                        );

                        // Create a unique name for this closure argument
                        let closure_name = format!(
                            "{}_{}_arg{}",
                            macro_call.macro_name,
                            macro_call.call_id,
                            arg_idx,
                        );
                        let closure_identifier = FuneeIdentifier {
                            name: closure_name.clone(),
                            uri: source_uri.clone(),
                        };

                        // Add the Closure as a node in the graph
                        if !definitions_index.contains_key(&closure_identifier) {
                            let closure_node = self.graph.add_node((
                                source_uri.clone(),
                                Declaration::ClosureValue(closure),
                            ));
                            self.graph.add_edge(nx, closure_node, closure_name.clone());
                            definitions_index.insert(closure_identifier, closure_node);

                            // Add to DFS stack to process closure's references
                            if !dfs.discovered.is_visited(&closure_node) {
                                dfs.discovered.grow(self.graph.node_count());
                                dfs.stack.push(closure_node);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_import_uri_absolute_path_from_http() {
        // This is the esm.sh case: importing "/lodash-es@4.17.21/es2022/add.mjs"
        // from "https://esm.sh/lodash-es@4.17.21/add"
        let result = resolve_import_uri(
            "/lodash-es@4.17.21/es2022/add.mjs",
            "https://esm.sh/lodash-es@4.17.21/add",
            &None,
        );
        assert_eq!(result, "https://esm.sh/lodash-es@4.17.21/es2022/add.mjs");
    }

    #[test]
    fn test_resolve_import_uri_absolute_path_from_http_with_subdirectory() {
        // Another case: absolute path from a module in a subdirectory
        let result = resolve_import_uri(
            "/lib/utils.ts",
            "https://example.com/packages/my-lib/index.ts",
            &None,
        );
        assert_eq!(result, "https://example.com/lib/utils.ts");
    }

    #[test]
    fn test_resolve_import_uri_absolute_path_from_file_unchanged() {
        // When base is a file path, absolute paths should remain absolute file paths
        let result = resolve_import_uri(
            "/usr/local/lib/module.ts",
            "/home/user/project/main.ts",
            &None,
        );
        assert_eq!(result, "/usr/local/lib/module.ts");
    }

    #[test]
    fn test_resolve_import_uri_relative_path_from_http() {
        let result = resolve_import_uri(
            "./utils.ts",
            "https://example.com/lib/mod.ts",
            &None,
        );
        assert_eq!(result, "https://example.com/lib/utils.ts");
    }

    #[test]
    fn test_resolve_import_uri_relative_parent_from_http() {
        let result = resolve_import_uri(
            "../other.ts",
            "https://example.com/lib/nested/mod.ts",
            &None,
        );
        assert_eq!(result, "https://example.com/lib/other.ts");
    }

    #[test]
    fn test_resolve_import_uri_absolute_http_url_unchanged() {
        let result = resolve_import_uri(
            "https://cdn.example.com/lodash.js",
            "https://esm.sh/lodash-es",
            &None,
        );
        assert_eq!(result, "https://cdn.example.com/lodash.js");
    }

    #[test]
    fn test_resolve_import_uri_funee_specifier() {
        let result = resolve_import_uri(
            "funee",
            "/some/path/module.ts",
            &Some("/path/to/funee-lib/index.ts".to_string()),
        );
        assert_eq!(result, "/path/to/funee-lib/index.ts");
    }

    #[test]
    fn test_resolve_import_uri_relative_path_from_file() {
        let result = resolve_import_uri(
            "./utils.ts",
            "/home/user/project/src/main.ts",
            &None,
        );
        assert_eq!(result, "/home/user/project/src/utils.ts");
    }

    #[test]
    fn test_resolve_import_uri_host_scheme() {
        // host:// URIs should be returned as-is
        let result = resolve_import_uri(
            "host://fs",
            "/home/user/project/main.ts",
            &None,
        );
        assert_eq!(result, "host://fs");
    }

    #[test]
    fn test_resolve_import_uri_host_scheme_with_path() {
        // host:// URIs with paths should be returned as-is
        let result = resolve_import_uri(
            "host://http/server",
            "/home/user/project/main.ts",
            &None,
        );
        assert_eq!(result, "host://http/server");
    }

    #[test]
    fn test_is_host_uri() {
        assert!(is_host_uri("host://fs"));
        assert!(is_host_uri("host://http/server"));
        assert!(is_host_uri("host://console"));
        assert!(!is_host_uri("http://example.com"));
        assert!(!is_host_uri("https://example.com"));
        assert!(!is_host_uri("/local/path"));
        assert!(!is_host_uri("./relative"));
    }
}
