use crate::compiler::ast::{Expr, Fun, Param, Stmt, Top, Ty};
use crate::compiler::lexer::{Tok, Token};

struct Parser {
    t: Vec<Token>,
    i: usize,
}
impl Parser {
    fn cur(&self) -> &Token {
        &self.t[self.i]
    }
    fn bump(&mut self) -> Token {
        let x = self.t[self.i].clone();
        if !matches!(x.kind, Tok::Eof) {
            self.i += 1;
        }
        x
    }
    fn sym(&self, s: &str) -> bool {
        self.cur().kind == Tok::Sym(s.into())
    }
    fn kw(&self, s: &str) -> bool {
        self.cur().kind == Tok::Ident(s.into())
    }
    fn eat_sym(&mut self, s: &str) -> bool {
        if self.sym(s) {
            self.bump();
            true
        } else {
            false
        }
    }
    fn eat_kw(&mut self, s: &str) -> bool {
        if self.kw(s) {
            self.bump();
            true
        } else {
            false
        }
    }
    fn expect_sym(&mut self, s: &str) -> Result<(), String> {
        if self.eat_sym(s) {
            Ok(())
        } else {
            Err(self.err(format!("expected '{s}'")))
        }
    }
    fn expect_ident(&mut self) -> Result<String, String> {
        match self.bump().kind {
            Tok::Ident(s) => Ok(s),
            _ => Err(self.err("expected identifier".into())),
        }
    }
    fn err(&self, msg: String) -> String {
        format!("{msg} at byte {}", self.cur().pos)
    }
    fn ty(&mut self) -> Result<Ty, String> {
        let n = self.expect_ident()?;
        Ty::parse(&n).ok_or_else(|| self.err(format!("unknown type '{n}'")))
    }
    fn program(&mut self) -> Result<Vec<Top>, String> {
        let mut v = vec![];
        while !matches!(self.cur().kind, Tok::Eof) {
            if self.eat_kw("fn") {
                v.push(Top::Fun(self.function()?));
            } else {
                v.push(Top::Stmt(self.stmt()?));
            }
        }
        Ok(v)
    }
    fn function(&mut self) -> Result<Fun, String> {
        let name = self.expect_ident()?;
        self.expect_sym("(")?;
        let mut params = vec![];
        if !self.sym(")") {
            loop {
                let ty = self.ty()?;
                let name = self.expect_ident()?;
                let default = if self.eat_sym("=") {
                    Some(self.expr(1)?)
                } else {
                    None
                };
                if default.is_none() && params.iter().any(|p: &Param| p.default.is_some()) {
                    return Err(self.err(
                        "parameters with defaults must come after required parameters".into(),
                    ));
                }
                params.push(Param { ty, name, default });
                if !self.eat_sym(",") {
                    break;
                }
            }
        }
        self.expect_sym(")")?;
        let ret = if self.eat_sym("->") {
            self.ty()?
        } else {
            Ty::Void
        };
        let body = self.block()?;
        Ok(Fun {
            name,
            params,
            ret,
            body,
        })
    }
    fn block(&mut self) -> Result<Vec<Stmt>, String> {
        self.expect_sym("{")?;
        let mut v = vec![];
        while !self.sym("}") && !matches!(self.cur().kind, Tok::Eof) {
            v.push(self.stmt()?);
        }
        self.expect_sym("}")?;
        Ok(v)
    }
    fn stmt(&mut self) -> Result<Stmt, String> {
        if self.eat_kw("const") {
            let ty = self.ty()?;
            if ty == Ty::Void {
                return Err(self.err("const variables cannot have type 'void'".into()));
            }
            let name = self.expect_ident()?;
            self.expect_sym("=")?;
            let value = self.expr(0)?;
            self.expect_sym(";")?;
            return Ok(Stmt::ConstDecl(ty, name, value));
        }
        if self.eat_kw("for") {
            self.expect_sym("(")?;
            let variable = self.expect_ident()?;
            self.expect_sym(",")?;
            let first = self.expr(1)?;
            let (start, stop, step) = if self.eat_sym(",") {
                let second = self.expr(1)?;
                if self.eat_sym(",") {
                    (first, second, self.expr(1)?)
                } else {
                    (first, second, Expr::Int(1))
                }
            } else {
                (Expr::Int(0), first, Expr::Int(1))
            };
            self.expect_sym(")")?;
            return Ok(Stmt::For {
                variable,
                start,
                stop,
                step,
                body: self.block()?,
            });
        }
        if self.eat_kw("if") {
            self.expect_sym("(")?;
            let c = self.expr(0)?;
            self.expect_sym(")")?;
            let yes = self.block()?;
            let no = if self.eat_kw("else") {
                self.block()?
            } else {
                vec![]
            };
            return Ok(Stmt::If(c, yes, no));
        }
        if self.eat_kw("while") {
            self.expect_sym("(")?;
            let c = self.expr(0)?;
            self.expect_sym(")")?;
            return Ok(Stmt::While(c, self.block()?));
        }
        if self.eat_kw("return") {
            let e = if self.sym(";") {
                None
            } else {
                Some(self.expr(0)?)
            };
            self.expect_sym(";")?;
            return Ok(Stmt::Return(e));
        }
        if let Tok::Ident(ref s) = self.cur().kind {
            if let Some(ty) = Ty::parse(s) {
                if ty != Ty::Void {
                    self.bump();
                    let name = self.expect_ident()?;
                    self.expect_sym("=")?;
                    let e = self.expr(0)?;
                    self.expect_sym(";")?;
                    return Ok(Stmt::Decl(ty, name, e));
                }
            }
        }
        if let Tok::Ident(name) = self.cur().kind.clone() {
            if self.t.get(self.i + 1).map(|x| &x.kind) == Some(&Tok::Sym("=".into()))
                || ["+=", "-=", "*=", "/="].iter().any(|op| {
                    self.t.get(self.i + 1).map(|x| &x.kind) == Some(&Tok::Sym((*op).into()))
                })
            {
                self.bump();
                let op = match self.bump().kind {
                    Tok::Sym(s) => s,
                    _ => unreachable!(),
                };
                let e = self.expr(0)?;
                self.expect_sym(";")?;
                return Ok(Stmt::Assign(name, op, e));
            }
        }
        let e = self.expr(0)?;
        self.expect_sym(";")?;
        Ok(Stmt::Expr(e))
    }
    fn prec(op: &str) -> Option<u8> {
        Some(match op {
            "," => 0,
            "||" => 1,
            "&&" => 2,
            "==" | "!=" => 3,
            "<" | ">" | "<=" | ">=" => 4,
            "+" | "-" => 5,
            "*" | "/" | "%" => 6,
            _ => return None,
        })
    }
    fn expr(&mut self, min: u8) -> Result<Expr, String> {
        let mut lhs = if self.eat_sym("!") {
            Expr::Unary("!".into(), Box::new(self.expr(7)?))
        } else if self.eat_sym("-") {
            Expr::Unary("-".into(), Box::new(self.expr(7)?))
        } else {
            self.atom()?
        };
        loop {
            let op = match &self.cur().kind {
                Tok::Sym(s) => s.clone(),
                _ => break,
            };
            let p = match Self::prec(&op) {
                Some(p) if p >= min => p,
                _ => break,
            };
            self.bump();
            let rhs = self.expr(p + 1)?;
            lhs = Expr::Bin(Box::new(lhs), op, Box::new(rhs));
        }
        Ok(lhs)
    }
    fn atom(&mut self) -> Result<Expr, String> {
        let tok = self.bump();
        match tok.kind {
            Tok::Num(n) => {
                if n.contains('.') {
                    Ok(Expr::Float(n.parse().map_err(|_| {
                        format!("invalid number at byte {}", tok.pos)
                    })?))
                } else {
                    Ok(Expr::Int(n.parse().map_err(|_| {
                        format!("invalid integer at byte {}", tok.pos)
                    })?))
                }
            }
            Tok::Str(s) => Ok(Expr::Str(s)),
            Tok::Ident(s) if s == "true" => Ok(Expr::Bool(true)),
            Tok::Ident(s) if s == "false" => Ok(Expr::Bool(false)),
            Tok::Ident(s) => {
                if self.eat_sym("(") {
                    let mut a = vec![];
                    if !self.sym(")") {
                        loop {
                            a.push(self.expr(1)?);
                            if !self.eat_sym(",") {
                                break;
                            }
                        }
                    }
                    self.expect_sym(")")?;
                    Ok(Expr::Call(s, a))
                } else {
                    Ok(Expr::Var(s))
                }
            }
            Tok::Sym(s) if s == "(" => {
                let e = self.expr(0)?;
                self.expect_sym(")")?;
                Ok(e)
            }
            _ => Err(format!("expected expression at byte {}", tok.pos)),
        }
    }
}

pub(crate) fn parse(tokens: Vec<Token>) -> Result<Vec<Top>, String> {
    let mut parser = Parser { t: tokens, i: 0 };
    parser.program()
}
