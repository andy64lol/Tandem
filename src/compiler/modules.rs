use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use crate::compiler::ast::{Export, FnSig, ModuleExports, Stmt, Top};
use crate::compiler::{lexer, parser};

pub(crate) struct LoadedModules {
    pub(crate) exports: HashMap<String, ModuleExports>,
    pub(crate) programs: Vec<(String, Vec<Top>)>,
}

struct Loader<'a> {
    _base: &'a Path,
    exports: HashMap<String, ModuleExports>,
    programs: Vec<(String, Vec<Top>)>,
    loading: Vec<String>,
}

impl<'a> Loader<'a> {
    fn new(base: &'a Path) -> Self {
        Self {
            _base: base,
            exports: HashMap::new(),
            programs: Vec::new(),
            loading: Vec::new(),
        }
    }

    fn load_requires(&mut self, program: &[Top], dir: &Path) -> Result<(), String> {
        let mut required = Vec::new();
        collect_requires(program, &mut required);
        for id in required {
            self.load_module(&id, dir)?;
        }
        Ok(())
    }

    fn load_module(&mut self, id: &str, dir: &Path) -> Result<(), String> {
        if self.exports.contains_key(id) {
            return Ok(());
        }
        if self.loading.iter().any(|loading| loading == id) {
            return Err(format!("cyclic import of module '{id}'"));
        }
        let file = resolve(dir, id);
        let source = fs::read_to_string(&file).map_err(|_| {
            format!(
                "cannot find module '{id}' (looked for {})",
                file.display()
            )
        })?;
        let tokens = lexer::lex(&source).map_err(|error| format!("in module '{id}': {error}"))?;
        let program =
            parser::parse(tokens).map_err(|error| format!("in module '{id}': {error}"))?;

        self.loading.push(id.to_string());
        let module_dir = file.parent().unwrap_or(dir).to_path_buf();
        let result = self.load_requires(&program, &module_dir);
        self.loading.pop();
        result?;

        let exports = build_exports(&program).map_err(|error| format!("in module '{id}': {error}"))?;
        self.exports.insert(id.to_string(), exports);
        self.programs.push((id.to_string(), program));
        Ok(())
    }
}

fn resolve(dir: &Path, id: &str) -> PathBuf {
    let mut path = dir.join(id);
    if path.extension().and_then(|extension| extension.to_str()) != Some("tlua") {
        path.set_extension("tlua");
    }
    path
}

fn build_exports(program: &[Top]) -> Result<ModuleExports, String> {
    let mut exports = ModuleExports::default();
    let mut insert = |name: &str, export: Export| -> Result<(), String> {
        if exports.items.contains_key(name) {
            return Err(format!("duplicate module member '{name}'"));
        }
        exports.items.insert(name.to_string(), export);
        Ok(())
    };
    for top in program {
        match top {
            Top::Fun(function) => {
                let required = function.params.iter().take_while(|p| p.default.is_none()).count();
                insert(
                    &function.name,
                    Export::Fn(FnSig {
                        args: function.params.iter().map(|p| p.ty.clone()).collect(),
                        required,
                        ret: function.ret.clone(),
                    }),
                )?;
            }
            Top::Stmt(Stmt::Decl(decl)) => insert(&decl.name, Export::Var(decl.ty.clone()))?,
            _ => {}
        }
    }
    Ok(exports)
}

fn collect_requires(program: &[Top], out: &mut Vec<String>) {
    for top in program {
        match top {
            Top::Fun(function) => collect_stmt_requires(&function.body, out),
            Top::Stmt(stmt) => collect_stmt_requires(std::slice::from_ref(stmt), out),
        }
    }
}

fn collect_stmt_requires(stmts: &[Stmt], out: &mut Vec<String>) {
    for stmt in stmts {
        match stmt {
            Stmt::Decl(decl) => collect_expr_requires(&decl.value, out),
            Stmt::Assign(_, _, value) => collect_expr_requires(value, out),
            Stmt::Require(path) => push_unique(out, path),
            Stmt::Expr(expr) => collect_expr_requires(expr, out),
            Stmt::Block(body) => collect_stmt_requires(body, out),
            Stmt::If(condition, yes, no) => {
                collect_expr_requires(condition, out);
                collect_stmt_requires(yes, out);
                collect_stmt_requires(no, out);
            }
            Stmt::While(condition, body) => {
                collect_expr_requires(condition, out);
                collect_stmt_requires(body, out);
            }
            Stmt::For {
                start,
                stop,
                step,
                body,
                ..
            } => {
                collect_expr_requires(start, out);
                collect_expr_requires(stop, out);
                collect_expr_requires(step, out);
                collect_stmt_requires(body, out);
            }
            Stmt::Return(value) => {
                if let Some(value) = value {
                    collect_expr_requires(value, out);
                }
            }
        }
    }
}

fn collect_expr_requires(expr: &crate::compiler::ast::Expr, out: &mut Vec<String>) {
    use crate::compiler::ast::Expr;
    match expr {
        Expr::Call(name, args) => {
            if name == "require" {
                if let [Expr::Str(path)] = args.as_slice() {
                    push_unique(out, path);
                }
            }
            for arg in args {
                collect_expr_requires(arg, out);
            }
        }
        Expr::Unary(_, inner) => collect_expr_requires(inner, out),
        Expr::Bin(left, _, right) => {
            collect_expr_requires(left, out);
            collect_expr_requires(right, out);
        }
        Expr::Member(base, _) => collect_expr_requires(base, out),
        Expr::MethodCall(base, _, args) => {
            collect_expr_requires(base, out);
            for arg in args {
                collect_expr_requires(arg, out);
            }
        }
        Expr::Int(_) | Expr::Float(_) | Expr::Str(_) | Expr::Bool(_) | Expr::Var(_) => {}
    }
}

fn push_unique(out: &mut Vec<String>, path: &str) {
    if !out.iter().any(|existing| existing == path) {
        out.push(path.to_string());
    }
}

pub(crate) fn load(program: &[Top], base: &Path) -> Result<LoadedModules, String> {
    let mut loader = Loader::new(base);
    loader.load_requires(program, base)?;
    Ok(LoadedModules {
        exports: loader.exports,
        programs: loader.programs,
    })
}
