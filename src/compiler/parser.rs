use crate::compiler::ast::{
    line_column, ClassDef, ClassField, Expr, Fun, Param, Stmt, StmtKind, StructDef, StructField,
    Top, Ty, VarDecl,
};
use crate::compiler::lexer::{Tok, Token};

struct Parser<'a> {
    t: Vec<Token>,
    i: usize,
    source: &'a str,
    errors: Vec<String>,
}
fn describe(kind: &Tok) -> String {
    match kind {
        Tok::Ident(name) => format!("'{name}'"),
        Tok::Num(value) => format!("number '{value}'"),
        Tok::Str(_) => "string".into(),
        Tok::Sym(symbol) => format!("'{symbol}'"),
        Tok::Eof => "end of file".into(),
    }
}
impl Parser<'_> {
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
            Err(self.err(format!(
                "expected '{s}', found {}",
                describe(&self.cur().kind)
            )))
        }
    }
    fn expect_ident(&mut self) -> Result<String, String> {
        if let Tok::Ident(name) = self.cur().kind.clone() {
            self.bump();
            Ok(name)
        } else {
            Err(self.err(format!(
                "expected identifier, found {}",
                describe(&self.cur().kind)
            )))
        }
    }
    fn err(&self, msg: String) -> String {
        self.err_at(self.cur().pos, msg)
    }
    fn err_at(&self, pos: usize, msg: String) -> String {
        let (line, column) = line_column(self.source, pos);
        format!("{msg} at line {line}, column {column}")
    }
    fn make_stmt(&self, pos: usize, kind: StmtKind) -> Stmt {
        let (line, column) = line_column(self.source, pos);
        Stmt { line, column, kind }
    }
    fn ty(&mut self) -> Result<Ty, String> {
        let token = self.bump();
        let pos = token.pos;
        let n = match token.kind {
            Tok::Ident(name) => name,
            other => {
                return Err(self.err_at(
                    pos,
                    format!("expected type name, found {}", describe(&other)),
                ))
            }
        };
        if n == "list" {
            let element = self.ty()?;
            if element == Ty::Void {
                return Err(self.err("lists cannot contain values of type 'void'".into()));
            }
            return Ok(Ty::List(Box::new(element)));
        }
        if n == "table" {
            let keyed = matches!(
                self.t.get(self.i).map(|token| &token.kind),
                Some(Tok::Ident(_))
            ) && matches!(
                self.t.get(self.i + 1).map(|token| &token.kind),
                Some(Tok::Ident(_))
            );
            if keyed {
                let key = self.ty()?;
                let value = self.ty()?;
                return Ok(Ty::Map(Box::new(key), Box::new(value)));
            }
        }
        Ok(Ty::parse(&n).unwrap_or_else(|| Ty::Struct(n)))
    }
    fn looks_like_declaration(&self) -> bool {
        fn skip_type(tokens: &[Token], index: usize) -> Option<usize> {
            let Tok::Ident(name) = &tokens.get(index)?.kind else {
                return None;
            };
            if name == "list" {
                return skip_type(tokens, index + 1);
            }
            if name == "table"
                && matches!(
                    tokens.get(index + 1).map(|token| &token.kind),
                    Some(Tok::Ident(_))
                )
                && matches!(
                    tokens.get(index + 2).map(|token| &token.kind),
                    Some(Tok::Ident(_))
                )
            {
                let value = skip_type(tokens, index + 1)?;
                return skip_type(tokens, value);
            }
            Some(index + 1)
        }
        let Some(index) = skip_type(&self.t, self.i) else {
            return false;
        };
        matches!(
            self.t.get(index).map(|token| &token.kind),
            Some(Tok::Ident(_))
        ) && self.t.get(index + 1).map(|token| &token.kind) == Some(&Tok::Sym("=".into()))
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
    fn declaration(&mut self, is_local: bool, is_const: bool, pos: usize) -> Result<Stmt, String> {
        let ty = self.ty()?;
        self.declaration_after_type(ty, is_local, is_const, pos)
    }
    fn program(&mut self) -> Vec<Top> {
        let mut v = vec![];
        while !matches!(self.cur().kind, Tok::Eof) {
            let pos = self.cur().pos;
            let item = if self.eat_kw("import") {
                (|| {
                    let mut packages = vec![self.expect_ident()?];
                    while self.eat_sym(",") {
                        packages.push(self.expect_ident()?);
                    }
                    self.expect_sym(";")?;
                    let (line, column) = line_column(self.source, pos);
                    Ok(Top::Import {
                        packages,
                        line,
                        column,
                    })
                })()
            } else if self.eat_kw("struct") {
                self.struct_def(pos).map(Top::Struct)
            } else if self.eat_kw("class") {
                self.class_top(pos)
            } else if self.eat_kw("fn") {
                self.function(pos).map(Top::Fun)
            } else {
                self.stmt().map(Top::Stmt)
            };
            match item {
                Ok(top) => v.push(top),
                Err(error) => {
                    self.errors.push(error);
                    self.recover(true);
                }
            }
        }
        v
    }
    fn function(&mut self, pos: usize) -> Result<Fun, String> {
        let name = self.expect_ident()?;
        let params = self.params()?;
        let ret = if self.eat_sym("->") {
            self.ty()?
        } else {
            Ty::Void
        };
        let body = self.block()?;
        let (line, column) = line_column(self.source, pos);
        Ok(Fun {
            name,
            line,
            column,
            params,
            ret,
            body,
        })
    }
    fn params(&mut self) -> Result<Vec<Param>, String> {
        self.expect_sym("(")?;
        let mut params = vec![];
        if !self.sym(")") {
            loop {
                let ty = self.ty()?;
                let name = self.expect_ident()?;
                if name == "this" || name == "self" || name == "super" {
                    return Err(self.err(format!(
                        "'{name}' is reserved and cannot be a parameter name"
                    )));
                }
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
        Ok(params)
    }
    fn class_top(&mut self, pos: usize) -> Result<Top, String> {
        let name = self.expect_ident()?;
        if self.eat_sym("=") {
            let stmt = self.class_instance_after_name(name, false, false, pos)?;
            return Ok(Top::Stmt(stmt));
        }
        let parent = if self.eat_kw("extends") {
            Some(self.expect_ident()?)
        } else {
            None
        };
        self.class_def_after_name(name, parent, pos).map(Top::Class)
    }
    fn class_instance_declaration(
        &mut self,
        is_local: bool,
        is_const: bool,
        pos: usize,
    ) -> Result<Stmt, String> {
        let name = self.expect_ident()?;
        if self.sym("{") {
            return Err(self.err("class definitions are only allowed at the top level".into()));
        }
        self.expect_sym("=")?;
        self.class_instance_after_name(name, is_local, is_const, pos)
    }
    fn class_instance_after_name(
        &mut self,
        name: String,
        is_local: bool,
        is_const: bool,
        pos: usize,
    ) -> Result<Stmt, String> {
        if name == "this" || name == "self" || name == "super" {
            return Err(self.err(format!(
                "'{name}' is reserved and cannot be a variable name"
            )));
        }
        let value = self.expr(0)?;
        self.expect_sym(";")?;
        let Expr::New(class_name, _) = &value else {
            return Err(
                self.err("a 'class' declaration must be initialized with new ClassName".into())
            );
        };
        Ok(self.make_stmt(
            pos,
            StmtKind::Decl(VarDecl {
                ty: Ty::Struct(class_name.clone()),
                name,
                value,
                is_const,
                is_local,
            }),
        ))
    }
    fn class_def_after_name(
        &mut self,
        name: String,
        parent: Option<String>,
        pos: usize,
    ) -> Result<ClassDef, String> {
        if parent.as_deref() == Some(&name) {
            return Err(self.err(format!("class '{name}' cannot extend itself")));
        }
        self.expect_sym("{")?;
        let mut fields = Vec::new();
        let mut constructor = None;
        let mut methods = Vec::new();
        while !self.sym("}") && !matches!(self.cur().kind, Tok::Eof) {
            let member_pos = self.cur().pos;
            if self.eat_kw("constructor") {
                if constructor.is_some() {
                    return Err(self.err("class can only define one constructor".into()));
                }
                let params = self.params()?;
                if self.sym("->") {
                    return Err(self.err("constructors cannot declare a return type".into()));
                }
                let body = self.block()?;
                let (line, column) = line_column(self.source, member_pos);
                constructor = Some(Fun {
                    name: "constructor".into(),
                    line,
                    column,
                    params,
                    ret: Ty::Void,
                    body,
                });
            } else if self.eat_kw("fn") {
                let method_name = self.expect_ident()?;
                if method_name == "constructor" {
                    if constructor.is_some() {
                        return Err(self.err("class can only define one constructor".into()));
                    }
                    let params = self.params()?;
                    if self.sym("->") {
                        return Err(self.err("constructors cannot declare a return type".into()));
                    }
                    let body = self.block()?;
                    let (line, column) = line_column(self.source, member_pos);
                    constructor = Some(Fun {
                        name: "constructor".into(),
                        line,
                        column,
                        params,
                        ret: Ty::Void,
                        body,
                    });
                    continue;
                }
                if method_name == "new" || method_name == "__init" {
                    return Err(self.err(format!(
                        "'{method_name}' is reserved for class construction"
                    )));
                }
                if method_name == "__index" {
                    return Err(self.err("'__index' is reserved for class method lookup".into()));
                }
                if method_name == "this" || method_name == "self" || method_name == "super" {
                    return Err(self.err(format!(
                        "'{method_name}' is reserved and cannot be a method name"
                    )));
                }
                let params = self.params()?;
                let ret = if self.eat_sym("->") {
                    self.ty()?
                } else {
                    Ty::Void
                };
                let body = self.block()?;
                let (line, column) = line_column(self.source, member_pos);
                methods.push(Fun {
                    name: method_name,
                    line,
                    column,
                    params,
                    ret,
                    body,
                });
            } else {
                let ty = self.ty()?;
                if ty == Ty::Void {
                    return Err(self.err("class fields cannot have type 'void'".into()));
                }
                let field_name = self.expect_ident()?;
                if field_name == "this" || field_name == "self" || field_name == "super" {
                    return Err(self.err(format!(
                        "'{field_name}' is reserved and cannot be a field name"
                    )));
                }
                if !self.eat_sym("=") {
                    return Err(self.err(format!(
                        "class field '{field_name}' requires an initializer"
                    )));
                }
                let default = self.expr(1)?;
                self.expect_sym(";")?;
                let (line, column) = line_column(self.source, member_pos);
                fields.push(ClassField {
                    ty,
                    name: field_name,
                    default,
                    line,
                    column,
                });
            }
        }
        self.expect_sym("}")?;
        let (line, column) = line_column(self.source, pos);
        Ok(ClassDef {
            name,
            parent,
            line,
            column,
            fields,
            constructor,
            methods,
        })
    }
    fn struct_def(&mut self, pos: usize) -> Result<StructDef, String> {
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
        let (line, column) = line_column(self.source, pos);
        Ok(StructDef {
            name,
            line,
            column,
            fields,
        })
    }
    fn block(&mut self) -> Result<Vec<Stmt>, String> {
        self.expect_sym("{")?;
        let mut v = vec![];
        while !self.sym("}") && !matches!(self.cur().kind, Tok::Eof) {
            match self.stmt() {
                Ok(statement) => v.push(statement),
                Err(error) => {
                    self.errors.push(error);
                    self.recover(false);
                }
            }
        }
        self.expect_sym("}")?;
        Ok(v)
    }
    fn recover(&mut self, top_level: bool) {
        while !matches!(self.cur().kind, Tok::Eof) {
            match &self.cur().kind {
                Tok::Sym(symbol) if symbol == ";" => {
                    self.bump();
                    return;
                }
                Tok::Sym(symbol) if symbol == "}" => {
                    if !top_level {
                        return;
                    }
                    self.bump();
                    return;
                }
                Tok::Ident(_) => return,
                Tok::Sym(symbol) if symbol == "{" => return,
                _ => {
                    self.bump();
                }
            }
        }
    }
    fn stmt(&mut self) -> Result<Stmt, String> {
        let pos = self.cur().pos;
        if self.eat_kw("import") {
            return Err(self.err("'import' is only allowed at the top level".into()));
        }
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
            if self.kw("class") {
                self.bump();
                return self.class_instance_declaration(is_local, is_const, pos);
            }
            if let Some(decl) = self.inferred_require_declaration(is_local, is_const, pos)? {
                return Ok(decl);
            }
            return self.declaration(is_local, is_const, pos);
        }
        if self.kw("class") {
            self.bump();
            return self.class_instance_declaration(false, false, pos);
        }
        if self.sym("{") {
            let body = self.block()?;
            return Ok(self.make_stmt(pos, StmtKind::Block(body)));
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
            let body = self.block()?;
            return Ok(self.make_stmt(
                pos,
                StmtKind::For {
                    variable,
                    start,
                    stop,
                    step,
                    body,
                },
            ));
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
            return Ok(self.make_stmt(pos, StmtKind::If(c, yes, no)));
        }
        if self.eat_kw("while") {
            self.expect_sym("(")?;
            let c = self.expr(0)?;
            self.expect_sym(")")?;
            let body = self.block()?;
            return Ok(self.make_stmt(pos, StmtKind::While(c, body)));
        }
        if self.eat_kw("return") {
            let e = if self.sym(";") {
                None
            } else {
                Some(self.expr(0)?)
            };
            self.expect_sym(";")?;
            return Ok(self.make_stmt(pos, StmtKind::Return(e)));
        }
        if self.looks_like_declaration() {
            let ty = self.ty()?;
            return self.declaration_after_type(ty, false, false, pos);
        }
        let e = self.expr(0)?;
        if let Tok::Sym(op) = &self.cur().kind {
            if ["=", "+=", "-=", "*=", "/="].contains(&op.as_str()) {
                let op = op.clone();
                self.bump();
                let value = self.expr(0)?;
                self.expect_sym(";")?;
                return match e {
                    Expr::Var(name) => Ok(self.make_stmt(pos, StmtKind::Assign(name, op, value))),
                    Expr::Member(base, name) => {
                        Ok(self.make_stmt(pos, StmtKind::AssignMember(base, name, op, value)))
                    }
                    Expr::Index(base, index) => {
                        Ok(self.make_stmt(pos, StmtKind::AssignIndex(base, index, op, value)))
                    }
                    _ => Err(self.err("invalid assignment target".into())),
                };
            }
        }
        self.expect_sym(";")?;
        Ok(self.make_stmt(pos, StmtKind::Expr(e)))
    }
    fn inferred_require_declaration(
        &mut self,
        is_local: bool,
        is_const: bool,
        pos: usize,
    ) -> Result<Option<Stmt>, String> {
        let is_ident = matches!(self.cur().kind, Tok::Ident(_));
        let followed_by_equals =
            self.t.get(self.i + 1).map(|token| &token.kind) == Some(&Tok::Sym("=".into()));
        if !is_ident || !followed_by_equals {
            return Ok(None);
        }
        let name = self.expect_ident()?;
        if name == "this" || name == "self" || name == "super" {
            return Err(self.err(format!(
                "'{name}' is reserved and cannot be a variable name"
            )));
        }
        self.expect_sym("=")?;
        let value = self.expr(0)?;
        self.expect_sym(";")?;
        let Expr::Call(call, args) = &value else {
            return Err(self.err(
                "a declaration without a type must be initialized from require(\"module\")".into(),
            ));
        };
        let [Expr::Str(path)] = args.as_slice() else {
            return Err(self.err(
                "a declaration without a type must be initialized from require(\"module\")".into(),
            ));
        };
        if call != "require" {
            return Err(self.err(
                "a declaration without a type must be initialized from require(\"module\")".into(),
            ));
        }
        Ok(Some(self.make_stmt(
            pos,
            StmtKind::Decl(VarDecl {
                ty: Ty::Module(path.clone()),
                name,
                value,
                is_const,
                is_local,
            }),
        )))
    }
    fn declaration_after_type(
        &mut self,
        ty: Ty,
        is_local: bool,
        is_const: bool,
        pos: usize,
    ) -> Result<Stmt, String> {
        if ty == Ty::Void {
            return Err(self.err("variables cannot have type 'void'".into()));
        }
        let name = self.expect_ident()?;
        if name == "this" || name == "self" || name == "super" {
            return Err(self.err(format!(
                "'{name}' is reserved and cannot be a variable name"
            )));
        }
        self.expect_sym("=")?;
        let value = if matches!(ty, Ty::Tuple(_) | Ty::Auto) && self.sym("(") {
            self.tuple_literal()?
        } else {
            self.expr(0)?
        };
        self.expect_sym(";")?;
        let ty = if ty == Ty::Module(String::new()) {
            match &value {
                Expr::Call(call, args) if call == "require" => match args.as_slice() {
                    [Expr::Str(path)] => Ty::Module(path.clone()),
                    _ => {
                        return Err(self.err(
                            "a 'module' declaration must be initialized from require(\"path\")"
                                .into(),
                        ))
                    }
                },
                _ => {
                    return Err(self.err(
                        "a 'module' declaration must be initialized from require(\"path\")".into(),
                    ))
                }
            }
        } else {
            ty
        };
        Ok(self.make_stmt(
            pos,
            StmtKind::Decl(VarDecl {
                ty,
                name,
                value,
                is_const,
                is_local,
            }),
        ))
    }
    fn prec(op: &str) -> Option<u8> {
        Some(match op {
            "," => 0,
            "||" => 1,
            "&&" => 2,
            "==" | "!=" => 3,
            "<" | ">" | "<=" | ">=" => 4,
            "+" | "-" | ".." => 5,
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
            if matches!(e, Expr::Var(ref name) if name == "require") {
                if let Tok::Str(path) = self.cur().kind.clone() {
                    self.bump();
                    e = Expr::Call("require".into(), vec![Expr::Str(path)]);
                    continue;
                }
            }
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
        match self.cur().kind.clone() {
            Tok::Num(n) => {
                self.bump();
                if n.contains('.') {
                    Ok(Expr::Float(n.parse().map_err(|_| {
                        self.err_at(self.t[self.i - 1].pos, "invalid number".into())
                    })?))
                } else {
                    Ok(Expr::Int(n.parse().map_err(|_| {
                        self.err_at(self.t[self.i - 1].pos, "invalid integer".into())
                    })?))
                }
            }
            Tok::Str(s) => {
                self.bump();
                Ok(Expr::Str(s))
            }
            Tok::Ident(s) if s == "true" => {
                self.bump();
                Ok(Expr::Bool(true))
            }
            Tok::Ident(s) if s == "false" => {
                self.bump();
                Ok(Expr::Bool(false))
            }
            Tok::Ident(s) if s == "new" => {
                self.bump();
                let name = self.expect_ident()?;
                let args = if self.sym("(") {
                    self.call_args()?
                } else {
                    Vec::new()
                };
                Ok(Expr::New(name, args))
            }
            Tok::Ident(s) if s == "super" => {
                self.bump();
                if self.eat_sym(".") {
                    let name = self.expect_ident()?;
                    let args = self.call_args()?;
                    Ok(Expr::SuperMethod(name, args))
                } else {
                    let args = self.call_args()?;
                    Ok(Expr::SuperCall(args))
                }
            }
            Tok::Ident(s) => {
                self.bump();
                Ok(Expr::Var(s))
            }
            Tok::Sym(s) if s == "(" => {
                self.bump();
                let e = self.expr(0)?;
                self.expect_sym(")")?;
                Ok(e)
            }
            Tok::Sym(s) if s == "{" => {
                self.bump();
                self.table_fields()
            }
            Tok::Sym(s) if s == "[" => {
                self.bump();
                self.list_values()
            }
            _ => Err(self.err(format!(
                "expected expression, found {}",
                describe(&self.cur().kind)
            ))),
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

pub(crate) fn parse(tokens: Vec<Token>, source: &str) -> Result<Vec<Top>, Vec<String>> {
    let mut parser = Parser {
        t: tokens,
        i: 0,
        source,
        errors: Vec::new(),
    };
    let program = parser.program();
    if parser.errors.is_empty() {
        Ok(program)
    } else {
        Err(parser.errors)
    }
}
