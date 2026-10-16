use std::collections::HashMap;

use crate::compiler::ast::{parse_printf_format, Expr, FormatPart, Stmt, Top, Ty};

#[derive(Clone)]
struct FnSig {
    args: Vec<Ty>,
    required: usize,
    ret: Ty,
}
#[derive(Clone)]
struct Binding {
    ty: Ty,
    mutable: bool,
}
struct Checker {
    scopes: Vec<HashMap<String, Binding>>,
    funcs: HashMap<String, FnSig>,
}
impl Checker {
    fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            funcs: HashMap::new(),
        }
    }
    fn push(&mut self) {
        self.scopes.push(HashMap::new());
    }
    fn pop(&mut self) {
        self.scopes.pop();
    }
    fn lookup(&self, n: &str) -> Option<Binding> {
        self.scopes.iter().rev().find_map(|s| s.get(n).cloned())
    }
    fn declare(&mut self, n: &str, t: Ty, mutable: bool) -> Result<(), String> {
        let s = self.scopes.last_mut().unwrap();
        if s.contains_key(n) {
            return Err(format!("duplicate variable '{n}' in this scope"));
        }
        s.insert(n.into(), Binding { ty: t, mutable });
        Ok(())
    }
    fn expr(&self, e: &Expr) -> Result<Ty, String> {
        match e {
            Expr::Int(_) => Ok(Ty::Int),
            Expr::Float(_) => Ok(Ty::Float),
            Expr::Str(_) => Ok(Ty::Str),
            Expr::Bool(_) => Ok(Ty::Bool),
            Expr::Var(n) => self
                .lookup(n)
                .map(|binding| binding.ty)
                .ok_or_else(|| format!("unknown variable '{n}'")),
            Expr::Unary(op, x) => {
                let t = self.expr(x)?;
                match (op.as_str(), t) {
                    ("!", Ty::Bool) => Ok(Ty::Bool),
                    ("-", Ty::Int) => Ok(Ty::Int),
                    ("-", Ty::Float) => Ok(Ty::Float),
                    _ => Err(format!("operator {op} is not valid for this type")),
                }
            }
            Expr::Bin(a, op, b) => {
                let x = self.expr(a)?;
                let y = self.expr(b)?;
                match op.as_str() {
                    "+" | "-" | "*" | "/" | "%" if x == y && (x == Ty::Int || x == Ty::Float) => {
                        Ok(x)
                    }
                    "+" if x == Ty::Str && y == Ty::Str => Ok(Ty::Str),
                    "," if x == Ty::Str && y == Ty::Str => Ok(Ty::Str),
                    "==" | "!=" if x == y && x != Ty::Void => Ok(Ty::Bool),
                    "<" | ">" | "<=" | ">="
                        if x == y && (x == Ty::Int || x == Ty::Float || x == Ty::Str) =>
                    {
                        Ok(Ty::Bool)
                    }
                    "&&" | "||" if x == Ty::Bool && y == Ty::Bool => Ok(Ty::Bool),
                    _ => Err(format!("operator {op} cannot combine {x:?} and {y:?}")),
                }
            }
            Expr::Call(n, args) => {
                if n == "print" {
                    for a in args {
                        self.expr(a)?;
                    }
                    return Ok(Ty::Void);
                }
                if n == "printf" {
                    let [Expr::Str(format)] = args.as_slice() else {
                        return Err("printf expects one string literal argument".into());
                    };
                    for part in parse_printf_format(format)? {
                        if let FormatPart::Variable(name) = part {
                            self.lookup(&name).ok_or_else(|| {
                                format!("printf references unknown variable '{name}'")
                            })?;
                        }
                    }
                    return Ok(Ty::Void);
                }
                let f = self
                    .funcs
                    .get(n)
                    .ok_or_else(|| format!("unknown function '{n}'"))?;
                if args.len() < f.required || args.len() > f.args.len() {
                    return Err(format!(
                        "function '{n}' expects {}..{} arguments, got {}",
                        f.required,
                        f.args.len(),
                        args.len()
                    ));
                }
                for (a, t) in args.iter().zip(&f.args) {
                    let got = self.expr(a)?;
                    if &got != t {
                        return Err(format!("argument to '{n}' expects {t:?}, got {got:?}"));
                    }
                }
                Ok(f.ret.clone())
            }
        }
    }
    fn stmt(&mut self, s: &Stmt, ret: &Ty) -> Result<(), String> {
        match s {
            Stmt::Decl(decl) => {
                let got = self.expr(&decl.value)?;
                if got != decl.ty {
                    return Err(format!(
                        "variable '{}' declared {:?}, initializer is {got:?}",
                        decl.name, decl.ty
                    ));
                }
                self.declare(&decl.name, decl.ty.clone(), !decl.is_const)
            }
            Stmt::Assign(n, op, e) => {
                let binding = self
                    .lookup(n)
                    .ok_or_else(|| format!("assignment to unknown variable '{n}'"))?;
                if !binding.mutable {
                    return Err(format!("cannot assign to constant '{n}'"));
                }
                let t = binding.ty;
                let r = self.expr(e)?;
                let expected = if op == "=" {
                    t.clone()
                } else {
                    if t != Ty::Int && t != Ty::Float {
                        return Err(format!(
                            "compound assignment {op} requires a numeric variable"
                        ));
                    }
                    t.clone()
                };
                if r != expected {
                    return Err(format!(
                        "assignment to '{n}' expects {expected:?}, got {r:?}"
                    ));
                }
                Ok(())
            }
            Stmt::Expr(e) => {
                self.expr(e)?;
                Ok(())
            }
            Stmt::Block(body) => {
                self.push();
                for stmt in body {
                    self.stmt(stmt, ret)?;
                }
                self.pop();
                Ok(())
            }
            Stmt::If(c, a, b) => {
                if self.expr(c)? != Ty::Bool {
                    return Err("if condition must be bool".into());
                }
                self.push();
                for x in a {
                    self.stmt(x, ret)?;
                }
                self.pop();
                self.push();
                for x in b {
                    self.stmt(x, ret)?;
                }
                self.pop();
                Ok(())
            }
            Stmt::While(c, b) => {
                if self.expr(c)? != Ty::Bool {
                    return Err("while condition must be bool".into());
                }
                self.push();
                for x in b {
                    self.stmt(x, ret)?;
                }
                self.pop();
                Ok(())
            }
            Stmt::For {
                variable,
                start,
                stop,
                step,
                body,
            } => {
                for value in [start, stop, step] {
                    if self.expr(value)? != Ty::Int {
                        return Err("for loop start, stop, and step must be int".into());
                    }
                }
                self.push();
                self.declare(variable, Ty::Int, true)?;
                for stmt in body {
                    self.stmt(stmt, ret)?;
                }
                self.pop();
                Ok(())
            }
            Stmt::Return(e) => match (e, ret) {
                (None, Ty::Void) => Ok(()),
                (Some(x), r) if self.expr(x)? == *r => Ok(()),
                (None, _) => Err("return value required".into()),
                (Some(_), Ty::Void) => Err("void function cannot return a value".into()),
                (Some(x), r) => Err(format!("return expects {r:?}, got {:?}", self.expr(x)?)),
            },
        }
    }
}

pub(crate) fn check(program: &[Top]) -> Result<(), String> {
    let mut c = Checker::new();
    c.funcs.insert(
        "print".into(),
        FnSig {
            args: vec![],
            required: 0,
            ret: Ty::Void,
        },
    );
    c.funcs.insert(
        "printf".into(),
        FnSig {
            args: vec![Ty::Str],
            required: 1,
            ret: Ty::Void,
        },
    );
    for t in program {
        match t {
            Top::Stmt(Stmt::Decl(decl)) => {
                c.declare(&decl.name, decl.ty.clone(), !decl.is_const)?;
            }
            _ => {}
        }
    }
    for t in program {
        if let Top::Fun(f) = t {
            if c.funcs.contains_key(&f.name) {
                return Err(format!(
                    "function '{}' is already defined or reserved",
                    f.name
                ));
            }
            let required = f.params.iter().take_while(|p| p.default.is_none()).count();
            c.funcs.insert(
                f.name.clone(),
                FnSig {
                    args: f.params.iter().map(|p| p.ty.clone()).collect(),
                    required,
                    ret: f.ret.clone(),
                },
            );
        }
    }
    for t in program {
        match t {
            Top::Fun(f) => {
                c.push();
                for p in &f.params {
                    if let Some(d) = &p.default {
                        let got = c.expr(d)?;
                        if got != p.ty {
                            return Err(format!(
                                "default value for parameter '{}' expects {:?}, got {:?}",
                                p.name, p.ty, got
                            ));
                        }
                    }
                    c.declare(&p.name, p.ty.clone(), true)?;
                }
                for s in &f.body {
                    c.stmt(s, &f.ret)?;
                }
                c.pop();
            }
            Top::Stmt(Stmt::Decl(decl)) => {
                let got = c.expr(&decl.value)?;
                if got != decl.ty {
                    return Err(format!(
                        "variable '{}' declared {:?}, initializer is {got:?}",
                        decl.name, decl.ty
                    ));
                }
            }
            Top::Stmt(s) => c.stmt(s, &Ty::Void)?,
        }
    }
    Ok(())
}
