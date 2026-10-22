use std::collections::HashMap;

use crate::compiler::ast::{
    module_basename, parse_printf_format, retyped_global_names, Expr, FormatPart, ListElement, Stmt,
    Top, Ty,
};

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

fn inferred_expr_type(
    expr: &Expr,
    scopes: &[HashMap<String, Ty>],
    function_returns: &HashMap<String, Ty>,
) -> Option<Ty> {
    match expr {
        Expr::Int(_) => Some(Ty::Int),
        Expr::Float(_) => Some(Ty::Float),
        Expr::Str(_) => Some(Ty::Str),
        Expr::Bool(_) => Some(Ty::Bool),
        Expr::Var(name) => lookup_type(name, scopes),
        Expr::Cast(_, ty) => Some(ty.clone()),
        Expr::Unary(_, value) => inferred_expr_type(value, scopes, function_returns),
        Expr::Bin(left, op, right) => {
            let left_ty = inferred_expr_type(left, scopes, function_returns)?;
            let right_ty = inferred_expr_type(right, scopes, function_returns)?;
            if left_ty == right_ty
                && (left_ty == Ty::Str
                    || left_ty == Ty::Int
                    || left_ty == Ty::Float
                    || left_ty == Ty::Bool)
            {
                Some(if matches!(op.as_str(), "==" | "!=" | "<" | ">" | "<=" | ">=" | "&&" | "||") {
                    Ty::Bool
                } else {
                    left_ty
                })
            } else {
                None
            }
        }
        Expr::Call(name, _) => function_returns.get(name).cloned(),
        Expr::Member(_, _) | Expr::MethodCall(_, _, _) => None,
        Expr::Index(base, index) => match inferred_expr_type(base, scopes, function_returns)? {
            Ty::List(element_ty) => Some(*element_ty),
            Ty::Tuple(Some(elements)) => match index.as_ref() {
                Expr::Int(index) if *index > 0 => elements.get(*index as usize - 1).cloned(),
                _ => Some(Ty::Any),
            },
            Ty::Tuple(None) | Ty::Table | Ty::Dict => Some(Ty::Any),
            _ => None,
        },
        Expr::Table(_) => Some(Ty::Table),
        Expr::List(values) => {
            let mut element_ty = None;
            for value in values {
                let ListElement::Value(value) = value else {
                    return None;
                };
                let ty = inferred_expr_type(value, scopes, function_returns)?;
                if element_ty.as_ref().is_some_and(|element| element != &ty) {
                    return None;
                }
                element_ty = Some(ty);
            }
            Some(Ty::List(Box::new(element_ty.unwrap_or(Ty::Any))))
        }
        Expr::Tuple(values) => values
            .iter()
            .map(|value| inferred_expr_type(value, scopes, function_returns))
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

fn is_string_expr(
    expr: &Expr,
    scopes: &[HashMap<String, Ty>],
    function_returns: &HashMap<String, Ty>,
) -> bool {
    match expr {
        Expr::Str(_) => true,
        Expr::Var(name) => lookup_type(name, scopes) == Some(Ty::Str),
        Expr::Bin(left, op, right) if op == "+" || op == "," => {
            is_string_expr(left, scopes, function_returns)
                && is_string_expr(right, scopes, function_returns)
        }
        Expr::Call(name, _) => function_returns.get(name) == Some(&Ty::Str),
        Expr::Cast(_, Ty::Str) => true,
        Expr::Index(_, _) => {
            inferred_expr_type(expr, scopes, function_returns) == Some(Ty::Str)
        }
        _ => false,
    }
}

fn emit_expr(
    e: &Expr,
    scopes: &[HashMap<String, Ty>],
    function_returns: &HashMap<String, Ty>,
) -> String {
    match e {
        Expr::Int(n) => n.to_string(),
        Expr::Float(n) => n.to_string(),
        Expr::Str(s) => lua_str(s),
        Expr::Bool(b) => b.to_string(),
        Expr::Var(n) => n.clone(),
        Expr::Unary(op, x) => format!(
            "({}{})",
            if op == "!" { "not " } else { op },
            emit_expr(x, scopes, function_returns)
        ),
        Expr::Bin(a, op, b) => {
            let lua_op = match op.as_str() {
                "&&" => "and",
                "||" => "or",
                "," => "..",
                "+" if is_string_expr(a, scopes, function_returns)
                    && is_string_expr(b, scopes, function_returns) =>
                {
                    ".."
                }
                _ => op,
            };
            format!(
                "({} {} {})",
                emit_expr(a, scopes, function_returns),
                lua_op,
                emit_expr(b, scopes, function_returns)
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
                .map(|arg| emit_expr(arg, scopes, function_returns))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::Member(base, name) => format!("{}.{}", emit_expr(base, scopes, function_returns), name),
        Expr::MethodCall(base, name, args) => format!(
            "{}.{}({})",
            emit_expr(base, scopes, function_returns),
            name,
            args.iter()
                .map(|arg| emit_expr(arg, scopes, function_returns))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::Index(base, index) => format!(
            "{}[{}]",
            emit_expr(base, scopes, function_returns),
            emit_expr(index, scopes, function_returns)
        ),
        Expr::Cast(value, _) => emit_expr(value, scopes, function_returns),
        Expr::Table(fields) => format!(
            "{{ {} }}",
            fields
                .iter()
                .map(|(name, value)| {
                    let value = emit_expr(value, scopes, function_returns);
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
                    ListElement::Value(value) => emit_expr(value, scopes, function_returns),
                    ListElement::Previous => unreachable!("list preservation needs its old binding"),
                })
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::Tuple(values) => format!(
            "{{ {} }}",
            values
                .iter()
                .map(|value| emit_expr(value, scopes, function_returns))
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
            ListElement::Value(value) => emit_expr(value, scopes, function_returns),
            ListElement::Previous => format!(
                "(function(v) if v == nil then error(\"list preservation index {} is out of range\") end return v end)(old[{}])",
                index + 1,
                index + 1
            ),
        })
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!("(function(old) return {{ {elements} }} end)({name})"))
}

fn emit_stmts(
    stmts: &[Stmt],
    out: &mut String,
    indent: usize,
    scopes: &mut Vec<HashMap<String, Ty>>,
    function_returns: &HashMap<String, Ty>,
) {
    let pad = "    ".repeat(indent);
    for s in stmts {
        match s {
            Stmt::Decl(decl) => {
                let value = emit_list_replacement(
                    &decl.name,
                    &decl.value,
                    scopes,
                    function_returns,
                )
                .unwrap_or_else(|| emit_expr(&decl.value, scopes, function_returns));
                let ty = inferred_expr_type(&decl.value, scopes, function_returns)
                    .unwrap_or_else(|| decl.ty.clone());
                let current_scope = scopes.last_mut().unwrap();
                if current_scope.contains_key(&decl.name) {
                    out.push_str(&format!("{pad}{} = {value}\n", decl.name));
                } else {
                    out.push_str(&format!("{pad}local {} = {value}\n", decl.name));
                }
                current_scope.insert(decl.name.clone(), ty);
            }
            Stmt::Require(path) => {
                let namespace = module_basename(path);
                out.push_str(&format!(
                    "{pad}local {namespace} = require({})\n",
                    lua_str(path)
                ));
                scopes
                    .last_mut()
                    .unwrap()
                    .insert(namespace, Ty::Module(path.clone()));
            }
            Stmt::Assign(n, op, e) => {
                let value = emit_list_replacement(n, e, scopes, function_returns)
                    .unwrap_or_else(|| emit_expr(e, scopes, function_returns));
                if op == "=" {
                    out.push_str(&format!("{pad}{n} = {value}\n"));
                    if let Expr::Cast(_, ty) = e {
                        update_type(n, ty.clone(), scopes);
                    }
                } else {
                    let base = op.trim_end_matches('=');
                    out.push_str(&format!("{pad}{n} = {n} {base} {value}\n"));
                }
            }
            Stmt::AssignMember(base, name, op, e) => {
                let target = format!("{}.{}", emit_expr(base, scopes, function_returns), name);
                let value = emit_expr(e, scopes, function_returns);
                if op == "=" {
                    out.push_str(&format!("{pad}{target} = {value}\n"));
                } else {
                    let base_op = op.trim_end_matches('=');
                    out.push_str(&format!("{pad}{target} = {target} {base_op} {value}\n"));
                }
            }
            Stmt::AssignIndex(base, index, op, e) => {
                let target = format!(
                    "{}[{}]",
                    emit_expr(base, scopes, function_returns),
                    emit_expr(index, scopes, function_returns)
                );
                let value = emit_expr(e, scopes, function_returns);
                if op == "=" {
                    out.push_str(&format!("{pad}{target} = {value}\n"));
                } else {
                    let base_op = op.trim_end_matches('=');
                    out.push_str(&format!("{pad}{target} = {target} {base_op} {value}\n"));
                }
            }
            Stmt::Expr(e) => out.push_str(&format!(
                "{pad}{}\n",
                emit_expr(e, scopes, function_returns)
            )),
            Stmt::Block(body) => {
                out.push_str(&format!("{pad}do\n"));
                scopes.push(HashMap::new());
                emit_stmts(body, out, indent + 1, scopes, function_returns);
                scopes.pop();
                out.push_str(&format!("{pad}end\n"));
            }
            Stmt::If(c, a, b) => {
                let before = scopes.clone();
                out.push_str(&format!(
                    "{pad}if {} then\n",
                    emit_expr(c, scopes, function_returns)
                ));
                scopes.push(HashMap::new());
                emit_stmts(a, out, indent + 1, scopes, function_returns);
                scopes.pop();
                let yes_types = scopes.clone();
                *scopes = before.clone();
                if !b.is_empty() {
                    out.push_str(&format!("{pad}else\n"));
                    scopes.push(HashMap::new());
                    emit_stmts(b, out, indent + 1, scopes, function_returns);
                    scopes.pop();
                }
                let no_types = scopes.clone();
                merge_types(scopes, &yes_types, &no_types);
                out.push_str(&format!("{pad}end\n"));
            }
            Stmt::While(c, b) => {
                let before = scopes.clone();
                out.push_str(&format!(
                    "{pad}while {} do\n",
                    emit_expr(c, scopes, function_returns)
                ));
                scopes.push(HashMap::new());
                emit_stmts(b, out, indent + 1, scopes, function_returns);
                scopes.pop();
                let after = scopes.clone();
                merge_types(scopes, &before, &after);
                out.push_str(&format!("{pad}end\n"));
            }
            Stmt::For {
                variable,
                start,
                stop,
                step,
                body,
            } => {
                let before = scopes.clone();
                out.push_str(&format!(
                    "{pad}for {variable} = {}, {}, {} do\n",
                    emit_expr(start, scopes, function_returns),
                    emit_expr(stop, scopes, function_returns),
                    emit_expr(step, scopes, function_returns)
                ));
                let mut loop_scope = HashMap::new();
                loop_scope.insert(variable.clone(), Ty::Int);
                scopes.push(loop_scope);
                emit_stmts(body, out, indent + 1, scopes, function_returns);
                scopes.pop();
                let after = scopes.clone();
                merge_types(scopes, &before, &after);
                out.push_str(&format!("{pad}end\n"));
            }
            Stmt::Return(e) => out.push_str(&format!(
                "{pad}return{}\n",
                e.as_ref()
                    .map(|x| format!(" {}", emit_expr(x, scopes, function_returns)))
                    .unwrap_or_default()
            )),
        }
    }
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
    let mut function_names = Vec::new();
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
            Top::Stmt(Stmt::Decl(decl)) => {
                if !globals.contains_key(&decl.name) {
                    let ty = inferred_expr_type(&decl.value, &[], &function_returns)
                        .unwrap_or_else(|| decl.ty.clone());
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
            Top::Struct(_) => {}
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
    if !locals.is_empty() {
        out.push_str(&format!("local {}\n", locals.join(", ")));
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
                if module_mode {
                    out.push_str(&format!("{} = function({params})\n", f.name));
                } else {
                    out.push_str(&format!("function {}({params})\n", f.name));
                }
                let function_scope = f
                    .params
                    .iter()
                    .map(|param| (param.name.clone(), param.ty.clone()))
                    .collect::<HashMap<_, _>>();
                scopes.push(function_scope);
                for p in &f.params {
                    if let Some(d) = &p.default {
                        out.push_str(&format!(
                            "    if {} == nil then {} = {} end\n",
                            p.name,
                            p.name,
                            emit_expr(d, &scopes, &function_returns)
                        ));
                    }
                }
                emit_stmts(&f.body, &mut out, 1, &mut scopes, &function_returns);
                scopes.pop();
                scopes = outer_scopes;
                out.push_str("end\n\n");
            }
            Top::Stmt(Stmt::Decl(decl)) => {
                let value = emit_list_replacement(
                    &decl.name,
                    &decl.value,
                    &scopes,
                    &function_returns,
                )
                .unwrap_or_else(|| emit_expr(&decl.value, &scopes, &function_returns));
                out.push_str(&format!("{} = {value}\n", decl.name));
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
            ),
            Top::Struct(_) => {}
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
