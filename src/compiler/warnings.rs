use crate::compiler::ast::{
    parse_printf_format, Expr, FormatPart, Fun, ListElement, Stmt, StmtKind, Top,
};

#[derive(Clone)]
struct Binding {
    name: String,
    used: bool,
    warn_unused: bool,
}

struct WarnChecker {
    scopes: Vec<Vec<Binding>>,
    warnings: Vec<String>,
}

impl WarnChecker {
    fn new() -> Self {
        Self {
            scopes: vec![Vec::new()],
            warnings: Vec::new(),
        }
    }

    fn push(&mut self) {
        self.scopes.push(Vec::new());
    }

    fn pop(&mut self) {
        let scope = self.scopes.pop().expect("scope stack underflow");
        for binding in scope {
            if binding.warn_unused && !binding.used {
                self.warnings
                    .push(format!("unused variable '{}'", binding.name));
            }
        }
    }

    fn shadows_outer(&self, name: &str) -> bool {
        let outer = self.scopes.len().saturating_sub(1);
        self.scopes[..outer]
            .iter()
            .any(|scope| scope.iter().any(|binding| binding.name == name))
    }

    fn declared(&self, name: &str) -> bool {
        self.scopes
            .iter()
            .any(|scope| scope.iter().any(|binding| binding.name == name))
    }

    fn declare(&mut self, name: &str, warn_unused: bool) {
        if self.shadows_outer(name) {
            self.warnings
                .push(format!("variable '{name}' shadows an outer declaration"));
        }
        self.scopes
            .last_mut()
            .expect("scope stack empty")
            .push(Binding {
                name: name.to_owned(),
                used: false,
                warn_unused,
            });
    }

    fn use_var(&mut self, name: &str) {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(binding) = scope.iter_mut().rev().find(|binding| binding.name == name) {
                binding.used = true;
                return;
            }
        }
    }

    fn program(&mut self, program: &[Top]) {
        for top in program {
            if let Top::Stmt(Stmt {
                kind: StmtKind::Decl(decl),
                ..
            }) = top
            {
                self.declare(&decl.name, false);
            }
        }
        for top in program {
            match top {
                Top::Fun(function) => self.function(function),
                Top::Stmt(stmt) => self.stmt(stmt, false),
                Top::Class(class) => {
                    for field in &class.fields {
                        self.expr(&field.default);
                    }
                    for callable in class.methods.iter().chain(class.constructor.iter()) {
                        self.function(callable);
                    }
                }
                Top::Struct(_) => {}
                Top::Import { .. } => {}
            }
        }
    }

    fn function(&mut self, function: &Fun) {
        self.push();
        for param in &function.params {
            if let Some(default) = &param.default {
                self.expr(default);
            }
            self.declare(&param.name, true);
        }
        self.stmts(&function.body, true);
        self.pop();
    }

    fn stmts(&mut self, stmts: &[Stmt], declare: bool) {
        let mut returned = false;
        for statement in stmts {
            if returned {
                self.warnings
                    .push("unreachable statement after return".into());
                returned = false;
            }
            if matches!(&statement.kind, StmtKind::Return(_)) {
                returned = true;
            }
            self.stmt(statement, declare);
        }
    }

    fn stmt(&mut self, stmt: &Stmt, declare: bool) {
        match &stmt.kind {
            StmtKind::Decl(decl) => {
                self.expr(&decl.value);
                let already_in_scope = self
                    .scopes
                    .last()
                    .is_some_and(|scope| scope.iter().any(|binding| binding.name == decl.name));
                if declare && !already_in_scope {
                    self.declare(&decl.name, true);
                }
            }
            StmtKind::Assign(name, op, value) => {
                if op == "=" && !self.declared(name) && is_require_call(value) {
                    self.declare(name, true);
                }
                self.expr(value);
            }
            StmtKind::AssignMember(base, _, _, value) => {
                self.expr(base);
                self.expr(value);
            }
            StmtKind::AssignIndex(base, index, _, value) => {
                self.expr(base);
                self.expr(index);
                self.expr(value);
            }
            StmtKind::Expr(expr) => self.expr(expr),
            StmtKind::Block(body) => {
                self.push();
                self.stmts(body, true);
                self.pop();
            }
            StmtKind::If(condition, yes, no) => {
                self.expr(condition);
                self.push();
                self.stmts(yes, true);
                self.pop();
                self.push();
                self.stmts(no, true);
                self.pop();
            }
            StmtKind::While(condition, body) => {
                self.expr(condition);
                self.push();
                self.stmts(body, true);
                self.pop();
            }
            StmtKind::For {
                variable,
                start,
                stop,
                step,
                body,
            } => {
                self.expr(start);
                self.expr(stop);
                self.expr(step);
                self.push();
                self.declare(variable, true);
                self.stmts(body, true);
                self.pop();
            }
            StmtKind::Return(value) => {
                if let Some(value) = value {
                    self.expr(value);
                }
            }
        }
    }

    fn expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Var(name) => self.use_var(name),
            Expr::Unary(_, inner) => self.expr(inner),
            Expr::Cast(value, _) => self.expr(value),
            Expr::Bin(left, _, right) => {
                self.expr(left);
                self.expr(right);
            }
            Expr::Member(base, _) => self.expr(base),
            Expr::MethodCall(base, _, args) => {
                self.expr(base);
                for arg in args {
                    self.expr(arg);
                }
            }
            Expr::New(_, args) => {
                for arg in args {
                    self.expr(arg);
                }
            }
            Expr::SuperCall(args) | Expr::SuperMethod(_, args) => {
                for arg in args {
                    self.expr(arg);
                }
            }
            Expr::Call(name, args) => {
                if name == "printf" {
                    if let Some(Expr::Str(format)) = args.first() {
                        if let Ok(parts) = parse_printf_format(format) {
                            for part in parts {
                                if let FormatPart::Variable(variable) = part {
                                    self.use_var(&variable);
                                }
                            }
                        }
                    }
                }
                for arg in args {
                    self.expr(arg);
                }
            }
            Expr::Int(_) | Expr::Float(_) | Expr::Str(_) | Expr::Bool(_) => {}
            Expr::Index(base, index) => {
                self.expr(base);
                self.expr(index);
            }
            Expr::Table(fields) => {
                for (_, value) in fields {
                    self.expr(value);
                }
            }
            Expr::List(values) => {
                for value in values {
                    if let ListElement::Value(value) = value {
                        self.expr(value);
                    }
                }
            }
            Expr::Tuple(values) => {
                for value in values {
                    self.expr(value);
                }
            }
        }
    }
}

fn is_require_call(expr: &Expr) -> bool {
    matches!(expr, Expr::Call(name, args) if name == "require" && matches!(args.as_slice(), [Expr::Str(_)]))
}

pub(crate) fn check(program: &[Top]) -> Vec<String> {
    let mut checker = WarnChecker::new();
    checker.program(program);
    checker.pop();
    checker.warnings
}
