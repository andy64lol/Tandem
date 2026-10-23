use std::collections::{HashMap, HashSet};

use crate::compiler::ast::{
    parse_printf_format, retyped_global_names, ClassDef, Expr, FormatPart, Fun, ListElement, Stmt,
    StmtKind, TableShape, Top, Ty, VarDecl,
};
use crate::compiler::stdlib;

#[derive(Clone, Default)]
struct ClassInfo {
    parent: Option<String>,
    fields: HashMap<String, Ty>,
    methods: HashMap<String, Ty>,
}

type Classes = HashMap<String, ClassInfo>;

fn collect_classes(program: &[Top]) -> Classes {
    let mut classes = Classes::new();
    for top in program {
        let Top::Class(definition) = top else {
            continue;
        };
        classes.insert(
            definition.name.clone(),
            ClassInfo {
                parent: definition.parent.clone(),
                fields: definition
                    .fields
                    .iter()
                    .map(|field| (field.name.clone(), field.ty.clone()))
                    .collect(),
                methods: definition
                    .methods
                    .iter()
                    .map(|method| (method.name.clone(), method.ret.clone()))
                    .collect(),
            },
        );
    }
    classes
}

fn class_field(classes: &Classes, class_name: &str, field: &str) -> Option<Ty> {
    let mut current = Some(class_name);
    while let Some(name) = current {
        let class = classes.get(name)?;
        if let Some(ty) = class.fields.get(field) {
            return Some(ty.clone());
        }
        current = class.parent.as_deref();
    }
    None
}

fn class_method(classes: &Classes, class_name: &str, method: &str) -> Option<Ty> {
    let mut current = Some(class_name);
    while let Some(name) = current {
        let class = classes.get(name)?;
        if let Some(ty) = class.methods.get(method) {
            return Some(ty.clone());
        }
        current = class.parent.as_deref();
    }
    None
}

fn declaration_type(
    decl: &VarDecl,
    scopes: &[HashMap<String, Ty>],
    function_returns: &HashMap<String, Ty>,
    classes: &Classes,
) -> Ty {
    match &decl.ty {
        Ty::Auto => {
            inferred_value_type(&decl.value, scopes, function_returns, classes).unwrap_or(Ty::Any)
        }
        Ty::Table => {
            inferred_value_type(&decl.value, scopes, function_returns, classes).unwrap_or(Ty::Table)
        }
        Ty::Tuple(None) => inferred_expr_type(&decl.value, scopes, function_returns, classes)
            .unwrap_or(Ty::Tuple(None)),
        declared => declared.clone(),
    }
}

fn table_shape(
    expr: &Expr,
    scopes: &[HashMap<String, Ty>],
    function_returns: &HashMap<String, Ty>,
    classes: &Classes,
) -> Option<TableShape> {
    let Expr::Table(fields) = expr else {
        return None;
    };
    let mut shape = TableShape::default();
    for (name, value) in fields {
        let mut ty =
            inferred_expr_type(value, scopes, function_returns, classes).unwrap_or(Ty::Any);
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
    classes: &Classes,
) -> Option<Ty> {
    table_shape(expr, scopes, function_returns, classes)
        .map(|shape| Ty::TableShape(Box::new(shape)))
        .or_else(|| inferred_expr_type(expr, scopes, function_returns, classes))
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
    classes: &Classes,
) -> Option<Ty> {
    let Ty::Struct(class_name) = lookup_type("self", scopes)? else {
        return None;
    };
    if scopes.iter().skip(1).any(|scope| scope.contains_key(name)) {
        return None;
    }
    class_field(classes, &class_name, name)
}

fn is_implicit_field(name: &str, scopes: &[HashMap<String, Ty>], classes: &Classes) -> bool {
    implicit_field_type(name, scopes, classes).is_some()
}

fn inferred_expr_type(
    expr: &Expr,
    scopes: &[HashMap<String, Ty>],
    function_returns: &HashMap<String, Ty>,
    classes: &Classes,
) -> Option<Ty> {
    match expr {
        Expr::Int(_) => Some(Ty::Int),
        Expr::Float(_) => Some(Ty::Float),
        Expr::Str(_) => Some(Ty::Str),
        Expr::Bool(_) => Some(Ty::Bool),
        Expr::Var(name) => {
            implicit_field_type(name, scopes, classes).or_else(|| lookup_type(name, scopes))
        }
        Expr::New(name, _) => Some(Ty::Struct(name.clone())),
        Expr::SuperCall(_) => Some(Ty::Void),
        Expr::SuperMethod(name, _) => {
            let Ty::Struct(class_name) = lookup_type("self", scopes)? else {
                return None;
            };
            let parent = classes.get(&class_name)?.parent.as_deref()?;
            class_method(classes, parent, name)
        }
        Expr::Cast(_, ty) => Some(ty.clone()),
        Expr::Unary(_, value) => inferred_expr_type(value, scopes, function_returns, classes),
        Expr::Bin(left, op, right) => {
            let left_ty = inferred_expr_type(left, scopes, function_returns, classes)?;
            let right_ty = inferred_expr_type(right, scopes, function_returns, classes)?;
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
            match inferred_expr_type(base, scopes, function_returns, classes)? {
                Ty::Map(key_ty, value_ty) if *key_ty == Ty::Str => Some(*value_ty),
                Ty::TableShape(shape) => Some(shape.named.get(name).cloned().unwrap_or(Ty::Any)),
                Ty::Module(_) => package_value(base, name),
                Ty::Struct(class_name) => class_field(classes, &class_name, name),
                _ => None,
            }
        }
        Expr::MethodCall(base, name, _) => {
            if let Some(Ty::Struct(class_name)) =
                inferred_expr_type(base, scopes, function_returns, classes)
            {
                if let Some(ty) = class_method(classes, &class_name, name) {
                    return Some(ty);
                }
            }
            package_return(base, name)
        }
        Expr::Index(base, index) => {
            match inferred_expr_type(base, scopes, function_returns, classes)? {
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
                let ty = inferred_expr_type(value, scopes, function_returns, classes)?;
                if element_ty.as_ref().is_some_and(|element| element != &ty) {
                    return None;
                }
                element_ty = Some(ty);
            }
            Some(Ty::List(Box::new(element_ty.unwrap_or(Ty::Any))))
        }
        Expr::Tuple(values) => values
            .iter()
            .map(|value| inferred_expr_type(value, scopes, function_returns, classes))
            .collect::<Option<Vec<_>>>()
            .map(|types| Ty::Tuple(Some(types))),
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
    classes: &Classes,
) -> bool {
    match expr {
        Expr::Str(_) => true,
        Expr::Var(_) => {
            matches!(
                inferred_expr_type(expr, scopes, function_returns, classes),
                Some(Ty::Str)
            )
        }
        Expr::Bin(left, op, right) if op == "+" || op == "," => {
            is_string_expr(left, scopes, function_returns, classes)
                && is_string_expr(right, scopes, function_returns, classes)
        }
        Expr::Call(name, _) => {
            function_returns.get(name) == Some(&Ty::Str) || stdlib_return(name) == Some(Ty::Str)
        }
        Expr::Cast(_, Ty::Str) => true,
        Expr::Member(base, name) => {
            inferred_expr_type(expr, scopes, function_returns, classes) == Some(Ty::Str)
                || package_value(base, name) == Some(Ty::Str)
        }
        Expr::Index(_, _) => {
            inferred_expr_type(expr, scopes, function_returns, classes) == Some(Ty::Str)
        }
        Expr::MethodCall(base, name, _) => {
            inferred_expr_type(expr, scopes, function_returns, classes) == Some(Ty::Str)
                || package_return(base, name) == Some(Ty::Str)
        }
        _ => false,
    }
}

fn emit_expr(
    e: &Expr,
    scopes: &[HashMap<String, Ty>],
    function_returns: &HashMap<String, Ty>,
    classes: &Classes,
) -> String {
    match e {
        Expr::Int(n) => n.to_string(),
        Expr::Float(n) => n.to_string(),
        Expr::Str(s) => lua_str(s),
        Expr::Bool(b) => b.to_string(),
        Expr::Var(n) => {
            if is_implicit_field(n, scopes, classes) {
                format!("self.{n}")
            } else {
                n.clone()
            }
        }
        Expr::Unary(op, x) => format!(
            "({}{})",
            if op == "!" { "not " } else { op },
            emit_expr(x, scopes, function_returns, classes)
        ),
        Expr::Bin(a, op, b) => {
            let lua_op = match op.as_str() {
                "&&" => "and",
                "||" => "or",
                "," => "..",
                "+" if is_string_expr(a, scopes, function_returns, classes)
                    && is_string_expr(b, scopes, function_returns, classes) =>
                {
                    ".."
                }
                _ => op,
            };
            format!(
                "({} {} {})",
                emit_expr(a, scopes, function_returns, classes),
                lua_op,
                emit_expr(b, scopes, function_returns, classes)
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
                .map(|arg| emit_expr(arg, scopes, function_returns, classes))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::New(name, args) => format!(
            "{}.new({})",
            name,
            args.iter()
                .map(|arg| emit_expr(arg, scopes, function_returns, classes))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::SuperCall(args) => {
            let args = args
                .iter()
                .map(|arg| emit_expr(arg, scopes, function_returns, classes))
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
                .map(|arg| emit_expr(arg, scopes, function_returns, classes))
                .collect::<Vec<_>>();
            if args.is_empty() {
                format!("super.{name}(self)")
            } else {
                format!("super.{name}(self, {})", args.join(", "))
            }
        }
        Expr::Member(base, name) => {
            format!(
                "{}.{}",
                emit_expr(base, scopes, function_returns, classes),
                name
            )
        }
        Expr::MethodCall(base, name, args) => {
            let separator = match inferred_expr_type(base, scopes, function_returns, classes) {
                Some(Ty::Struct(class_name))
                    if class_method(classes, &class_name, name).is_some() =>
                {
                    ":"
                }
                _ => ".",
            };
            format!(
                "{}{}{}({})",
                emit_expr(base, scopes, function_returns, classes),
                separator,
                name,
                args.iter()
                    .map(|arg| emit_expr(arg, scopes, function_returns, classes))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        Expr::Index(base, index) => format!(
            "{}[{}]",
            emit_expr(base, scopes, function_returns, classes),
            emit_expr(index, scopes, function_returns, classes)
        ),
        Expr::Cast(value, _) => emit_expr(value, scopes, function_returns, classes),
        Expr::Table(fields) => format!(
            "{{ {} }}",
            fields
                .iter()
                .map(|(name, value)| {
                    let value = emit_expr(value, scopes, function_returns, classes);
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
                        emit_expr(value, scopes, function_returns, classes),
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
                .map(|value| emit_expr(value, scopes, function_returns, classes))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn emit_list_replacement(
    name: &str,
    expr: &Expr,
    scopes: &[HashMap<String, Ty>],
    function_returns: &HashMap<String, Ty>,
    classes: &Classes,
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
            ListElement::Value(value) => emit_expr(value, scopes, function_returns, classes),
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
        out.insert_str(start + offset, &format!(" -- tlua:{line}:{column}"));
    }
}

fn emit_stmts(
    stmts: &[Stmt],
    out: &mut String,
    indent: usize,
    scopes: &mut Vec<HashMap<String, Ty>>,
    function_returns: &HashMap<String, Ty>,
    classes: &Classes,
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
                    classes,
                )
                .unwrap_or_else(|| emit_expr(&decl.value, scopes, function_returns, classes));
                let ty = declaration_type(decl, scopes, function_returns, classes);
                let current_scope = scopes.last_mut().unwrap();
                if current_scope.contains_key(&decl.name) {
                    out.push_str(&format!("{pad}{} = {value}\n", decl.name));
                } else {
                    out.push_str(&format!("{pad}local {} = {value}\n", decl.name));
                }
                current_scope.insert(decl.name.clone(), ty);
            }
            StmtKind::Assign(n, op, e) => {
                let target = if is_implicit_field(n, scopes, classes) {
                    format!("self.{n}")
                } else {
                    n.clone()
                };
                let value = emit_list_replacement(&target, e, scopes, function_returns, classes)
                    .unwrap_or_else(|| emit_expr(e, scopes, function_returns, classes));
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
                let target = format!(
                    "{}.{}",
                    emit_expr(base, scopes, function_returns, classes),
                    name
                );
                let value = emit_expr(e, scopes, function_returns, classes);
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
                    emit_expr(base, scopes, function_returns, classes),
                    emit_expr(index, scopes, function_returns, classes)
                );
                let value = emit_expr(e, scopes, function_returns, classes);
                if op == "=" {
                    out.push_str(&format!("{pad}{target} = {value}\n"));
                } else {
                    let base_op = op.trim_end_matches('=');
                    out.push_str(&format!("{pad}{target} = {target} {base_op} {value}\n"));
                }
            }
            StmtKind::Expr(e) => out.push_str(&format!(
                "{pad}{}\n",
                emit_expr(e, scopes, function_returns, classes)
            )),
            StmtKind::Block(body) => {
                out.push_str(&format!("{pad}do\n"));
                scopes.push(HashMap::new());
                emit_stmts(body, out, indent + 1, scopes, function_returns, classes);
                scopes.pop();
                out.push_str(&format!("{pad}end\n"));
            }
            StmtKind::If(c, a, b) => {
                let before = scopes.clone();
                out.push_str(&format!(
                    "{pad}if {} then\n",
                    emit_expr(c, scopes, function_returns, classes)
                ));
                scopes.push(HashMap::new());
                emit_stmts(a, out, indent + 1, scopes, function_returns, classes);
                scopes.pop();
                let yes_types = scopes.clone();
                *scopes = before.clone();
                if !b.is_empty() {
                    out.push_str(&format!("{pad}else\n"));
                    scopes.push(HashMap::new());
                    emit_stmts(b, out, indent + 1, scopes, function_returns, classes);
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
                    emit_expr(c, scopes, function_returns, classes)
                ));
                scopes.push(HashMap::new());
                emit_stmts(b, out, indent + 1, scopes, function_returns, classes);
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
                    emit_expr(start, scopes, function_returns, classes),
                    emit_expr(stop, scopes, function_returns, classes),
                    emit_expr(step, scopes, function_returns, classes)
                ));
                let mut loop_scope = HashMap::new();
                loop_scope.insert(variable.clone(), Ty::Int);
                scopes.push(loop_scope);
                emit_stmts(body, out, indent + 1, scopes, function_returns, classes);
                scopes.pop();
                let after = scopes.clone();
                merge_types(scopes, &before, &after);
                out.push_str(&format!("{pad}end\n"));
            }
            StmtKind::Return(e) => out.push_str(&format!(
                "{pad}return{}\n",
                e.as_ref()
                    .map(|x| format!(" {}", emit_expr(x, scopes, function_returns, classes)))
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
    classes: &Classes,
) {
    for param in params {
        if let Some(default) = &param.default {
            out.push_str(&format!(
                "    if {} == nil then {} = {} end\n",
                param.name,
                param.name,
                emit_expr(default, scopes, function_returns, classes)
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
    let mut current = Some(class);
    while let Some(definition) = current {
        if let Some(constructor) = &definition.constructor {
            return Some(constructor);
        }
        current = definition
            .parent
            .as_deref()
            .and_then(|parent| find_class(program, parent));
    }
    None
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
    classes: &Classes,
) {
    for field in &class.fields {
        out.push_str(&format!(
            "    self.{} = {}\n",
            field.name,
            emit_expr(&field.default, scopes, function_returns, classes)
        ));
    }
}

fn emit_class(
    class: &ClassDef,
    program: &[Top],
    out: &mut String,
    scopes: &mut Vec<HashMap<String, Ty>>,
    function_returns: &HashMap<String, Ty>,
    classes: &Classes,
) {
    let start = out.len();
    if let Some(parent) = &class.parent {
        out.push_str(&format!(
            "{} = setmetatable({{}}, {{ __index = {} }})\n",
            class.name, parent
        ));
    } else {
        out.push_str(&format!("{} = {{}}\n", class.name));
    }
    annotate_line(out, start, class.line, class.column);
    out.push_str(&format!("{}.__index = {}\n", class.name, class.name));

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
    out.push_str(&format!("    self:__init({init_args})\n"));
    out.push_str("    return self\nend\n\n");

    let init_signature = class
        .constructor
        .as_ref()
        .or_else(|| class.parent.as_ref().and(effective));
    let init_params = param_list(init_signature).join(", ");
    let start = out.len();
    out.push_str(&format!("function {}:__init({init_params})\n", class.name));
    if let Some(constructor) = &class.constructor {
        annotate_line(out, start, constructor.line, constructor.column);
    } else {
        annotate_line(out, start, class.line, class.column);
    }
    let mut method_scope = HashMap::new();
    method_scope.insert("self".to_string(), Ty::Struct(class.name.clone()));
    if let Some(signature) = init_signature {
        for param in &signature.params {
            method_scope.insert(param.name.clone(), param.ty.clone());
        }
    }
    scopes.push(method_scope);
    if let Some(parent) = &class.parent {
        out.push_str(&format!("    local super = {parent}\n"));
    }
    if let Some(constructor) = &class.constructor {
        emit_params(out, &constructor.params, scopes, function_returns, classes);
    }
    if let Some(constructor) = &class.constructor {
        if class.parent.is_some() {
            if let Some(first) = constructor.body.first() {
                emit_stmts(
                    std::slice::from_ref(first),
                    out,
                    1,
                    scopes,
                    function_returns,
                    classes,
                );
            }
            emit_fields(class, out, scopes, function_returns, classes);
            if constructor.body.len() > 1 {
                emit_stmts(
                    &constructor.body[1..],
                    out,
                    1,
                    scopes,
                    function_returns,
                    classes,
                );
            }
        } else {
            emit_fields(class, out, scopes, function_returns, classes);
            emit_stmts(&constructor.body, out, 1, scopes, function_returns, classes);
        }
    } else {
        if class.parent.is_some() {
            if init_args.is_empty() {
                out.push_str("    super.__init(self)\n");
            } else {
                out.push_str(&format!("    super.__init(self, {init_args})\n"));
            }
        }
        emit_fields(class, out, scopes, function_returns, classes);
    }
    scopes.pop();
    out.push_str("end\n\n");

    for method in &class.methods {
        let start = out.len();
        let params = method
            .params
            .iter()
            .map(|param| param.name.clone())
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "function {}:{}({params})\n",
            class.name, method.name
        ));
        annotate_line(out, start, method.line, method.column);
        scopes.push(
            method
                .params
                .iter()
                .map(|param| (param.name.clone(), param.ty.clone()))
                .chain(std::iter::once((
                    "self".into(),
                    Ty::Struct(class.name.clone()),
                )))
                .collect(),
        );
        if let Some(parent) = &class.parent {
            out.push_str(&format!("    local super = {parent}\n"));
        }
        emit_params(out, &method.params, scopes, function_returns, classes);
        emit_stmts(&method.body, out, 1, scopes, function_returns, classes);
        scopes.pop();
        out.push_str("end\n\n");
    }
}

fn ordered_classes(program: &[Top]) -> Vec<&ClassDef> {
    fn visit<'a>(
        name: &str,
        definitions: &HashMap<String, &'a ClassDef>,
        visiting: &mut HashSet<String>,
        emitted: &mut HashSet<String>,
        ordered: &mut Vec<&'a ClassDef>,
    ) {
        if emitted.contains(name) || !visiting.insert(name.to_string()) {
            return;
        }
        if let Some(class) = definitions.get(name) {
            if let Some(parent) = &class.parent {
                visit(parent, definitions, visiting, emitted, ordered);
            }
            ordered.push(*class);
        }
        visiting.remove(name);
        emitted.insert(name.to_string());
    }

    let definitions = program
        .iter()
        .filter_map(|top| match top {
            Top::Class(class) => Some((class.name.clone(), class)),
            _ => None,
        })
        .collect::<HashMap<_, _>>();
    let mut ordered = Vec::new();
    let mut visiting = HashSet::new();
    let mut emitted = HashSet::new();
    for top in program {
        if let Top::Class(class) = top {
            visit(
                &class.name,
                &definitions,
                &mut visiting,
                &mut emitted,
                &mut ordered,
            );
        }
    }
    ordered
}

pub(crate) fn generate(program: &[Top], modules: &[(String, Vec<Top>)]) -> String {
    let mut out = String::from("-- Generated by TypeLua 0.1.1. Do not edit this file.\n");
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
    let classes = collect_classes(program);
    let mut function_names = Vec::new();
    let mut class_names = Vec::new();
    let mut local_globals = Vec::new();
    let mut export_names = Vec::new();
    for top in program {
        match top {
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
                    let ty = declaration_type(decl, &[], &function_returns, &classes);
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
            Top::Class(class) => class_names.push(class.name.clone()),
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
    for class in ordered_classes(program) {
        emit_class(
            class,
            program,
            &mut out,
            &mut scopes,
            &function_returns,
            &classes,
        );
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
                emit_params(&mut out, &f.params, &scopes, &function_returns, &classes);
                emit_stmts(
                    &f.body,
                    &mut out,
                    1,
                    &mut scopes,
                    &function_returns,
                    &classes,
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
                    &classes,
                )
                .unwrap_or_else(|| emit_expr(&decl.value, &scopes, &function_returns, &classes));
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
                &classes,
            ),
            Top::Class(_) => {}
            Top::Struct(_) => {}
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
