use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use crate::compiler::ast::{
    retyped_global_names, Export, FnSig, ModuleExports, Stmt, StmtKind, Top, Ty,
};
use crate::compiler::{lexer, parser};

pub(crate) struct LoadedModules {
    pub(crate) exports: HashMap<String, ModuleExports>,
    pub(crate) programs: Vec<(String, Vec<Top>)>,
}

struct Loader<'a> {
    base: &'a Path,
    exports: HashMap<String, ModuleExports>,
    programs: Vec<(String, Vec<Top>)>,
    loading: Vec<String>,
}

impl<'a> Loader<'a> {
    fn new(base: &'a Path) -> Self {
        Self {
            base,
            exports: HashMap::new(),
            programs: Vec::new(),
            loading: Vec::new(),
        }
    }

    fn load_requires(&mut self, program: &[Top]) -> Result<(), String> {
        let mut required = Vec::new();
        collect_requires(program, &mut required);
        for id in required {
            self.load_module(&id)?;
        }
        Ok(())
    }

    fn load_module(&mut self, id: &str) -> Result<(), String> {
        if self.exports.contains_key(id) {
            return Ok(());
        }
        if self.loading.iter().any(|loading| loading == id) {
            return Err(format!("cyclic import of module '{id}'"));
        }
        let file = match resolve(self.base, id) {
            Some(ModuleFile::TypeLua(file)) => file,
            Some(ModuleFile::Lua) => {
                self.insert_dynamic(id);
                return Ok(());
            }
            None => {
                if is_explicit_path(id) {
                    return Err(format!(
                        "cannot find module '{id}' relative to {}",
                        self.base.display()
                    ));
                }
                self.insert_dynamic(id);
                return Ok(());
            }
        };
        let source = fs::read_to_string(&file)
            .map_err(|error| format!("cannot read module '{id}' ({}) : {error}", file.display()))?;
        let tokens = lexer::lex(&source).map_err(|error| format!("in module '{id}': {error}"))?;
        let program = parser::parse(tokens, &source).map_err(|errors| {
            errors
                .into_iter()
                .map(|error| format!("in module '{id}': {error}"))
                .collect::<Vec<_>>()
                .join("\n")
        })?;

        self.loading.push(id.to_string());
        let result = self.load_requires(&program);
        self.loading.pop();
        result?;

        let exports =
            build_exports(&program).map_err(|error| format!("in module '{id}': {error}"))?;
        self.exports.insert(id.to_string(), exports);
        self.programs.push((id.to_string(), program));
        Ok(())
    }

    fn insert_dynamic(&mut self, id: &str) {
        self.exports.insert(
            id.to_string(),
            ModuleExports {
                dynamic: true,
                ..ModuleExports::default()
            },
        );
    }
}

enum ModuleFile {
    TypeLua(PathBuf),
    Lua,
}

fn is_explicit_path(id: &str) -> bool {
    id.ends_with(".tlua")
        || id.ends_with(".lua")
        || id.starts_with("./")
        || id.starts_with("../")
        || Path::new(id).is_absolute()
}

fn resolve(dir: &Path, id: &str) -> Option<ModuleFile> {
    if let Some(stem) = id.strip_suffix(".tlua") {
        let file = with_suffix(&dir.join(stem), ".tlua");
        return file.is_file().then_some(ModuleFile::TypeLua(file));
    }
    if let Some(stem) = id.strip_suffix(".lua") {
        let file = with_suffix(&dir.join(stem), ".lua");
        return file.is_file().then_some(ModuleFile::Lua);
    }
    if let Some(file) = resolve_stem(dir, id) {
        return Some(file);
    }
    if !id.contains('/') && id.contains('.') && !id.starts_with('.') {
        return resolve_stem(dir, &id.replace('.', "/"));
    }
    None
}

fn resolve_stem(dir: &Path, id: &str) -> Option<ModuleFile> {
    let base = dir.join(id);
    let tlua = with_suffix(&base, ".tlua");
    if tlua.is_file() {
        return Some(ModuleFile::TypeLua(tlua));
    }
    let lua = with_suffix(&base, ".lua");
    if lua.is_file() {
        return Some(ModuleFile::Lua);
    }
    let tlua_init = base.join("init.tlua");
    if tlua_init.is_file() {
        return Some(ModuleFile::TypeLua(tlua_init));
    }
    let lua_init = base.join("init.lua");
    if lua_init.is_file() {
        return Some(ModuleFile::Lua);
    }
    None
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

fn build_exports(program: &[Top]) -> Result<ModuleExports, String> {
    let mut exports = ModuleExports::default();
    let retyped_globals = retyped_global_names(program);
    fn insert(exports: &mut ModuleExports, name: &str, export: Export) -> Result<(), String> {
        if exports.items.contains_key(name) {
            return Err(format!("duplicate module member '{name}'"));
        }
        exports.items.insert(name.to_string(), export);
        Ok(())
    }
    for top in program {
        match top {
            Top::Fun(function) => {
                let required = function
                    .params
                    .iter()
                    .take_while(|p| p.default.is_none())
                    .count();
                insert(
                    &mut exports,
                    &function.name,
                    Export::Fn(FnSig {
                        args: function.params.iter().map(|p| p.ty.clone()).collect(),
                        required,
                        ret: function.ret.clone(),
                        varargs: false,
                        generic: function.generic.clone(),
                    }),
                )?;
            }
            Top::Stmt(Stmt {
                kind: StmtKind::Decl(decl),
                ..
            }) => {
                if let Some(Export::Var(existing)) = exports.items.get(&decl.name) {
                    let compatible = match (&decl.ty, existing) {
                        (Ty::Dict, Ty::Dict) => true,
                        (
                            Ty::Map(declared_key, declared_value),
                            Ty::Map(existing_key, existing_value),
                        ) => declared_key == existing_key && declared_value == existing_value,
                        (Ty::List(left), Ty::List(right)) => left == right,
                        (Ty::Tuple(_), Ty::Tuple(_)) => true,
                        _ => false,
                    };
                    if !compatible {
                        return Err(format!(
                            "duplicate module member '{}' changes its declared type",
                            decl.name
                        ));
                    }
                } else {
                    insert(
                        &mut exports,
                        &decl.name,
                        Export::Var(if retyped_globals.contains(&decl.name) {
                            Ty::Any
                        } else {
                            decl.ty.clone()
                        }),
                    )?;
                }
            }
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
            Top::Class(class) => {
                for field in &class.fields {
                    if let Some(default) = &field.default {
                        collect_expr_requires(default, out);
                    }
                }
                for callable in class.methods.iter().chain(class.constructor.iter()) {
                    collect_stmt_requires(&callable.body, out);
                }
            }
            Top::Struct(_) => {}
            Top::Interface(_) => {}
            Top::Import { .. } => {}
        }
    }
}

fn collect_stmt_requires(stmts: &[Stmt], out: &mut Vec<String>) {
    for stmt in stmts {
        match &stmt.kind {
            StmtKind::Decl(decl) => collect_expr_requires(&decl.value, out),
            StmtKind::Assign(_, _, value) => collect_expr_requires(value, out),
            StmtKind::AssignMember(base, _, _, value) => {
                collect_expr_requires(base, out);
                collect_expr_requires(value, out);
            }
            StmtKind::AssignIndex(base, index, _, value) => {
                collect_expr_requires(base, out);
                collect_expr_requires(index, out);
                collect_expr_requires(value, out);
            }
            StmtKind::Expr(expr) => collect_expr_requires(expr, out),
            StmtKind::Block(body) => collect_stmt_requires(body, out),
            StmtKind::If(condition, yes, no) => {
                collect_expr_requires(condition, out);
                collect_stmt_requires(yes, out);
                collect_stmt_requires(no, out);
            }
            StmtKind::While(condition, body) => {
                collect_expr_requires(condition, out);
                collect_stmt_requires(body, out);
            }
            StmtKind::For {
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
            StmtKind::Return(value) => {
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
        Expr::New(_, args) | Expr::NewTyped(_, _, args) => {
            for arg in args {
                collect_expr_requires(arg, out);
            }
        }
        Expr::SuperCall(args) | Expr::SuperMethod(_, args) => {
            for arg in args {
                collect_expr_requires(arg, out);
            }
        }
        Expr::Unary(_, inner) => collect_expr_requires(inner, out),
        Expr::Cast(value, _) => collect_expr_requires(value, out),
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
        Expr::Index(base, index) => {
            collect_expr_requires(base, out);
            collect_expr_requires(index, out);
        }
        Expr::Table(fields) => {
            for (_, value) in fields {
                collect_expr_requires(value, out);
            }
        }
        Expr::List(values) => {
            for value in values {
                if let crate::compiler::ast::ListElement::Value(value) = value {
                    collect_expr_requires(value, out);
                }
            }
        }
        Expr::Tuple(values) => {
            for value in values {
                collect_expr_requires(value, out);
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
    loader.load_requires(program)?;
    Ok(LoadedModules {
        exports: loader.exports,
        programs: loader.programs,
    })
}
