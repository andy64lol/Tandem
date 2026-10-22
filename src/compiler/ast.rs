use std::collections::{HashMap, HashSet};

pub(crate) fn line_column(source: &str, position: usize) -> (usize, usize) {
    let position = position.min(source.len());
    let mut line = 1;
    let mut column = 1;
    for (index, character) in source.char_indices() {
        if index >= position {
            break;
        }
        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Ty {
    Int,
    Float,
    Bool,
    Str,
    Void,
    Module(String),
    Table,
    Map(Box<Ty>, Box<Ty>),
    Dict,
    List(Box<Ty>),
    Tuple(Option<Vec<Ty>>),
    Any,
    Auto,
    Struct(String),
    TableShape(Box<TableShape>),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TableShape {
    pub(crate) named: HashMap<String, Ty>,
    pub(crate) positional: Vec<Ty>,
}
impl Ty {
    pub(crate) fn parse(s: &str) -> Option<Self> {
        match s {
            "int" => Some(Self::Int),
            "float" => Some(Self::Float),
            "bool" => Some(Self::Bool),
            "str" => Some(Self::Str),
            "void" => Some(Self::Void),
            "module" => Some(Self::Module(String::new())),
            "table" => Some(Self::Table),
            "dict" => Some(Self::Dict),
            "tuple" => Some(Self::Tuple(None)),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Expr {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Var(String),
    Unary(String, Box<Expr>),
    Bin(Box<Expr>, String, Box<Expr>),
    Call(String, Vec<Expr>),
    Member(Box<Expr>, String),
    MethodCall(Box<Expr>, String, Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Cast(Box<Expr>, Ty),
    Table(Vec<(Option<String>, Expr)>),
    List(Vec<ListElement>),
    Tuple(Vec<Expr>),
}

#[derive(Clone, Debug)]
pub(crate) enum ListElement {
    Value(Expr),
    Previous,
}

#[derive(Clone)]
pub(crate) struct FnSig {
    pub(crate) args: Vec<Ty>,
    pub(crate) required: usize,
    pub(crate) ret: Ty,
    pub(crate) varargs: bool,
}

#[derive(Clone)]
pub(crate) enum Export {
    Fn(FnSig),
    Var(Ty),
}

#[derive(Clone, Default)]
pub(crate) struct ModuleExports {
    pub(crate) items: HashMap<String, Export>,
    pub(crate) dynamic: bool,
    pub(crate) members: Option<HashSet<String>>,
}

pub(crate) fn module_basename(path: &str) -> String {
    let last = path.rsplit(['/', '\\']).next().unwrap_or(path);
    last.strip_suffix(".tlua")
        .or_else(|| last.strip_suffix(".lua"))
        .unwrap_or(last)
        .to_string()
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FormatPart {
    Text(String),
    Variable(String),
}

pub(crate) fn parse_printf_format(format: &str) -> Result<Vec<FormatPart>, String> {
    let chars: Vec<char> = format.chars().collect();
    let mut parts = Vec::new();
    let mut text = String::new();
    let mut index = 0;

    while index < chars.len() {
        match chars[index] {
            '{' if chars.get(index + 1) == Some(&'{') => {
                text.push('{');
                index += 2;
            }
            '{' => {
                let Some(relative_end) = chars[index + 1..].iter().position(|c| *c == '}') else {
                    return Err("unclosed placeholder in printf string".into());
                };
                let end = index + 1 + relative_end;
                let name: String = chars[index + 1..end].iter().collect();
                let mut name_chars = name.chars();
                let valid_start = name_chars
                    .next()
                    .is_some_and(|c| c == '_' || c.is_ascii_alphabetic());
                if !valid_start || !name_chars.all(|c| c == '_' || c.is_ascii_alphanumeric()) {
                    return Err(format!(
                        "printf placeholder '{{{name}}}' must be a variable name"
                    ));
                }
                if !text.is_empty() {
                    parts.push(FormatPart::Text(std::mem::take(&mut text)));
                }
                parts.push(FormatPart::Variable(name));
                index = end + 1;
            }
            '}' if chars.get(index + 1) == Some(&'}') => {
                text.push('}');
                index += 2;
            }
            '}' => return Err("unmatched '}' in printf string".into()),
            c => {
                text.push(c);
                index += 1;
            }
        }
    }

    if !text.is_empty() || parts.is_empty() {
        parts.push(FormatPart::Text(text));
    }
    Ok(parts)
}

#[derive(Clone, Debug)]
pub(crate) struct Stmt {
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) kind: StmtKind,
}

#[derive(Clone, Debug)]
pub(crate) enum StmtKind {
    Decl(VarDecl),
    Assign(String, String, Expr),
    AssignMember(Box<Expr>, String, String, Expr),
    AssignIndex(Box<Expr>, Box<Expr>, String, Expr),
    Expr(Expr),
    Block(Vec<Stmt>),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    For {
        variable: String,
        start: Expr,
        stop: Expr,
        step: Expr,
        body: Vec<Stmt>,
    },
    Return(Option<Expr>),
}
#[derive(Clone, Debug)]
pub(crate) struct VarDecl {
    pub(crate) ty: Ty,
    pub(crate) name: String,
    pub(crate) value: Expr,
    pub(crate) is_const: bool,
    pub(crate) is_local: bool,
}
#[derive(Clone, Debug)]
pub(crate) struct Param {
    pub(crate) ty: Ty,
    pub(crate) name: String,
    pub(crate) default: Option<Expr>,
}
#[derive(Clone, Debug)]
pub(crate) struct Fun {
    pub(crate) name: String,
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) params: Vec<Param>,
    pub(crate) ret: Ty,
    pub(crate) body: Vec<Stmt>,
}
#[derive(Clone, Debug)]
pub(crate) struct StructField {
    pub(crate) ty: Ty,
    pub(crate) name: String,
}

#[derive(Clone, Debug)]
pub(crate) struct StructDef {
    pub(crate) name: String,
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) fields: Vec<StructField>,
}

#[derive(Clone, Debug)]
pub(crate) enum Top {
    Import {
        packages: Vec<String>,
        line: usize,
        column: usize,
    },
    Fun(Fun),
    Stmt(Stmt),
    Struct(StructDef),
}

fn collect_retyped_assignments(
    stmts: &[Stmt],
    globals: &HashSet<String>,
    scopes: &mut Vec<HashSet<String>>,
    names: &mut HashSet<String>,
) {
    for stmt in stmts {
        match &stmt.kind {
            StmtKind::Decl(decl) => {
                if let Some(scope) = scopes.last_mut() {
                    scope.insert(decl.name.clone());
                }
            }
            StmtKind::Assign(name, op, Expr::Cast(_, _)) if op == "=" => {
                if globals.contains(name) && !scopes.iter().rev().any(|scope| scope.contains(name))
                {
                    names.insert(name.clone());
                }
            }
            StmtKind::Block(body) | StmtKind::While(_, body) => {
                scopes.push(HashSet::new());
                collect_retyped_assignments(body, globals, scopes, names);
                scopes.pop();
            }
            StmtKind::If(_, yes, no) => {
                scopes.push(HashSet::new());
                collect_retyped_assignments(yes, globals, scopes, names);
                scopes.pop();
                scopes.push(HashSet::new());
                collect_retyped_assignments(no, globals, scopes, names);
                scopes.pop();
            }
            StmtKind::For { variable, body, .. } => {
                scopes.push(HashSet::from([variable.clone()]));
                collect_retyped_assignments(body, globals, scopes, names);
                scopes.pop();
            }
            _ => {}
        }
    }
}

pub(crate) fn retyped_global_names(program: &[Top]) -> HashSet<String> {
    let globals: HashSet<String> = program
        .iter()
        .filter_map(|top| match top {
            Top::Stmt(Stmt {
                kind: StmtKind::Decl(decl),
                ..
            }) => Some(decl.name.clone()),
            Top::Stmt(Stmt {
                kind: StmtKind::Assign(name, op, Expr::Call(call, args)),
                ..
            }) if op == "=" && call == "require" && matches!(args.as_slice(), [Expr::Str(_)]) => {
                Some(name.clone())
            }
            _ => None,
        })
        .collect();
    let mut names = HashSet::new();
    for top in program {
        match top {
            Top::Fun(function) => {
                let mut scopes = vec![function
                    .params
                    .iter()
                    .map(|param| param.name.clone())
                    .collect()];
                collect_retyped_assignments(&function.body, &globals, &mut scopes, &mut names);
            }
            Top::Stmt(stmt) => collect_retyped_assignments(
                std::slice::from_ref(stmt),
                &globals,
                &mut Vec::new(),
                &mut names,
            ),
            Top::Struct(_) => {}
            Top::Import { .. } => {}
        }
    }
    names
}
