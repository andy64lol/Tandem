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
    Class(String),
    Generic(String),
    Applied(String, Vec<Ty>),
    TableShape(Box<TableShape>),
    /// `T?`, a value of type `T` or `null`.
    Optional(Box<Ty>),
    /// The type of the `null` literal.
    Null,
    /// A first-class function type.
    Function(Box<FnType>),
    /// A nominal enum type.
    Enum(String),
    /// A nominal discriminated union type.
    Union(String),
    /// A compile-time integer constant used as a const generic argument.
    Const(i64),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FnType {
    pub(crate) args: Vec<Ty>,
    pub(crate) required: usize,
    pub(crate) ret: Ty,
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
    Null,
    Var(String),
    Unary(String, Box<Expr>),
    Bin(Box<Expr>, String, Box<Expr>),
    Call(String, Vec<Expr>),
    /// A call whose callee is an arbitrary expression, such as a closure value.
    CallValue(Box<Expr>, Vec<Expr>),
    New(String, Vec<Expr>),
    NewTyped(String, Vec<Ty>, Vec<Expr>),
    SuperCall(Vec<Expr>),
    SuperMethod(String, Vec<Expr>),
    /// A read of a static member of the base class, such as `super.counter`.
    SuperMember(String),
    Member(Box<Expr>, String),
    /// A member read that yields `null` instead of failing, such as `player?.name`.
    SafeMember(Box<Expr>, String),
    MethodCall(Box<Expr>, String, Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Cast(Box<Expr>, Ty),
    Table(Vec<(Option<String>, Expr)>),
    List(Vec<ListElement>),
    Tuple(Vec<Expr>),
    /// An anonymous function value.
    Lambda(Box<Fun>),
    /// A reference to a named function, such as `&double`.
    FuncRef(String),
    /// A reference to a method, such as `&player.name`.
    MethodRef(Box<Expr>),
    /// `left ?? right`, the null-coalescing operator.
    Coalesce(Box<Expr>, Box<Expr>),
    /// `value!`, which unwraps an optional value.
    Unwrap(Box<Expr>),
    /// `value instanceof ClassName`.
    InstanceOf(Box<Expr>, String),
    /// A `match` expression.
    Match(Box<MatchExpr>),
    Spread(Box<Expr>),
    /// `await expr`, which runs an async value to completion.
    Await(Box<Expr>),
    /// `comptime(expr)`, evaluated by the compiler.
    Comptime(Box<Expr>),
}

#[derive(Clone, Debug)]
pub(crate) struct MatchExpr {
    pub(crate) subject: Expr,
    pub(crate) arms: Vec<MatchArm>,
}

#[derive(Clone, Debug)]
pub(crate) struct MatchArm {
    pub(crate) pattern: Pattern,
    pub(crate) body: Expr,
}

#[derive(Clone, Debug)]
pub(crate) enum Pattern {
    Wildcard,
    Literal(Expr),
    Binding(String),
    Tuple(Vec<Pattern>),
    Variant(String, String, Option<Box<Pattern>>),
}

#[derive(Clone, Debug)]
pub(crate) enum ListElement {
    Value(Expr),
    Previous,
}

#[derive(Clone, Debug)]
pub(crate) struct FnSig {
    pub(crate) args: Vec<Ty>,
    pub(crate) required: usize,
    pub(crate) ret: Ty,
    pub(crate) varargs: bool,
    pub(crate) generic: Vec<String>,
    pub(crate) is_async: bool,
}

#[derive(Clone)]
pub(crate) enum Export {
    Fn(FnSig),
    Var(Ty),
    Class(ClassExport),
}

#[derive(Clone, Debug, Default)]
pub(crate) struct InterfaceDef {
    pub(crate) name: String,
    pub(crate) parents: Vec<String>,
    pub(crate) methods: Vec<Fun>,
    pub(crate) line: usize,
    pub(crate) column: usize,
}

#[derive(Clone, Default)]
pub(crate) struct ModuleExports {
    pub(crate) items: HashMap<String, Export>,
    pub(crate) dynamic: bool,
    pub(crate) members: Option<HashSet<String>>,
}

#[derive(Clone, Default)]
pub(crate) struct ClassExport {
    pub(crate) name: String,
    pub(crate) fields: HashMap<String, Ty>,
    pub(crate) constructor: Option<FnSig>,
    pub(crate) methods: HashMap<String, FnSig>,
    pub(crate) static_fields: HashMap<String, Ty>,
    pub(crate) static_methods: HashMap<String, FnSig>,
}

pub(crate) fn module_basename(path: &str) -> String {
    let last = path.rsplit(['/', '\\']).next().unwrap_or(path);
    last.strip_suffix(".tdm")
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
    MultiDecl(Vec<VarDecl>),
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
    Defer(Box<Stmt>),
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
    pub(crate) generic: Vec<String>,
    pub(crate) is_static: bool,
    pub(crate) is_private: bool,
    pub(crate) is_protected: bool,
    pub(crate) is_abstract: bool,
    pub(crate) is_final: bool,
    pub(crate) is_getter: bool,
    pub(crate) is_setter: bool,
    pub(crate) is_async: bool,
    /// The trait a copied default method came from, if any.
    pub(crate) trait_default: Option<String>,
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
#[allow(dead_code)]
pub(crate) struct TypeAliasDef {
    pub(crate) name: String,
    pub(crate) ty: Ty,
    pub(crate) line: usize,
    pub(crate) column: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct ClassField {
    pub(crate) ty: Ty,
    pub(crate) name: String,
    pub(crate) default: Option<Expr>,
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) is_static: bool,
    pub(crate) is_private: bool,
    pub(crate) is_protected: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct ClassDef {
    pub(crate) name: String,
    pub(crate) parents: Vec<String>,
    pub(crate) interfaces: Vec<String>,
    pub(crate) mixins: Vec<String>,
    pub(crate) generic: Vec<String>,
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) fields: Vec<ClassField>,
    pub(crate) constructor: Option<Fun>,
    pub(crate) methods: Vec<Fun>,
    /// Modules allowed to access this class's private members.
    pub(crate) friends: Vec<String>,
    pub(crate) is_abstract: bool,
    pub(crate) is_final: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct EnumVariant {
    pub(crate) name: String,
    pub(crate) value: i64,
    pub(crate) line: usize,
    pub(crate) column: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct EnumDef {
    pub(crate) name: String,
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) variants: Vec<EnumVariant>,
}

#[derive(Clone, Debug)]
pub(crate) struct UnionVariant {
    pub(crate) name: String,
    pub(crate) payload: Option<Ty>,
    pub(crate) line: usize,
    pub(crate) column: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct UnionDef {
    pub(crate) name: String,
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) variants: Vec<UnionVariant>,
}

#[derive(Clone, Debug)]
pub(crate) struct MixinDef {
    pub(crate) name: String,
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) fields: Vec<ClassField>,
    pub(crate) methods: Vec<Fun>,
}

/// Runtime information about an enum declaration.
#[derive(Clone, Debug)]
pub(crate) struct EnumInfo {
    /// Variant name to its integer value.
    pub(crate) variants: HashMap<String, i64>,
}

/// Runtime information about a discriminated union declaration.
#[derive(Clone, Debug)]
pub(crate) struct UnionInfo {
    /// Variant name to the type of its payload, if it has one.
    pub(crate) variants: HashMap<String, Option<Ty>>,
}

/// A value known at compile time.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ConstValue {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
}

/// Evaluates a literal-only expression at compile time.
pub(crate) fn eval_const(expr: &Expr) -> Option<ConstValue> {
    match expr {
        Expr::Int(value) => Some(ConstValue::Int(*value)),
        Expr::Float(value) => Some(ConstValue::Float(*value)),
        Expr::Str(value) => Some(ConstValue::Str(value.clone())),
        Expr::Bool(value) => Some(ConstValue::Bool(*value)),
        Expr::Unary(op, value) => match (op.as_str(), eval_const(value)?) {
            ("-", ConstValue::Int(value)) => Some(ConstValue::Int(-value)),
            ("-", ConstValue::Float(value)) => Some(ConstValue::Float(-value)),
            ("!", ConstValue::Bool(value)) => Some(ConstValue::Bool(!value)),
            _ => None,
        },
        Expr::Bin(left, op, right) => {
            let left = eval_const(left)?;
            let right = eval_const(right)?;
            match (left, op.as_str(), right) {
                (ConstValue::Int(a), "+", ConstValue::Int(b)) => Some(ConstValue::Int(a + b)),
                (ConstValue::Int(a), "-", ConstValue::Int(b)) => Some(ConstValue::Int(a - b)),
                (ConstValue::Int(a), "*", ConstValue::Int(b)) => Some(ConstValue::Int(a * b)),
                (ConstValue::Int(a), "/", ConstValue::Int(b)) if b != 0 => Some(ConstValue::Int(a / b)),
                (ConstValue::Int(a), "%", ConstValue::Int(b)) if b != 0 => Some(ConstValue::Int(a % b)),
                (ConstValue::Float(a), "+", ConstValue::Float(b)) => Some(ConstValue::Float(a + b)),
                (ConstValue::Float(a), "-", ConstValue::Float(b)) => Some(ConstValue::Float(a - b)),
                (ConstValue::Float(a), "*", ConstValue::Float(b)) => Some(ConstValue::Float(a * b)),
                (ConstValue::Float(a), "/", ConstValue::Float(b)) => Some(ConstValue::Float(a / b)),
                (ConstValue::Str(a), "+", ConstValue::Str(b))
                | (ConstValue::Str(a), ",", ConstValue::Str(b)) => {
                    Some(ConstValue::Str(a + &b))
                }
                (ConstValue::Int(a), "==", ConstValue::Int(b)) => Some(ConstValue::Bool(a == b)),
                (ConstValue::Int(a), "!=", ConstValue::Int(b)) => Some(ConstValue::Bool(a != b)),
                (ConstValue::Int(a), "<", ConstValue::Int(b)) => Some(ConstValue::Bool(a < b)),
                (ConstValue::Int(a), ">", ConstValue::Int(b)) => Some(ConstValue::Bool(a > b)),
                (ConstValue::Int(a), "<=", ConstValue::Int(b)) => Some(ConstValue::Bool(a <= b)),
                (ConstValue::Int(a), ">=", ConstValue::Int(b)) => Some(ConstValue::Bool(a >= b)),
                (ConstValue::Bool(a), "&&", ConstValue::Bool(b)) => Some(ConstValue::Bool(a && b)),
                (ConstValue::Bool(a), "||", ConstValue::Bool(b)) => Some(ConstValue::Bool(a || b)),
                _ => None,
            }
        }
        Expr::Comptime(value) => eval_const(value),
        _ => None,
    }
}

pub(crate) fn substitute_type(ty: &Ty, bindings: &HashMap<String, Ty>) -> Ty {
    match ty {
        Ty::Generic(name) => bindings.get(name).cloned().unwrap_or_else(|| ty.clone()),
        Ty::List(element) => Ty::List(Box::new(substitute_type(element, bindings))),
        Ty::Map(key, value) => Ty::Map(
            Box::new(substitute_type(key, bindings)),
            Box::new(substitute_type(value, bindings)),
        ),
        Ty::Tuple(Some(elements)) => Ty::Tuple(Some(
            elements
                .iter()
                .map(|element| substitute_type(element, bindings))
                .collect(),
        )),
        Ty::Applied(name, arguments) => Ty::Applied(
            name.clone(),
            arguments
                .iter()
                .map(|argument| substitute_type(argument, bindings))
                .collect(),
        ),
        Ty::TableShape(shape) => {
            let mut shape = (**shape).clone();
            shape.named = shape
                .named
                .iter()
                .map(|(name, ty)| (name.clone(), substitute_type(ty, bindings)))
                .collect();
            shape.positional = shape
                .positional
                .iter()
                .map(|ty| substitute_type(ty, bindings))
                .collect();
            Ty::TableShape(Box::new(shape))
        }
        Ty::Optional(inner) => Ty::Optional(Box::new(substitute_type(inner, bindings))),
        Ty::Function(signature) => Ty::Function(Box::new(FnType {
            args: signature
                .args
                .iter()
                .map(|ty| substitute_type(ty, bindings))
                .collect(),
            required: signature.required,
            ret: substitute_type(&signature.ret, bindings),
        })),
        _ => ty.clone(),
    }
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
    TypeAlias(TypeAliasDef),
    Interface(InterfaceDef),
    Trait(TraitDef),
    Class(ClassDef),
    Enum(EnumDef),
    Union(UnionDef),
    Mixin(MixinDef),
    Test(TestBlock),
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) struct TestBlock {
    pub(crate) name: String,
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) body: Vec<Stmt>,
}

#[derive(Clone, Debug)]
pub(crate) struct TraitDef {
    pub(crate) name: String,
    pub(crate) parents: Vec<String>,
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) methods: Vec<Fun>,
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
            Top::Class(class) => {
                for method in class.methods.iter().chain(class.constructor.iter()) {
                    let mut scopes = vec![method
                        .params
                        .iter()
                        .map(|param| param.name.clone())
                        .collect()];
                    collect_retyped_assignments(&method.body, &globals, &mut scopes, &mut names);
                }
            }
            Top::Struct(_) => {}
            Top::TypeAlias(_) => {}
            Top::Interface(_) => {}
            Top::Trait(_) => {}
            Top::Enum(_) => {}
            Top::Union(_) => {}
            Top::Mixin(_) => {}
            Top::Test(_) => {}
            Top::Import { .. } => {}
        }
    }
    names
}
