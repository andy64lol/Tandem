use std::collections::HashMap;

use crate::compiler::ast::{parse_printf_format, Expr, FormatPart, Stmt, Top, Ty};

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
    }
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
            Stmt::Decl(ty, n, e) => {
                let value = emit_expr(e, scopes, function_returns);
                out.push_str(&format!("{pad}local {n} = {value}\n"));
                scopes.last_mut().unwrap().insert(n.clone(), ty.clone());
            }
            Stmt::Assign(n, op, e) => {
                let value = emit_expr(e, scopes, function_returns);
                if op == "=" {
                    out.push_str(&format!("{pad}{n} = {value}\n"));
                } else {
                    let base = op.trim_end_matches('=');
                    out.push_str(&format!("{pad}{n} = {n} {base} {value}\n"));
                }
            }
            Stmt::Expr(e) => out.push_str(&format!(
                "{pad}{}\n",
                emit_expr(e, scopes, function_returns)
            )),
            Stmt::If(c, a, b) => {
                out.push_str(&format!(
                    "{pad}if {} then\n",
                    emit_expr(c, scopes, function_returns)
                ));
                scopes.push(HashMap::new());
                emit_stmts(a, out, indent + 1, scopes, function_returns);
                scopes.pop();
                if !b.is_empty() {
                    out.push_str(&format!("{pad}else\n"));
                    scopes.push(HashMap::new());
                    emit_stmts(b, out, indent + 1, scopes, function_returns);
                    scopes.pop();
                }
                out.push_str(&format!("{pad}end\n"));
            }
            Stmt::While(c, b) => {
                out.push_str(&format!(
                    "{pad}while {} do\n",
                    emit_expr(c, scopes, function_returns)
                ));
                scopes.push(HashMap::new());
                emit_stmts(b, out, indent + 1, scopes, function_returns);
                scopes.pop();
                out.push_str(&format!("{pad}end\n"));
            }
            Stmt::For {
                variable,
                start,
                stop,
                step,
                body,
            } => {
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

pub(crate) fn generate(program: &[Top]) -> String {
    let mut globals = HashMap::new();
    let mut function_returns = HashMap::new();
    for top in program {
        match top {
            Top::Fun(function) => {
                function_returns.insert(function.name.clone(), function.ret.clone());
            }
            Top::Stmt(Stmt::Decl(ty, name, _)) => {
                globals.insert(name.clone(), ty.clone());
            }
            Top::Stmt(_) => {}
        }
    }

    let mut scopes = vec![globals];
    let mut out = String::from("-- Generated by TypeLua 0.1.0. Do not edit this file.\n");
    for t in program {
        match t {
            Top::Fun(f) => {
                out.push_str(&format!(
                    "function {}({})\n",
                    f.name,
                    f.params
                        .iter()
                        .map(|p| p.name.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
                let mut function_scope = f
                    .params
                    .iter()
                    .map(|param| (param.name.clone(), param.ty.clone()))
                    .collect::<HashMap<_, _>>();
                scopes.push(std::mem::take(&mut function_scope));
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
                out.push_str("end\n\n");
            }
            Top::Stmt(Stmt::Decl(_, n, e)) => out.push_str(&format!(
                "{n} = {}\n",
                emit_expr(e, &scopes, &function_returns)
            )),
            Top::Stmt(s) => emit_stmts(
                std::slice::from_ref(s),
                &mut out,
                0,
                &mut scopes,
                &function_returns,
            ),
        }
    }
    out
}
