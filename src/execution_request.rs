mod capture_closure;
mod closure;
mod declaration;
mod detect_macro_calls;
mod get_inline_source_map;
mod get_module_declarations;
mod get_references_from_declaration;
mod load_module_declaration;
mod macro_runtime;
mod source_graph;
mod source_graph_to_js_execution_code;

use crate::{funee_identifier::FuneeIdentifier, http_loader::HttpFileLoader, run_js::run_js};
use ast::Expr;
use deno_core::{error::AnyError, OpDecl};
use std::collections::HashMap;
use swc_common::FileLoader;
use swc_ecma_ast as ast;

use self::source_graph::{LoadParams, SourceGraph};

pub struct ExecutionRequest {
    pub expression: Expr,
    pub scope: String,
    pub host_functions: HashMap<FuneeIdentifier, OpDecl>,
    pub file_loader: Box<dyn FileLoader + Sync + Send>,
    /// Path to the funee standard library (funee-lib/index.ts)
    pub funee_lib_path: Option<String>,
    pub replacement_paths: Vec<String>,
}

impl Default for ExecutionRequest {
    fn default() -> Self {
        Self {
            expression: ast::Expr::Lit(ast::Lit::Null(ast::Null {
                span: Default::default(),
            })),
            scope: "".to_string(),
            host_functions: HashMap::new(),
            file_loader: Box::new(HttpFileLoader::default()),
            funee_lib_path: None,
            replacement_paths: Vec::new(),
        }
    }
}

impl ExecutionRequest {
    /// Build the source graph and emit bundled JavaScript code
    pub fn emit(self) -> String {
        let source_graph = SourceGraph::load(LoadParams {
            scope: self.scope,
            expression: self.expression,
            host_functions: self.host_functions.keys().cloned().collect(),
            file_loader: self.file_loader,
            funee_lib_path: self.funee_lib_path,
            replacement_paths: self.replacement_paths,
        });

        source_graph.into_js_execution_code()
    }

    /// Build and execute the bundled code
    pub fn execute(self) -> Result<(), AnyError> {
        let source_graph = SourceGraph::load(LoadParams {
            scope: self.scope,
            expression: self.expression,
            host_functions: self.host_functions.keys().cloned().collect(),
            file_loader: self.file_loader,
            funee_lib_path: self.funee_lib_path,
            replacement_paths: self.replacement_paths,
        });

        let execution_code = source_graph.into_js_execution_code();

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        if let Err(error) = runtime.block_on(run_js(
            &execution_code,
            self.host_functions.into_values().collect(),
        )) {
            eprintln!("error: {}", error);
            return Err(error);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests;
