use crate::compiler::ast::{Expr, Fun, Param, Stmt, StructDef, StructField, Top, Ty, VarDecl};
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
        if n == "list" {
            let element = self.ty()?;
            if element == Ty::Void {
                return Err(self.err("lists cannot contain values of type 'void'".into()));
            }
            return Ok(Ty::List(Box::new(element)));
        }
        Ok(Ty::parse(&n).unwrap_or_else(|| Ty::Struct(n)))
    }
    fn looks_like_declaration(&self) -> bool {
        fn skip_type(tokens: &[Token], index: usize) -> Option<usize> {
            let Tok::Ident(name) = &tokens.get(index)?.kind else {
                return None;
            };
            if name == "list" {
                skip_type(tokens, index + 1)
            } else {
                Some(index + 1)
            }
        }
        let Some(index) = skip_type(&self.t, self.i) else {
            return false;
        };
        matches!(self.t.get(index).map(|token| &token.kind), Some(Tok::Ident(_)))
            && self.t.get(index + 1).map(|token| &token.kind) == Some(&Tok::Sym("=".into()))
    }
    fn eat_modifier(&mut self, modifier: &str) -> bool {
        if self.eat_kw(modifier) {
            return true;
        }
        let parenthesized = self.sym("(")
            && self.t.get(self.i + 1).map(|token| &token.kind)
                == Some(&Tok::Ident(modifier.into()))
            && self.t.get(self.i + 2).map(|token| &token.kind) == Some(&Tok::Sym(")".into()));
        if parenthesized {
            self.bump();
            self.bump();
            self.bump();
        }
        parenthesized
    }
    fn declaration(&mut self, is_local: bool, is_const: bool) -> Result<Stmt, String> {
        let ty = self.ty()?;
        self.declaration_after_type(ty, is_local, is_const)
    }
    fn program(&mut self) -> Result<Vec<Top>, String> {
        let mut v = vec![];
        while !matches!(self.cur().kind, Tok::Eof) {
            if self.eat_kw("struct") {
                v.push(Top::Struct(self.struct_def()?));
            } else if self.eat_kw("fn") {
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
    fn struct_def(&mut self) -> Result<StructDef, String> {
        let name = self.expect_ident()?;
        self.expect_sym("{")?;
        let mut fields = Vec::new();
        while !self.sym("}") && !matches!(self.cur().kind, Tok::Eof) {
            let ty = self.ty()?;
            let field_name = self.expect_ident()?;
            self.expect_sym(";")?;
            fields.push(StructField {
                ty,
                name: field_name,
            });
        }
        self.expect_sym("}")?;
        Ok(StructDef { name, fields })
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
        let mut is_local = false;
        let mut is_const = false;
        loop {
            if self.eat_modifier("local") {
                if is_local {
                    return Err(self.err("duplicate 'local' modifier".into()));
                }
                is_local = true;
            } else if self.eat_modifier("const") {
                if is_const {
                    return Err(self.err("duplicate 'const' modifier".into()));
                }
                is_const = true;
            } else {
                break;
            }
        }
        if is_local || is_const {
            return self.declaration(is_local, is_const);
        }
        if self.sym("{") {
            return Ok(Stmt::Block(self.block()?));
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
        if self.looks_like_declaration() {
            let ty = self.ty()?;
            return self.declaration_after_type(ty, false, false);
        }
        let e = self.expr(0)?;
        if let Tok::Sym(op) = &self.cur().kind {
            if ["=", "+=", "-=", "*=", "/="].contains(&op.as_str()) {
                let op = op.clone();
                self.bump();
                let value = self.expr(0)?;
                self.expect_sym(";")?;
                return match e {
                    Expr::Var(name) => Ok(Stmt::Assign(name, op, value)),
                    Expr::Member(base, name) => {
                        Ok(Stmt::AssignMember(base, name, op, value))
                    }
                    Expr::Index(base, index) => {
                        Ok(Stmt::AssignIndex(base, index, op, value))
                    }
                    _ => Err(self.err("invalid assignment target".into())),
                };
            }
        }
        self.expect_sym(";")?;
        if let Expr::Call(name, args) = &e {
            if name == "require" {
                if let [Expr::Str(path)] = args.as_slice() {
                    return Ok(Stmt::Require(path.clone()));
                }
            }
        }
        Ok(Stmt::Expr(e))
    }
    fn declaration_after_type(
        &mut self,
        ty: Ty,
        is_local: bool,
        is_const: bool,
    ) -> Result<Stmt, String> {
        if ty == Ty::Void {
            return Err(self.err("variables cannot have type 'void'".into()));
        }
        let name = self.expect_ident()?;
        self.expect_sym("=")?;
        let value = if matches!(ty, Ty::Tuple(_)) && self.sym("(") {
            self.tuple_literal()?
        } else {
            self.expr(0)?
        };
        self.expect_sym(";")?;
        Ok(Stmt::Decl(VarDecl {
            ty,
            name,
            value,
            is_const,
            is_local,
        }))
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
            self.postfix()?
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
    fn call_args(&mut self) -> Result<Vec<Expr>, String> {
        self.expect_sym("(")?;
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
        Ok(a)
    }
    fn postfix(&mut self) -> Result<Expr, String> {
        let mut e = self.atom()?;
        loop {
            if self.eat_sym(".") {
                let name = self.expect_ident()?;
                if self.sym("(") {
                    let args = self.call_args()?;
                    e = Expr::MethodCall(Box::new(e), name, args);
                } else {
                    e = Expr::Member(Box::new(e), name);
                }
            } else if self.sym("(") {
                let args = self.call_args()?;
                match e {
                    Expr::Var(name) => e = Expr::Call(name, args),
                    _ => return Err(self.err("only named functions can be called".into())),
                }
            } else if self.eat_sym("[") {
                let index = self.expr(0)?;
                self.expect_sym("]")?;
                e = Expr::Index(Box::new(e), Box::new(index));
            } else if self.eat_kw("as") {
                let ty = self.ty()?;
                e = Expr::Cast(Box::new(e), ty);
            } else {
                break;
            }
        }
        Ok(e)
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
            Tok::Ident(s) => Ok(Expr::Var(s)),
            Tok::Sym(s) if s == "(" => {
                let e = self.expr(0)?;
                self.expect_sym(")")?;
                Ok(e)
            }
            Tok::Sym(s) if s == "{" => self.table_fields(),
            Tok::Sym(s) if s == "[" => self.list_values(),
            _ => Err(format!("expected expression at byte {}", tok.pos)),
        }
    }
    fn list_values(&mut self) -> Result<Expr, String> {
        let mut values = Vec::new();
        if !self.sym("]") {
            loop {
                if self.eat_sym("...") {
                    values.push(crate::compiler::ast::ListElement::Previous);
                } else {
                    values.push(crate::compiler::ast::ListElement::Value(self.expr(1)?));
                }
                if !self.eat_sym(",") {
                    break;
                }
                if self.sym("]") {
                    break;
                }
            }
        }
        self.expect_sym("]")?;
        Ok(Expr::List(values))
    }
    fn tuple_literal(&mut self) -> Result<Expr, String> {
        self.expect_sym("(")?;
        let mut values = Vec::new();
        if !self.sym(")") {
            loop {
                values.push(self.expr(1)?);
                if !self.eat_sym(",") {
                    break;
                }
                if self.sym(")") {
                    break;
                }
            }
        }
        self.expect_sym(")")?;
        Ok(Expr::Tuple(values))
    }
    fn table_fields(&mut self) -> Result<Expr, String> {
        let mut fields = Vec::new();
        if !self.sym("}") {
            loop {
                let named = matches!(self.cur().kind, Tok::Ident(_))
                    && self.t.get(self.i + 1).map(|token| &token.kind)
                        == Some(&Tok::Sym("=".into()));
                if named {
                    let name = self.expect_ident()?;
                    self.expect_sym("=")?;
                    fields.push((Some(name), self.expr(1)?));
                } else {
                    fields.push((None, self.expr(1)?));
                }
                if !self.eat_sym(",") {
                    break;
                }
                if self.sym("}") {
                    break;
                }
            }
        }
        self.expect_sym("}")?;
        Ok(Expr::Table(fields))
    }
}

pub(crate) fn parse(tokens: Vec<Token>) -> Result<Vec<Top>, String> {
    let mut parser = Parser { t: tokens, i: 0 };
    parser.program()
}
