use std::collections::{HashMap, HashSet};

use crate::compiler::ast::{
    parse_printf_format, retyped_global_names, substitute_type, ClassDef, EnumInfo, Expr, FnSig,
    FormatPart, Fun, ListElement, Stmt, StmtKind, TableShape, Top, Ty, UnionInfo, VarDecl,
};
use crate::compiler::stdlib;

#[derive(Clone, Default)]
struct ClassInfo {
    is_interface: bool,
    parents: Vec<String>,
    generic: Vec<String>,
    fields: HashMap<String, Ty>,
    methods: HashMap<String, FnSig>,
    getters: Vec<String>,
    setters: Vec<String>,
    static_fields: HashMap<String, Ty>,
    static_methods: HashMap<String, FnSig>,
}

type Classes = HashMap<String, ClassInfo>;

struct Ctx<'a> {
    classes: &'a Classes,
    enums: &'a HashMap<String, EnumInfo>,
    unions: &'a HashMap<String, UnionInfo>,
}

fn collect_enums(program: &[Top]) -> HashMap<String, EnumInfo> {
    let mut map = HashMap::new();
    for top in program {
        let Top::Enum(definition) = top else {
            continue;
        };
        let mut variants = HashMap::new();
        for variant in &definition.variants {
            variants.insert(variant.name.clone(), variant.value);
        }
        map.insert(
            definition.name.clone(),
            EnumInfo { variants },
        );
    }
    map
}

fn collect_unions(program: &[Top]) -> HashMap<String, UnionInfo> {
    let mut map = HashMap::new();
    for top in program {
        let Top::Union(definition) = top else {
            continue;
        };
        let mut variants = HashMap::new();
        for variant in &definition.variants {
            variants.insert(variant.name.clone(), variant.payload.clone());
        }
        map.insert(
            definition.name.clone(),
            UnionInfo { variants },
        );
    }
    map
}

fn method_signature(method: &Fun) -> FnSig {
    FnSig {
        args: method.params.iter().map(|param| param.ty.clone()).collect(),
        required: method
            .params
            .iter()
            .take_while(|param| param.default.is_none())
            .count(),
        ret: method.ret.clone(),
        varargs: false,
        generic: method.generic.clone(),
    }
}

fn collect_classes(program: &[Top]) -> Classes {
    let mut map = Classes::new();
    for top in program {
        let Top::Class(definition) = top else {
            continue;
        };
        map.insert(
            definition.name.clone(),
            ClassInfo {
                is_interface: false,
                parents: definition.parents.clone(),
                generic: definition.generic.clone(),
                fields: definition
                    .fields
                    .iter()
                    .filter(|field| !field.is_static)
                    .map(|field| (field.name.clone(), field.ty.clone()))
                    .collect(),
                methods: definition
                    .methods
                    .iter()
                    .filter(|method| !method.is_static && !method.is_getter && !method.is_setter)
                    .map(|method| (method.name.clone(), method_signature(method)))
                    .collect(),
                getters: definition
                    .methods
                    .iter()
                    .filter(|method| method.is_getter)
                    .map(|method| method.name.clone())
                    .collect(),
                setters: definition
                    .methods
                    .iter()
                    .filter(|method| method.is_setter)
                    .map(|method| method.name.clone())
                    .collect(),
                static_fields: definition
                    .fields
                    .iter()
                    .filter(|field| field.is_static)
                    .map(|field| (field.name.clone(), field.ty.clone()))
                    .collect(),
                static_methods: definition
                    .methods
                    .iter()
                    .filter(|method| method.is_static)
                    .map(|method| (method.name.clone(), method_signature(method)))
                    .collect(),
            },
        );
    }
    for top in program {
        let Top::Interface(definition) = top else {
            continue;
        };
        map.insert(
            definition.name.clone(),
            ClassInfo {
                is_interface: true,
                parents: definition.parents.clone(),
                methods: definition
                    .methods
                    .iter()
                    .map(|method| (method.name.clone(), method_signature(method)))
                    .collect(),
                ..ClassInfo::default()
            },
        );
    }
    map
}

fn class_field(ctx: &Ctx, class_name: &str, field: &str) -> Option<Ty> {
    fn find(
        ctx: &Ctx,
        class_name: &str,
        field: &str,
        visited: &mut HashSet<String>,
    ) -> Option<Ty> {
        if !visited.insert(class_name.to_owned()) {
            return None;
        }
        let class = ctx.classes.get(class_name)?;
        class.fields.get(field).cloned().or_else(|| {
            class.parents.iter().find_map(|parent| {
                ctx.classes
                    .contains_key(parent)
                    .then(|| find(ctx, parent, field, visited))
                    .flatten()
            })
        })
    }
    find(ctx, class_name, field, &mut HashSet::new())
}

fn class_static_method(ctx: &Ctx, class_name: &str, method: &str) -> Option<FnSig> {
    fn find(
        ctx: &Ctx,
        class_name: &str,
        method: &str,
        visited: &mut HashSet<String>,
    ) -> Option<FnSig> {
        if !visited.insert(class_name.to_owned()) {
            return None;
        }
        let class = ctx.classes.get(class_name)?;
        class.static_methods.get(method).cloned().or_else(|| {
            class.parents.iter().find_map(|parent| {
                ctx.classes
                    .contains_key(parent)
                    .then(|| find(ctx, parent, method, visited))
                    .flatten()
            })
        })
    }
    find(ctx, class_name, method, &mut HashSet::new())
}

fn class_static_field(ctx: &Ctx, class_name: &str, field: &str) -> Option<Ty> {
    fn find(
        ctx: &Ctx,
        class_name: &str,
        field: &str,
        visited: &mut HashSet<String>,
    ) -> Option<Ty> {
        if !visited.insert(class_name.to_owned()) {
            return None;
        }
        let class = ctx.classes.get(class_name)?;
        class.static_fields.get(field).cloned().or_else(|| {
            class.parents.iter().find_map(|parent| {
                ctx.classes
                    .contains_key(parent)
                    .then(|| find(ctx, parent, field, visited))
                    .flatten()
            })
        })
    }
    find(ctx, class_name, field, &mut HashSet::new())
}
fn class_method(ctx: &Ctx, class_name: &str, method: &str) -> Option<FnSig> {
    fn find(
        ctx: &Ctx,
        class_name: &str,
        method: &str,
        visited: &mut HashSet<String>,
    ) -> Option<FnSig> {
        if !visited.insert(class_name.to_owned()) {
            return None;
        }
        let class = ctx.classes.get(class_name)?;
        class.methods.get(method).cloned().or_else(|| {
            class.parents.iter().find_map(|parent| {
                ctx.classes
                    .contains_key(parent)
                    .then(|| find(ctx, parent, method, visited))
                    .flatten()
            })
        })
    }
    find(ctx, class_name, method, &mut HashSet::new())
}

fn declaration_type(
    decl: &VarDecl,
    scopes: &[HashMap<String, Ty>],
    function_returns: &HashMap<String, Ty>,
    ctx: &Ctx,
) -> Ty {
    match &decl.ty {
        Ty::Auto => {
            inferred_value_type(&decl.value, scopes, function_returns, ctx).unwrap_or(Ty::Any)
        }
        Ty::Table => {
            inferred_value_type(&decl.value, scopes, function_returns, ctx).unwrap_or(Ty::Table)
        }
        Ty::Tuple(None) => inferred_expr_type(&decl.value, scopes, function_returns, ctx)
            .unwrap_or(Ty::Tuple(None)),
        declared => declared.clone(),
    }
}

fn table_shape(
    expr: &Expr,
    scopes: &[HashMap<String, Ty>],
    function_returns: &HashMap<String, Ty>,
    ctx: &Ctx,
) -> Option<TableShape> {
    let Expr::Table(fields) = expr else {
        return None;
    };
    let mut shape = TableShape::default();
    for (name, value) in fields {
        let mut ty =
            inferred_expr_type(value, scopes, function_returns, ctx).unwrap_or(Ty::Any);
        if matches!(ty, Ty::TableShape(_)) {
            ty = Ty::Table;
        }
        match name {
            Some(name) => {
                shape.named.insert(name.clone(), ty);
            }
            None => shape.positional.push(ty),
        }
    }
    Some(shape)
}

fn inferred_value_type(
    expr: &Expr,
    scopes: &[HashMap<String, Ty>],
    function_returns: &HashMap<String, Ty>,
    ctx: &Ctx,
) -> Option<Ty> {
    table_shape(expr, scopes, function_returns, ctx)
        .map(|shape| Ty::TableShape(Box::new(shape)))
        .or_else(|| inferred_expr_type(expr, scopes, function_returns, ctx))
}

fn lua_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '"' => o.push_str("\\\""),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            _ => o.push(c),
        }
    }
    o.push('"');
    o
}

fn lookup_type(name: &str, scopes: &[HashMap<String, Ty>]) -> Option<Ty> {
    scopes
        .iter()
        .rev()
        .find_map(|scope| scope.get(name).cloned())
}

fn update_type(name: &str, ty: Ty, scopes: &mut [HashMap<String, Ty>]) {
    for scope in scopes.iter_mut().rev() {
        if scope.contains_key(name) {
            scope.insert(name.to_owned(), ty);
            return;
        }
    }
}

fn implicit_field_type(
    name: &str,
    scopes: &[HashMap<String, Ty>],
    ctx: &Ctx,
) -> Option<Ty> {
    let self_ty = lookup_type("self", scopes)?;
    let class_name = match &self_ty {
        Ty::Struct(name) | Ty::Applied(name, _) => name,
        _ => return None,
    };
    if scopes.iter().skip(1).any(|scope| scope.contains_key(name)) {
        return None;
    }
    let mut bindings = HashMap::new();
    if let (Some(class), Ty::Applied(_, arguments)) = (ctx.classes.get(class_name), &self_ty) {
        bindings.extend(class.generic.iter().cloned().zip(arguments.iter().cloned()));
    }
    class_field(ctx, class_name, name).map(|ty| substitute_type(&ty, &bindings))
}

fn is_implicit_field(name: &str, scopes: &[HashMap<String, Ty>], ctx: &Ctx) -> bool {
    implicit_field_type(name, scopes, ctx).is_some()
}

fn inferred_expr_type(
    expr: &Expr,
    scopes: &[HashMap<String, Ty>],
    function_returns: &HashMap<String, Ty>,
    ctx: &Ctx,
) -> Option<Ty> {
    match expr {
        Expr::Int(_) => Some(Ty::Int),
        Expr::Float(_) => Some(Ty::Float),
        Expr::Str(_) => Some(Ty::Str),
        Expr::Bool(_) => Some(Ty::Bool),
        Expr::Null => Some(Ty::Null),
        Expr::CallValue(callee, _) => inferred_expr_type(callee, scopes, function_returns, ctx),
        Expr::Var(name) => implicit_field_type(name, scopes, ctx)
            .or_else(|| lookup_type(name, scopes))
            .or_else(|| {
                ctx.classes
                    .get(name)
                    .is_some_and(|class| !class.is_interface)
                    .then(|| Ty::Class(name.clone()))
            })
            .or_else(|| {
                ctx.enums
                    .contains_key(name)
                    .then(|| Ty::Enum(name.clone()))
            })
            .or_else(|| {
                ctx.unions
                    .contains_key(name)
                    .then(|| Ty::Union(name.clone()))
            }),
        Expr::New(name, _) => Some(Ty::Struct(name.clone())),
        Expr::NewTyped(name, arguments, _) => Some(Ty::Applied(name.clone(), arguments.clone())),
        Expr::SuperCall(_) => Some(Ty::Void),
        Expr::SuperMethod(name, _) => {
            let self_ty = lookup_type("self", scopes)?;
            let class_name = match &self_ty {
                Ty::Struct(name) | Ty::Applied(name, _) => name,
                _ => return None,
            };
            let parent = ctx.classes.get(class_name)?.parents.iter().find(|parent| {
                ctx.classes
                    .get(parent.as_str())
                    .is_some_and(|parent| !parent.is_interface)
            })?;
            class_method(ctx, parent, name).map(|sig| sig.ret)
        }
        Expr::Cast(_, ty) => Some(ty.clone()),
        Expr::Unary(_, value) => inferred_expr_type(value, scopes, function_returns, ctx),
        Expr::Bin(left, op, right) => {
            let left_ty = inferred_expr_type(left, scopes, function_returns, ctx)?;
            let right_ty = inferred_expr_type(right, scopes, function_returns, ctx)?;
            if left_ty == right_ty
                && (left_ty == Ty::Str
                    || left_ty == Ty::Int
                    || left_ty == Ty::Float
                    || left_ty == Ty::Bool)
            {
                Some(
                    if matches!(
                        op.as_str(),
                        "==" | "!=" | "<" | ">" | "<=" | ">=" | "&&" | "||"
                    ) {
                        Ty::Bool
                    } else {
                        left_ty
                    },
                )
            } else {
                None
            }
        }
        Expr::Call(name, args) if name == "require" => match args.as_slice() {
            [Expr::Str(path)] => Some(Ty::Module(path.clone())),
            _ => None,
        },
        Expr::Call(name, _) => function_returns
            .get(name)
            .cloned()
            .or_else(|| stdlib_return(name)),
        Expr::Member(base, name) => {
            match inferred_expr_type(base, scopes, function_returns, ctx)? {
                Ty::Map(key_ty, value_ty) if *key_ty == Ty::Str => Some(*value_ty),
                Ty::TableShape(shape) => Some(shape.named.get(name).cloned().unwrap_or(Ty::Any)),
                Ty::Module(_) => package_value(base, name),
                Ty::Struct(class_name) => class_field(ctx, &class_name, name),
                Ty::Class(class_name) => class_static_field(ctx, &class_name, name),
                Ty::Enum(enum_name) => Some(Ty::Enum(enum_name.clone())),
                Ty::Union(union_name) => Some(Ty::Union(union_name.clone())),
                Ty::Applied(class_name, arguments) => {
                    let bindings = ctx.classes
                        .get(&class_name)?
                        .generic
                        .iter()
                        .cloned()
                        .zip(arguments)
                        .collect::<HashMap<_, _>>();
                    class_field(ctx, &class_name, name)
                        .map(|ty| substitute_type(&ty, &bindings))
                }
                _ => None,
            }
        }
        Expr::MethodCall(base, name, _) => {
            match inferred_expr_type(base, scopes, function_returns, ctx)? {
                Ty::Class(class_name) => {
                    if class_static_method(ctx, &class_name, name).is_some() {
                        return Some(Ty::Void);
                    }
                }
                Ty::Struct(class_name) | Ty::Applied(class_name, _) => {
                    if class_method(ctx, &class_name, name).is_some() {
                        return Some(Ty::Void);
                    }
                    if class_static_method(ctx, &class_name, name).is_some() {
                        return Some(Ty::Void);
                    }
                }
                Ty::Union(union_name) => {
                    if ctx
                        .unions
                        .get(&union_name)
                        .is_some_and(|info| info.variants.contains_key(name))
                    {
                        return Some(Ty::Union(union_name));
                    }
                }
                _ => {}
            }
            package_return(base, name)
        }
        Expr::Index(base, index) => {
            match inferred_expr_type(base, scopes, function_returns, ctx)? {
                Ty::List(element_ty) => Some(*element_ty),
                Ty::Map(_, value_ty) => Some(*value_ty),
                Ty::Tuple(Some(elements)) => match index.as_ref() {
                    Expr::Int(index) if *index > 0 => elements.get(*index as usize - 1).cloned(),
                    _ => Some(Ty::Any),
                },
                Ty::TableShape(shape) => Some(match index.as_ref() {
                    Expr::Str(key) => shape.named.get(key).cloned().unwrap_or(Ty::Any),
                    Expr::Int(index) if *index > 0 => shape
                        .positional
                        .get(*index as usize - 1)
                        .cloned()
                        .unwrap_or(Ty::Any),
                    _ => Ty::Any,
                }),
                Ty::Tuple(None) | Ty::Table | Ty::Dict => Some(Ty::Any),
                _ => None,
            }
        }
        Expr::Table(_) => Some(Ty::Table),
        Expr::List(values) => {
            let mut element_ty = None;
            for value in values {
                let ListElement::Value(value) = value else {
                    return None;
                };
                let ty = inferred_expr_type(value, scopes, function_returns, ctx)?;
                if element_ty.as_ref().is_some_and(|element| element != &ty) {
                    return None;
                }
                element_ty = Some(ty);
            }
            Some(Ty::List(Box::new(element_ty.unwrap_or(Ty::Any))))
        }
        Expr::Tuple(values) => values
            .iter()
            .map(|value| inferred_expr_type(value, scopes, function_returns, ctx))
            .collect::<Option<Vec<_>>>()
            .map(|types| Ty::Tuple(Some(types))),
        Expr::Coalesce(left, _) => match inferred_expr_type(left, scopes, function_returns, ctx)? {
            Ty::Optional(inner) => Some(*inner),
            other => Some(other),
        },
        Expr::Unwrap(value) => {
            match inferred_expr_type(value, scopes, function_returns, ctx)? {
                Ty::Optional(inner) => Some(*inner),
                _ => None,
            }
        }
        Expr::SafeMember(base, name) => {
            let base_ty = inferred_expr_type(base, scopes, function_returns, ctx)?;
            let Ty::Optional(inner) = base_ty else {
                return None;
            };
            let member = match *inner {
                Ty::Struct(ref class_name) => class_field(ctx, class_name, name),
                Ty::Applied(ref class_name, ref arguments) => {
                    let bindings = ctx
                        .classes
                        .get(class_name)?
                        .generic
                        .iter()
                        .cloned()
                        .zip(arguments.iter().cloned())
                        .collect::<HashMap<_, _>>();
                    class_field(ctx, class_name, name)
                        .map(|ty| substitute_type(&ty, &bindings))
                }
                Ty::Map(ref key_ty, ref value_ty) if **key_ty == Ty::Str => {
                    Some((**value_ty).clone())
                }
                Ty::TableShape(ref shape) => {
                    Some(shape.named.get(name).cloned().unwrap_or(Ty::Any))
                }
                _ => None,
            }?;
            Some(Ty::Optional(Box::new(member)))
        }
        Expr::InstanceOf(_, _) => Some(Ty::Bool),
        Expr::SuperMember(name) => {
            let self_ty = lookup_type("self", scopes)?;
            let class_name = match &self_ty {
                Ty::Struct(name) | Ty::Applied(name, _) => name.clone(),
                _ => return None,
            };
            let parent = ctx
                .classes
                .get(&class_name)?
                .parents
                .iter()
                .find(|parent| {
                    ctx.classes
                        .get(parent.as_str())
                        .is_some_and(|parent| !parent.is_interface)
                })?;
            class_static_field(ctx, parent, name)
        }
        Expr::Match(expression) => {
            let arm = expression.arms.first()?;
            inferred_expr_type(&arm.body, scopes, function_returns, ctx)
        }
        Expr::Lambda(_) | Expr::FuncRef(_) | Expr::MethodRef(_) => None,
    }
}

fn merge_types(
    scopes: &mut [HashMap<String, Ty>],
    left: &[HashMap<String, Ty>],
    right: &[HashMap<String, Ty>],
) {
    for (index, scope) in scopes.iter_mut().enumerate() {
        for (name, ty) in scope {
            let left_ty = left.get(index).and_then(|types| types.get(name));
            let right_ty = right.get(index).and_then(|types| types.get(name));
            *ty = match (left_ty, right_ty) {
                (Some(left_ty), Some(right_ty)) if left_ty == right_ty => left_ty.clone(),
                (Some(_), Some(_)) => Ty::Any,
                _ => ty.clone(),
            };
        }
    }
}

fn stdlib_return(name: &str) -> Option<Ty> {
    stdlib::base_signature(name).map(|signature| signature.ret.clone())
}

fn package_return(base: &Expr, name: &str) -> Option<Ty> {
    let Expr::Var(package) = base else {
        return None;
    };
    stdlib::package(package)?
        .functions
        .iter()
        .find(|signature| signature.name == name)
        .map(|signature| signature.ret.clone())
}

fn package_value(base: &Expr, name: &str) -> Option<Ty> {
    let Expr::Var(package) = base else {
        return None;
    };
    stdlib::package(package)?
        .values
        .iter()
        .find(|(member, _)| *member == name)
        .map(|(_, ty)| ty.clone())
}

fn is_string_expr(
    expr: &Expr,
    scopes: &[HashMap<String, Ty>],
    function_returns: &HashMap<String, Ty>,
    ctx: &Ctx,
) -> bool {
    match expr {
        Expr::Str(_) => true,
        Expr::Var(_) => {
            matches!(
                inferred_expr_type(expr, scopes, function_returns, ctx),
                Some(Ty::Str)
            )
        }
        Expr::Bin(left, op, right) if op == "+" || op == "," => {
            is_string_expr(left, scopes, function_returns, ctx)
                && is_string_expr(right, scopes, function_returns, ctx)
        }
        Expr::Call(name, _) => {
            function_returns.get(name) == Some(&Ty::Str) || stdlib_return(name) == Some(Ty::Str)
        }
        Expr::Cast(_, Ty::Str) => true,
        Expr::Member(base, name) => {
            inferred_expr_type(expr, scopes, function_returns, ctx) == Some(Ty::Str)
                || package_value(base, name) == Some(Ty::Str)
        }
        Expr::Index(_, _) => {
            inferred_expr_type(expr, scopes, function_returns, ctx) == Some(Ty::Str)
        }
        Expr::MethodCall(base, name, _) => {
            inferred_expr_type(expr, scopes, function_returns, ctx) == Some(Ty::Str)
                || package_return(base, name) == Some(Ty::Str)
        }
        _ => false,
    }
}

fn emit_expr(
    e: &Expr,
    scopes: &[HashMap<String, Ty>],
    function_returns: &HashMap<String, Ty>,
    ctx: &Ctx,
) -> String {
    match e {
        Expr::Int(n) => n.to_string(),
        Expr::Float(n) => n.to_string(),
        Expr::Str(s) => lua_str(s),
        Expr::Bool(b) => b.to_string(),
        Expr::Null => "nil".to_string(),
        Expr::Var(n) => {
            if is_implicit_field(n, scopes, ctx) {
                format!("self.{n}")
            } else {
                n.clone()
            }
        }
        Expr::Unary(op, x) => format!(
            "({}{})",
            if op == "!" { "not " } else { op },
            emit_expr(x, scopes, function_returns, ctx)
        ),
        Expr::Bin(a, op, b) => {
            if operator_class(a, scopes, function_returns, ctx, op).is_some() {
                return format!(
                    "{}:{}({})",
                    emit_expr(a, scopes, function_returns, ctx),
                    lua_method_name(&format!("operator{op}")),
                    emit_expr(b, scopes, function_returns, ctx)
                );
            }
            let lua_op = match op.as_str() {
                "&&" => "and",
                "||" => "or",
                "," => "..",
                "!=" => "~=",
                "+" if is_string_expr(a, scopes, function_returns, ctx)
                    && is_string_expr(b, scopes, function_returns, ctx) =>
                {
                    ".."
                }
                _ => op,
            };
            format!(
                "({} {} {})",
                emit_expr(a, scopes, function_returns, ctx),
                lua_op,
                emit_expr(b, scopes, function_returns, ctx)
            )
        }
        Expr::Call(n, a) if n == "printf" => {
            let [Expr::Str(format)] = a.as_slice() else {
                unreachable!("printf calls are validated by the checker")
            };
            let output = parse_printf_format(format)
                .expect("printf placeholders are validated by the checker")
                .into_iter()
                .map(|part| match part {
                    FormatPart::Text(text) => lua_str(&text),
                    FormatPart::Variable(name) => format!("tostring({name})"),
                })
                .collect::<Vec<_>>()
                .join(" .. ");
            format!("print({output})")
        }
        Expr::Call(n, a) => format!(
            "{}({})",
            if n == "print" { "print" } else { n },
            a.iter()
                .map(|arg| emit_expr(arg, scopes, function_returns, ctx))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::New(name, args) => format!(
            "{}.new({})",
            name,
            args.iter()
                .map(|arg| emit_expr(arg, scopes, function_returns, ctx))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::NewTyped(name, _, args) => format!(
            "{}.new({})",
            name,
            args.iter()
                .map(|arg| emit_expr(arg, scopes, function_returns, ctx))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::SuperCall(args) => {
            let args = args
                .iter()
                .map(|arg| emit_expr(arg, scopes, function_returns, ctx))
                .collect::<Vec<_>>();
            if args.is_empty() {
                "super.__init(self)".to_string()
            } else {
                format!("super.__init(self, {})", args.join(", "))
            }
        }
        Expr::SuperMethod(name, args) => {
            let args = args
                .iter()
                .map(|arg| emit_expr(arg, scopes, function_returns, ctx))
                .collect::<Vec<_>>();
            if args.is_empty() {
                format!("super.{name}(self)")
            } else {
                format!("super.{name}(self, {})", args.join(", "))
            }
        }
        Expr::Member(base, name) => {
            if let Expr::Var(enum_name) = base.as_ref() {
                if let Some(info) = ctx.enums.get(enum_name) {
                    if let Some(value) = info.variants.get(name) {
                        return value.to_string();
                    }
                }
            }
            if getter_class(base, scopes, function_returns, ctx, name).is_some() {
                return format!(
                    "{}.__tandem_getters.{}({})",
                    emit_expr(base, scopes, function_returns, ctx),
                    name,
                    emit_expr(base, scopes, function_returns, ctx)
                );
            }
            format!(
                "{}.{}",
                emit_expr(base, scopes, function_returns, ctx),
                name
            )
        }
        Expr::SafeMember(base, name) => format!(
            "(function(v) if v == nil then return nil end return v.{name} end)({})",
            emit_expr(base, scopes, function_returns, ctx)
        ),
        Expr::MethodCall(base, name, args) => {
            let base_ty = inferred_expr_type(base, scopes, function_returns, ctx);
            if let Some(Ty::Union(union_name)) = &base_ty {
                let payload = ctx
                    .unions
                    .get(union_name)
                    .and_then(|info| info.variants.get(name).cloned());
                let value = match payload {
                    Some(Some(_)) => {
                        let argument = args
                            .iter()
                            .map(|arg| emit_expr(arg, scopes, function_returns, ctx))
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!(", __tdm_value = {argument}")
                    }
                    Some(None) => String::new(),
                    None => String::new(),
                };
                return format!(
                    "{{ __tdm_variant = {}{value} }}",
                    lua_str(name)
                );
            }
            let separator = match base_ty {
                Some(Ty::Class(class_name))
                    if class_static_method(ctx, &class_name, name).is_some() =>
                {
                    "."
                }
                Some(Ty::Struct(class_name) | Ty::Applied(class_name, _))
                    if class_static_method(ctx, &class_name, name).is_some() =>
                {
                    "."
                }
                Some(Ty::Struct(class_name) | Ty::Applied(class_name, _))
                    if class_method(ctx, &class_name, name).is_some() =>
                {
                    ":"
                }
                Some(Ty::Struct(_) | Ty::Applied(_, _)) => ":",
                _ => ".",
            };
            format!(
                "{}{}{}({})",
                emit_expr(base, scopes, function_returns, ctx),
                separator,
                name,
                args.iter()
                    .map(|arg| emit_expr(arg, scopes, function_returns, ctx))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        Expr::SuperMember(name) => format!("super.{name}"),
        Expr::Index(base, index) => format!(
            "{}[{}]",
            emit_expr(base, scopes, function_returns, ctx),
            emit_expr(index, scopes, function_returns, ctx)
        ),
        Expr::Cast(value, _) => emit_expr(value, scopes, function_returns, ctx),
        Expr::Table(fields) => format!(
            "{{ {} }}",
            fields
                .iter()
                .map(|(name, value)| {
                    let value = emit_expr(value, scopes, function_returns, ctx);
                    match name {
                        Some(name) => format!("{name} = {value}"),
                        None => value,
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::List(values) => format!(
            "{{ {} }}",
            values
                .iter()
                .map(|value| match value {
                    ListElement::Value(value) =>
                        emit_expr(value, scopes, function_returns, ctx),
                    ListElement::Previous =>
                        unreachable!("list preservation needs its old binding"),
                })
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::Tuple(values) => format!(
            "{{ {} }}",
            values
                .iter()
                .map(|value| emit_expr(value, scopes, function_returns, ctx))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::Lambda(function) => {
            let params = function
                .params
                .iter()
                .map(|param| param.name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            let mut body = String::new();
            let mut lambda_scopes: Vec<HashMap<String, Ty>> = scopes.to_vec();
            emit_params(
                &mut body,
                &function.params,
                &lambda_scopes,
                function_returns,
                ctx,
            );
            let function_scope = function
                .params
                .iter()
                .map(|param| (param.name.clone(), param.ty.clone()))
                .collect::<HashMap<_, _>>();
            lambda_scopes.push(function_scope);
            emit_stmts(
                &function.body,
                &mut body,
                1,
                &mut lambda_scopes,
                function_returns,
                ctx,
            );
            format!("function({params})\n{body}end")
        }
        Expr::FuncRef(name) => name.clone(),
        Expr::MethodRef(target) => {
            let Expr::Member(base, name) = target.as_ref() else {
                return "nil".to_string();
            };
            let receiver = emit_expr(base, scopes, function_returns, ctx);
            let bound = !matches!(
                inferred_expr_type(base, scopes, function_returns, ctx),
                Some(Ty::Class(_))
            );
            if bound {
                format!("function(...) return {receiver}.{name}({receiver}, ...) end")
            } else {
                format!("function(...) return {receiver}.{name}(...) end")
            }
        }
        Expr::Coalesce(left, right) => format!(
            "(function(v, d) if v == nil then return d end return v end)({}, {})",
            emit_expr(left, scopes, function_returns, ctx),
            emit_expr(right, scopes, function_returns, ctx)
        ),
        Expr::Unwrap(value) => format!(
            "(function(v) if v == nil then error(\"unwrapped a null value\") end return v end)({})",
            emit_expr(value, scopes, function_returns, ctx)
        ),
        Expr::InstanceOf(value, class_name) => format!(
            "__tandem_instanceof({}, {})",
            emit_expr(value, scopes, function_returns, ctx),
            lua_str(class_name)
        ),
        Expr::Match(expression) => emit_match(expression, scopes, function_returns, ctx),
        Expr::CallValue(callee, args) => format!(
            "{}({})",
            emit_expr(callee, scopes, function_returns, ctx),
            args.iter()
                .map(|arg| emit_expr(arg, scopes, function_returns, ctx))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn getter_class(
    base: &Expr,
    scopes: &[HashMap<String, Ty>],
    function_returns: &HashMap<String, Ty>,
    ctx: &Ctx,
    name: &str,
) -> Option<String> {
    let class_name = match inferred_expr_type(base, scopes, function_returns, ctx)? {
        Ty::Struct(class_name) | Ty::Applied(class_name, _) => class_name,
        _ => return None,
    };
    let mut current = class_name.clone();
    let mut visited = HashSet::new();
    loop {
        if !visited.insert(current.clone()) {
            return None;
        }
        let Some(class) = ctx.classes.get(&current) else {
            return None;
        };
        if class.getters.iter().any(|getter| getter == name) {
            return Some(class_name);
        }
        current = class.parents.first()?.clone();
    }
}

fn operator_class(
    base: &Expr,
    scopes: &[HashMap<String, Ty>],
    function_returns: &HashMap<String, Ty>,
    ctx: &Ctx,
    op: &str,
) -> Option<String> {
    let class_name = match inferred_expr_type(base, scopes, function_returns, ctx)? {
        Ty::Struct(class_name) | Ty::Applied(class_name, _) => class_name,
        _ => return None,
    };
    let method = format!("operator{op}");
    let mut current = class_name;
    let mut visited = HashSet::new();
    loop {
        if !visited.insert(current.clone()) {
            return None;
        }
        let Some(class) = ctx.classes.get(&current) else {
            return None;
        };
        if class.methods.contains_key(&method) {
            return Some(current);
        }
        current = class.parents.first()?.clone();
    }
}

fn setter_class(
    base: &Expr,
    scopes: &[HashMap<String, Ty>],
    function_returns: &HashMap<String, Ty>,
    ctx: &Ctx,
    name: &str,
) -> Option<String> {
    let class_name = match inferred_expr_type(base, scopes, function_returns, ctx)? {
        Ty::Struct(class_name) | Ty::Applied(class_name, _) => class_name,
        _ => return None,
    };
    let mut current = class_name.clone();
    let mut visited = HashSet::new();
    loop {
        if !visited.insert(current.clone()) {
            return None;
        }
        let Some(class) = ctx.classes.get(&current) else {
            return None;
        };
        if class.setters.iter().any(|setter| setter == name) {
            return Some(class_name);
        }
        current = class.parents.first()?.clone();
    }
}

fn emit_match(
    expression: &crate::compiler::ast::MatchExpr,
    scopes: &[HashMap<String, Ty>],
    function_returns: &HashMap<String, Ty>,
    ctx: &Ctx,
) -> String {
    use crate::compiler::ast::Pattern;
    let subject = emit_expr(&expression.subject, scopes, function_returns, ctx);
    let mut body = String::new();
    let mut wildcard: Option<&crate::compiler::ast::MatchArm> = None;
    let mut first = true;
    for arm in &expression.arms {
        if matches!(arm.pattern, Pattern::Wildcard) {
            wildcard = Some(arm);
            continue;
        }
        let (condition, bindings) =
            emit_pattern(&arm.pattern, "v", scopes, function_returns, ctx);
        body.push_str(if first { "    if " } else { "    elseif " });
        body.push_str(&condition);
        body.push_str(" then\n");
        for (name, value) in &bindings {
            body.push_str(&format!("        local {name} = {value}\n"));
        }
        let value = emit_expr(&arm.body, scopes, function_returns, ctx);
        body.push_str(&format!("        return {value}\n"));
        first = false;
    }
    match wildcard {
        Some(arm) => {
            body.push_str("    else\n");
            let value = emit_expr(&arm.body, scopes, function_returns, ctx);
            body.push_str(&format!("        return {value}\n    end\n"));
        }
        None => {
            body.push_str("    else\n        error(\"no match arm matched\")\n    end\n");
        }
    }
    format!("(function(v)\n{body}end)({subject})")
}

fn emit_pattern(
    pattern: &crate::compiler::ast::Pattern,
    subject: &str,
    scopes: &[HashMap<String, Ty>],
    function_returns: &HashMap<String, Ty>,
    ctx: &Ctx,
) -> (String, Vec<(String, String)>) {
    use crate::compiler::ast::Pattern;
    match pattern {
        Pattern::Wildcard => ("true".to_string(), Vec::new()),
        Pattern::Literal(expr) => (
            format!("{subject} == {}", emit_expr(expr, scopes, function_returns, ctx)),
            Vec::new(),
        ),
        Pattern::Binding(name) => ("true".to_string(), vec![(name.clone(), subject.to_string())]),
        Pattern::Tuple(patterns) => {
            let mut conditions = Vec::new();
            let mut bindings = Vec::new();
            for (index, pattern) in patterns.iter().enumerate() {
                let element = format!("{subject}[{}]", index + 1);
                let (condition, inner) =
                    emit_pattern(pattern, &element, scopes, function_returns, ctx);
                conditions.push(format!("({condition})"));
                bindings.extend(inner);
            }
            (conditions.join(" and "), bindings)
        }
        Pattern::Variant(name, variant, payload) => {
            if let Some(value) = ctx
                .enums
                .get(name)
                .and_then(|info| info.variants.get(variant).copied())
            {
                return (format!("{subject} == {value}"), Vec::new());
            }
            let mut bindings = Vec::new();
            if let Some(payload) = payload {
                let value = format!("{subject}.__tdm_value");
                let (_, inner) = emit_pattern(payload, &value, scopes, function_returns, ctx);
                bindings.extend(inner);
            }
            (
                format!("{subject}.__tdm_variant == {}", lua_str(variant)),
                bindings,
            )
        }
    }
}

fn emit_list_replacement(
    name: &str,
    expr: &Expr,
    scopes: &[HashMap<String, Ty>],
    function_returns: &HashMap<String, Ty>,
    ctx: &Ctx,
) -> Option<String> {
    let Expr::List(values) = expr else {
        return None;
    };
    if !values
        .iter()
        .any(|value| matches!(value, ListElement::Previous))
    {
        return None;
    }
    let elements = values
        .iter()
        .enumerate()
        .map(|(index, value)| match value {
            ListElement::Value(value) => emit_expr(value, scopes, function_returns, ctx),
            ListElement::Previous => format!(
                "(function(v) if v == nil then error(\"list preservation index {} is out of range\") end return v end)(old[{}])",
                index + 1,
                index + 1
            ),
        })
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!(
        "(function(old) return {{ {elements} }} end)({name})"
    ))
}

fn annotate_line(out: &mut String, start: usize, line: usize, column: usize) {
    if let Some(offset) = out[start..].find('\n') {
        out.insert_str(start + offset, &format!(" -- tdm:{line}:{column}"));
    }
}

fn emit_stmts(
    stmts: &[Stmt],
    out: &mut String,
    indent: usize,
    scopes: &mut Vec<HashMap<String, Ty>>,
    function_returns: &HashMap<String, Ty>,
    ctx: &Ctx,
) {
    let pad = "    ".repeat(indent);
    for s in stmts {
        let start = out.len();
        match &s.kind {
            StmtKind::Decl(decl) => {
                let value = emit_list_replacement(
                    &decl.name,
                    &decl.value,
                    scopes,
                    function_returns,
                    ctx,
                )
                .unwrap_or_else(|| emit_expr(&decl.value, scopes, function_returns, ctx));
                let ty = declaration_type(decl, scopes, function_returns, ctx);
                let current_scope = scopes.last_mut().unwrap();
                if current_scope.contains_key(&decl.name) {
                    out.push_str(&format!("{pad}{} = {value}\n", decl.name));
                } else {
                    out.push_str(&format!("{pad}local {} = {value}\n", decl.name));
                }
                current_scope.insert(decl.name.clone(), ty);
            }
            StmtKind::Assign(n, op, e) => {
                let target = if is_implicit_field(n, scopes, ctx) {
                    format!("self.{n}")
                } else {
                    n.clone()
                };
                let value = emit_list_replacement(&target, e, scopes, function_returns, ctx)
                    .unwrap_or_else(|| emit_expr(e, scopes, function_returns, ctx));
                if op == "=" {
                    out.push_str(&format!("{pad}{target} = {value}\n"));
                    if let Expr::Cast(_, ty) = e {
                        update_type(n, ty.clone(), scopes);
                    }
                } else {
                    let base = op.trim_end_matches('=');
                    out.push_str(&format!("{pad}{target} = {target} {base} {value}\n"));
                }
            }
            StmtKind::AssignMember(base, name, op, e) => {
                if setter_class(base, scopes, function_returns, ctx, name).is_some() {
                    let target = emit_expr(base, scopes, function_returns, ctx);
                    let value = emit_expr(e, scopes, function_returns, ctx);
                    out.push_str(&format!(
                        "{pad}{target}.__tandem_setters.{name}({target}, {value})\n"
                    ));
                    continue;
                }
                let target = format!(
                    "{}.{}",
                    emit_expr(base, scopes, function_returns, ctx),
                    name
                );
                let value = emit_expr(e, scopes, function_returns, ctx);
                if op == "=" {
                    out.push_str(&format!("{pad}{target} = {value}\n"));
                } else {
                    let base_op = op.trim_end_matches('=');
                    out.push_str(&format!("{pad}{target} = {target} {base_op} {value}\n"));
                }
            }
            StmtKind::AssignIndex(base, index, op, e) => {
                let target = format!(
                    "{}[{}]",
                    emit_expr(base, scopes, function_returns, ctx),
                    emit_expr(index, scopes, function_returns, ctx)
                );
                let value = emit_expr(e, scopes, function_returns, ctx);
                if op == "=" {
                    out.push_str(&format!("{pad}{target} = {value}\n"));
                } else {
                    let base_op = op.trim_end_matches('=');
                    out.push_str(&format!("{pad}{target} = {target} {base_op} {value}\n"));
                }
            }
            StmtKind::Expr(e) => out.push_str(&format!(
                "{pad}{}\n",
                emit_expr(e, scopes, function_returns, ctx)
            )),
            StmtKind::Block(body) => {
                out.push_str(&format!("{pad}do\n"));
                scopes.push(HashMap::new());
                emit_stmts(body, out, indent + 1, scopes, function_returns, ctx);
                scopes.pop();
                out.push_str(&format!("{pad}end\n"));
            }
            StmtKind::If(c, a, b) => {
                let before = scopes.clone();
                out.push_str(&format!(
                    "{pad}if {} then\n",
                    emit_expr(c, scopes, function_returns, ctx)
                ));
                scopes.push(HashMap::new());
                emit_stmts(a, out, indent + 1, scopes, function_returns, ctx);
                scopes.pop();
                let yes_types = scopes.clone();
                *scopes = before.clone();
                if !b.is_empty() {
                    out.push_str(&format!("{pad}else\n"));
                    scopes.push(HashMap::new());
                    emit_stmts(b, out, indent + 1, scopes, function_returns, ctx);
                    scopes.pop();
                }
                let no_types = scopes.clone();
                merge_types(scopes, &yes_types, &no_types);
                out.push_str(&format!("{pad}end\n"));
            }
            StmtKind::While(c, b) => {
                let before = scopes.clone();
                out.push_str(&format!(
                    "{pad}while {} do\n",
                    emit_expr(c, scopes, function_returns, ctx)
                ));
                scopes.push(HashMap::new());
                emit_stmts(b, out, indent + 1, scopes, function_returns, ctx);
                scopes.pop();
                let after = scopes.clone();
                merge_types(scopes, &before, &after);
                out.push_str(&format!("{pad}end\n"));
            }
            StmtKind::For {
                variable,
                start,
                stop,
                step,
                body,
            } => {
                let before = scopes.clone();
                out.push_str(&format!(
                    "{pad}for {variable} = {}, {}, {} do\n",
                    emit_expr(start, scopes, function_returns, ctx),
                    emit_expr(stop, scopes, function_returns, ctx),
                    emit_expr(step, scopes, function_returns, ctx)
                ));
                let mut loop_scope = HashMap::new();
                loop_scope.insert(variable.clone(), Ty::Int);
                scopes.push(loop_scope);
                emit_stmts(body, out, indent + 1, scopes, function_returns, ctx);
                scopes.pop();
                let after = scopes.clone();
                merge_types(scopes, &before, &after);
                out.push_str(&format!("{pad}end\n"));
            }
            StmtKind::Return(e) => out.push_str(&format!(
                "{pad}return{}\n",
                e.as_ref()
                    .map(|x| format!(" {}", emit_expr(x, scopes, function_returns, ctx)))
                    .unwrap_or_default()
            )),
        }
        annotate_line(out, start, s.line, s.column);
    }
}

fn emit_params(
    out: &mut String,
    params: &[crate::compiler::ast::Param],
    scopes: &[HashMap<String, Ty>],
    function_returns: &HashMap<String, Ty>,
    ctx: &Ctx,
) {
    for param in params {
        if let Some(default) = &param.default {
            out.push_str(&format!(
                "    if {} == nil then {} = {} end\n",
                param.name,
                param.name,
                emit_expr(default, scopes, function_returns, ctx)
            ));
        }
    }
}

fn find_class<'a>(program: &'a [Top], name: &str) -> Option<&'a ClassDef> {
    program.iter().find_map(|top| match top {
        Top::Class(class) if class.name == name => Some(class),
        _ => None,
    })
}

fn effective_constructor<'a>(program: &'a [Top], class: &'a ClassDef) -> Option<&'a Fun> {
    if let Some(constructor) = &class.constructor {
        return Some(constructor);
    }
    class
        .parents
        .iter()
        .find_map(|parent| find_class(program, parent))
        .and_then(|parent| effective_constructor(program, parent))
}

fn param_list(function: Option<&Fun>) -> Vec<String> {
    function
        .map(|function| {
            function
                .params
                .iter()
                .map(|param| param.name.clone())
                .collect()
        })
        .unwrap_or_default()
}

fn emit_fields(
    class: &ClassDef,
    out: &mut String,
    scopes: &[HashMap<String, Ty>],
    function_returns: &HashMap<String, Ty>,
    ctx: &Ctx,
) {
    for field in &class.fields {
        if field.is_static || field.default.is_none() {
            continue;
        }
        out.push_str(&format!(
            "    self.{} = {}\n",
            field.name,
            emit_expr(
                field.default.as_ref().unwrap(),
                scopes,
                function_returns,
                ctx
            )
        ));
    }
}

fn emit_class(
    class: &ClassDef,
    program: &[Top],
    out: &mut String,
    scopes: &mut Vec<HashMap<String, Ty>>,
    function_returns: &HashMap<String, Ty>,
    ctx: &Ctx,
) -> () {
    let start = out.len();
    let parents = class
        .parents
        .iter()
        .filter(|parent| find_class(program, parent).is_some())
        .cloned()
        .collect::<Vec<_>>();
    if parents.len() > 1 {
        let mut chain = format!(
            "    local value = rawget({}, k)\n    if value == nil then\n        value = {}[k]\n    end\n",
            class.name, parents[0]
        );
        for parent in parents.iter().skip(1) {
            chain.push_str(&format!(
                "    if value == nil then\n        value = {}[k]\n    end\n",
                parent
            ));
        }
        chain.push_str("    return value\nend");
        out.push_str(&format!(
            "{} = setmetatable({{}}, {{ __index = function(t, k)\n{} }})\n",
            class.name, chain
        ));
    } else if let Some(parent) = parents.first() {
        out.push_str(&format!(
            "{} = setmetatable({{}}, {{ __index = {} }})\n",
            class.name, parent
        ));
    } else {
        out.push_str(&format!("{} = {{}}\n", class.name));
    }
    annotate_line(out, start, class.line, class.column);
    out.push_str(&format!("{}.__index = {}\n", class.name, class.name));
    out.push_str(&format!(
        "{}.__tandem_class = {}\n",
        class.name,
        lua_str(&class.name)
    ));
    let ancestors = collect_ancestors(class, program);
    if !ancestors.is_empty() {
        let list = ancestors
            .iter()
            .map(|name| lua_str(name))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "{}.__tandem_ancestors = {{ {list} }}\n",
            class.name
        ));
    }

    let effective = effective_constructor(program, class);
    let new_params = param_list(effective).join(", ");
    let init_args = param_list(effective).join(", ");
    let start = out.len();
    out.push_str(&format!("function {}.new({new_params})\n", class.name));
    annotate_line(out, start, class.line, class.column);
    out.push_str(&format!(
        "    local self = setmetatable({{}}, {})\n",
        class.name
    ));
    out.push_str("    __tandem_init_state[self] = {}\n");
    out.push_str(&format!("    self:__init({init_args})\n"));
    out.push_str("    return self\nend\n\n");

    let init_signature = class.constructor.as_ref().or_else(|| {
        parents
            .first()
            .and_then(|parent| effective_constructor(program, find_class(program, parent)?))
    });
    let init_params = param_list(init_signature).join(", ");
    let start = out.len();
    out.push_str(&format!("function {}:__init({init_params})\n", class.name));
    if let Some(constructor) = &class.constructor {
        annotate_line(out, start, constructor.line, constructor.column);
    } else {
        annotate_line(out, start, class.line, class.column);
    }
    out.push_str(&format!(
        "    if __tandem_init_state[self][{}] then return end\n    __tandem_init_state[self][{}] = true\n",
        lua_str(&class.name),
        lua_str(&class.name)
    ));
    let mut method_scope = HashMap::new();
    let self_ty = if class.generic.is_empty() {
        Ty::Struct(class.name.clone())
    } else {
        Ty::Applied(
            class.name.clone(),
            class.generic.iter().cloned().map(Ty::Generic).collect(),
        )
    };
    method_scope.insert("self".to_string(), self_ty.clone());
    if let Some(signature) = init_signature {
        for param in &signature.params {
            method_scope.insert(param.name.clone(), param.ty.clone());
        }
    }
    scopes.push(method_scope);
    if let Some(parent) = parents.first() {
        out.push_str(&format!("    local super = {parent}\n"));
    }
    if let Some(constructor) = &class.constructor {
        emit_params(out, &constructor.params, scopes, function_returns, ctx);
    }
    if let Some(constructor) = &class.constructor {
        if !parents.is_empty() {
            if let Some(first) = constructor.body.first() {
                emit_stmts(
                    std::slice::from_ref(first),
                    out,
                    1,
                    scopes,
                    function_returns,
                    ctx,
                );
            }
            for parent in parents.iter().skip(1) {
                out.push_str(&format!("    {parent}.__init(self)\n"));
            }
            emit_fields(class, out, scopes, function_returns, ctx);
            if constructor.body.len() > 1 {
                emit_stmts(
                    &constructor.body[1..],
                    out,
                    1,
                    scopes,
                    function_returns,
                    ctx,
                );
            }
        } else {
            emit_fields(class, out, scopes, function_returns, ctx);
            emit_stmts(&constructor.body, out, 1, scopes, function_returns, ctx);
        }
    } else {
        if !parents.is_empty() {
            if init_args.is_empty() {
                out.push_str("    super.__init(self)\n");
            } else {
                out.push_str(&format!("    super.__init(self, {init_args})\n"));
            }
        }
        for parent in parents.iter().skip(1) {
            out.push_str(&format!("    {parent}.__init(self)\n"));
        }
        emit_fields(class, out, scopes, function_returns, ctx);
    }
    scopes.pop();
    out.push_str("end\n\n");

    for method in &class.methods {
        if method.is_getter || method.is_setter {
            continue;
        }
        let start = out.len();
        let params = method
            .params
            .iter()
            .map(|param| param.name.clone())
            .collect::<Vec<_>>()
            .join(", ");
        if method.is_static {
            out.push_str(&format!(
                "{}.{} = function({params})\n",
                class.name, method.name
            ));
            annotate_line(out, start, method.line, method.column);
            let mut static_scope = HashMap::new();
            for param in &method.params {
                static_scope.insert(param.name.clone(), param.ty.clone());
            }
            scopes.push(static_scope);
            emit_params(out, &method.params, scopes, function_returns, ctx);
            emit_stmts(&method.body, out, 1, scopes, function_returns, ctx);
            scopes.pop();
            out.push_str("end\n\n");
            continue;
        }
        out.push_str(&format!(
            "function {}:{}({params})\n",
            class.name,
            lua_method_name(&method.name)
        ));
        annotate_line(out, start, method.line, method.column);
        scopes.push(
            method
                .params
                .iter()
                .map(|param| (param.name.clone(), param.ty.clone()))
                .chain(std::iter::once(("self".into(), self_ty.clone())))
                .collect(),
        );
        if let Some(parent) = parents.first() {
            out.push_str(&format!("    local super = {parent}\n"));
        }
        emit_params(out, &method.params, scopes, function_returns, ctx);
        emit_stmts(&method.body, out, 1, scopes, function_returns, ctx);
        scopes.pop();
        out.push_str("end\n\n");
    }
    for field in &class.fields {
        if !field.is_static || field.default.is_none() {
            continue;
        }
        out.push_str(&format!(
            "{}.{} = {}\n",
            class.name,
            field.name,
            emit_expr(
                field.default.as_ref().unwrap(),
                scopes,
                function_returns,
                ctx
            )
        ));
    }

    let getters: Vec<&Fun> = class.methods.iter().filter(|m| m.is_getter).collect();
    let setters: Vec<&Fun> = class.methods.iter().filter(|m| m.is_setter).collect();
    if !getters.is_empty() || !setters.is_empty() {
        out.push_str(&format!("{}.__tandem_getters = {{}}\n", class.name));
        out.push_str(&format!("{}.__tandem_setters = {{}}\n", class.name));
        for getter in getters {
            out.push_str(&format!(
                "{}.__tandem_getters.{} = function(self)\n",
                class.name, getter.name
            ));
            let mut method_scope = HashMap::new();
            method_scope.insert("self".to_string(), self_ty.clone());
            scopes.push(method_scope);
            emit_stmts(&getter.body, out, 1, scopes, function_returns, ctx);
            scopes.pop();
            out.push_str("end\n");
        }
        for setter in setters {
            let params = setter
                .params
                .iter()
                .map(|param| param.name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!(
                "{}.__tandem_setters.{} = function(self, {params})\n",
                class.name, setter.name
            ));
            let mut method_scope = HashMap::new();
            method_scope.insert("self".to_string(), self_ty.clone());
            for param in &setter.params {
                method_scope.insert(param.name.clone(), param.ty.clone());
            }
            scopes.push(method_scope);
            emit_params(out, &setter.params, scopes, function_returns, ctx);
            emit_stmts(&setter.body, out, 1, scopes, function_returns, ctx);
            scopes.pop();
            out.push_str("end\n");
        }
    }
}


fn lua_method_name(name: &str) -> String {
    if let Some(op) = name.strip_prefix("operator") {
        if let Some(suffix) = match op {
            "+" => Some("plus"),
            "-" => Some("minus"),
            "*" => Some("mul"),
            "/" => Some("div"),
            "%" => Some("mod"),
            "==" => Some("eq"),
            "!=" => Some("ne"),
            "<" => Some("lt"),
            ">" => Some("gt"),
            "<=" => Some("le"),
            ">=" => Some("ge"),
            ".." => Some("concat"),
            _ => None,
        } {
            return format!("operator_{suffix}");
        }
    }
    name.to_string()
}

fn collect_ancestors(class: &ClassDef, program: &[Top]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut stack = class.parents.clone();
    while let Some(parent) = stack.pop() {
        if names.contains(&parent) {
            continue;
        }
        names.push(parent.clone());
        if let Some(definition) = find_class(program, &parent) {
            stack.extend(definition.parents.clone());
        }
    }
    names
}

fn ordered_classes(program: &[Top]) -> Vec<String> {
    let definitions = program
        .iter()
        .filter_map(|top| match top {
            Top::Class(class) => Some((class.name.clone(), class)),
            _ => None,
        })
        .collect::<HashMap<_, _>>();

    fn visit<'a>(
        name: &str,
        definitions: &HashMap<String, &'a ClassDef>,
        visiting: &mut HashSet<String>,
        emitted: &mut HashSet<String>,
        ordered: &mut Vec<String>,
    ) {
        if emitted.contains(name) || !visiting.insert(name.to_string()) {
            return;
        }
        if let Some(class) = definitions.get(name) {
            for parent in &class.parents {
                visit(parent, definitions, visiting, emitted, ordered);
            }
            ordered.push(name.to_string());
        }
        visiting.remove(name);
        emitted.insert(name.to_string());
    }

    let mut ordered = Vec::new();
    let mut visiting = HashSet::new();
    let mut emitted = HashSet::new();
    for (name, _) in definitions.iter() {
        visit(
            name,
            &definitions,
            &mut visiting,
            &mut emitted,
            &mut ordered,
        );
    }
    ordered
}

pub(crate) fn generate(program: &[Top], modules: &[(String, Vec<Top>)]) -> String {
    let mut out = String::from("-- Generated by Tandem 0.2.0. Do not edit this file.\n");
    for (id, module_program) in modules {
        out.push_str(&format!("package.preload[{}] = function()\n", lua_str(id)));
        out.push_str(&emit_program(module_program, true));
        out.push_str("end\n\n");
    }
    out.push_str(&emit_program(program, false));
    out
}

fn emit_program(program: &[Top], module_mode: bool) -> String {
    let mut globals = HashMap::new();
    let mut function_returns = HashMap::new();
    let class_map = collect_classes(program);
    let enum_map = collect_enums(program);
    let union_map = collect_unions(program);
    let ctx = Ctx {
        classes: &class_map,
        enums: &enum_map,
        unions: &union_map,
    };
    let mut class_names = Vec::new();
    for top in program {
        if let Top::Class(class) = top {
            class_names.push(class.name.clone());
        }
    }
    let mut function_names = Vec::new();
    let mut class_names = Vec::new();
    let mut local_globals = Vec::new();
    let mut export_names = Vec::new();
    for top in program {
        match top {
            Top::Interface(_) | Top::Trait(_) | Top::Mixin(_) => {}
            Top::Enum(_) | Top::Union(_) => {}
            Top::Fun(function) => {
                function_returns.insert(function.name.clone(), function.ret.clone());
                function_names.push(function.name.clone());
                if module_mode {
                    export_names.push(function.name.clone());
                }
            }
            Top::Stmt(Stmt {
                kind: StmtKind::Decl(decl),
                ..
            }) => {
                if !globals.contains_key(&decl.name) {
                    let ty = declaration_type(decl, &[], &function_returns, &ctx);
                    globals.insert(decl.name.clone(), ty);
                }
                if (decl.is_local || module_mode) && !local_globals.contains(&decl.name) {
                    local_globals.push(decl.name.clone());
                }
                if module_mode && !export_names.contains(&decl.name) {
                    export_names.push(decl.name.clone());
                }
            }
            Top::Stmt(_) => {}
            Top::Class(class) => {
                class_names.push(class.name.clone());
                if module_mode && !export_names.contains(&class.name) {
                    export_names.push(class.name.clone());
                }
            }
            Top::Struct(_) => {}
            Top::Import { .. } => {}
        }
    }

    let mut scopes = vec![globals];
    let unstable_globals = retyped_global_names(program);
    let mut out = String::new();
    let mut locals: Vec<String> = Vec::new();
    if module_mode {
        locals.extend(function_names.iter().cloned());
        locals.extend(local_globals.iter().cloned());
    } else {
        locals.extend(local_globals.iter().cloned());
    }
    locals.extend(class_names.iter().cloned());
    if !locals.is_empty() {
        out.push_str(&format!("local {}\n", locals.join(", ")));
    }
    if !class_names.is_empty() {
        out.push_str("local __tandem_init_state = setmetatable({}, { __mode = \"k\" })\n\n");
        out.push_str(
            "local function __tandem_instanceof(value, name)\n    local mt = getmetatable(value)\n    if not mt then return false end\n    if mt.__tandem_class == name then return true end\n    for _, ancestor in ipairs(mt.__tandem_ancestors or {}) do\n        if ancestor == name then return true end\n    end\n    return false\nend\n\n",
        );
    }
    for class_name in ordered_classes(program) {
        if let Some(def) = program.iter().find_map(|top| {
            if let Top::Class(c) = top {
                if c.name == class_name {
                    Some(c)
                } else {
                    None
                }
            } else {
                None
            }
        }) {
            emit_class(
                def,
                program,
                &mut out,
                &mut scopes,
                &function_returns,
                &ctx,
            );
        }
    }
    for t in program {
        match t {
            Top::Fun(f) => {
                let outer_scopes = scopes.clone();
                if let Some(global_scope) = scopes.first_mut() {
                    for name in &unstable_globals {
                        global_scope.insert(name.clone(), Ty::Any);
                    }
                }
                let params = f
                    .params
                    .iter()
                    .map(|p| p.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ");
                let start = out.len();
                if module_mode {
                    out.push_str(&format!("{} = function({params})\n", f.name));
                } else {
                    out.push_str(&format!("function {}({params})\n", f.name));
                }
                annotate_line(&mut out, start, f.line, f.column);
                let function_scope = f
                    .params
                    .iter()
                    .map(|param| (param.name.clone(), param.ty.clone()))
                    .collect::<HashMap<_, _>>();
                scopes.push(function_scope);
                emit_params(&mut out, &f.params, &scopes, &function_returns, &ctx);
                emit_stmts(
                    &f.body,
                    &mut out,
                    1,
                    &mut scopes,
                    &function_returns,
                    &ctx,
                );
                scopes.pop();
                scopes = outer_scopes;
                out.push_str("end\n\n");
            }
            Top::Stmt(Stmt {
                kind: StmtKind::Decl(decl),
                line,
                column,
            }) => {
                let start = out.len();
                let value = emit_list_replacement(
                    &decl.name,
                    &decl.value,
                    &scopes,
                    &function_returns,
                    &ctx,
                )
                .unwrap_or_else(|| emit_expr(&decl.value, &scopes, &function_returns, &ctx));
                out.push_str(&format!("{} = {value}\n", decl.name));
                annotate_line(&mut out, start, *line, *column);
                if let Expr::Cast(_, ty) = &decl.value {
                    update_type(&decl.name, ty.clone(), &mut scopes);
                }
            }
            Top::Stmt(s) => emit_stmts(
                std::slice::from_ref(s),
                &mut out,
                0,
                &mut scopes,
                &function_returns,
                &ctx,
            ),
            Top::Class(_) => {}
            Top::Struct(_) => {}
            Top::Interface(_) => {}
            Top::Trait(_) => {}
            Top::Enum(_) => {}
            Top::Union(_) => {}
            Top::Mixin(_) => {}
            Top::Import { .. } => {}
        }
    }
    if module_mode {
        if export_names.is_empty() {
            out.push_str("return {}\n");
        } else {
            out.push_str(&format!(
                "return {{ {} }}\n",
                export_names
                    .iter()
                    .map(|name| format!("{name} = {name}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    out
}
