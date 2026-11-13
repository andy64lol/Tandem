use crate::compiler::ast::{
    line_column, ClassDef, ClassField, EnumDef, EnumVariant, Expr, Fun, FnType, InterfaceDef,
    MatchArm, MatchExpr, MixinDef, Param, Pattern, Stmt, StmtKind, StructDef, StructField, Top,
    TraitDef, TypeAliasDef, Ty, UnionDef, UnionVariant, VarDecl,
};
use std::collections::HashSet;

use crate::compiler::lexer::{Tok, Token};

struct Parser<'a> {
    t: Vec<Token>,
    i: usize,
    source: &'a str,
    errors: Vec<String>,
    generic_types: Vec<String>,
    pending: Vec<Top>,
    anonymous_classes: usize,
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
impl<'a> Parser<'a> {
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
    fn next_is_operator(&self) -> bool {
        matches!(
            self.t.get(self.i + 1).map(|token| &token.kind),
            Some(Tok::Sym(symbol))
                if matches!(
                    symbol.as_str(),
                    "+" | "-" | "*" | "/" | "%" | "==" | "!=" | "<" | ">" | "<=" | ">=" | ".."
                )
        )
    }
    fn next_is_sym(&self, s: &str) -> bool {
        self.t
            .get(self.i + 1)
            .is_some_and(|token| token.kind == Tok::Sym(s.into()))
    }
    fn next_is_kw(&self, s: &str) -> bool {
        self.t
            .get(self.i + 1)
            .is_some_and(|token| token.kind == Tok::Ident(s.into()))
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
            if name == "__tandem_init_state" {
                return Err(self.err_at(
                    self.t[self.i.saturating_sub(1)].pos,
                    "'__tandem_init_state' is reserved for compiler use".into(),
                ));
            }
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
        let mut ty = self.ty_base()?;
        while self.eat_sym("?") {
            ty = match ty {
                Ty::Optional(inner) => *inner,
                other => Ty::Optional(Box::new(other)),
            };
        }
        Ok(ty)
    }
    fn ty_base(&mut self) -> Result<Ty, String> {
    let token = self.bump();
    let pos = token.pos;
    if let Tok::Num(text) = &token.kind {
        if let Ok(value) = text.parse::<i64>() {
            return Ok(Ty::Const(value));
        }
    }
    let n = match token.kind {
        Tok::Ident(name) => name,
        other => {
            return Err(self.err_at(
                pos,
                format!("expected type name, found {}", describe(&other)),
            ))
        }
    };

    if self.generic_types.contains(&n) {
        return Ok(Ty::Generic(n.to_string()));
    }

    if self.sym(".") && matches!(self.t.get(self.i + 1).map(|t| &t.kind), Some(Tok::Ident(_))) {
        self.bump();
        let member = self.expect_ident()?;
        return Ok(Ty::Struct(member));
    }

    if n == "fn" {
        self.expect_sym("(")?;
        let mut args = Vec::new();
        if !self.sym(")") {
            loop {
                args.push(self.ty()?);
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
        return Ok(Ty::Function(Box::new(FnType {
            required: args.len(),
            args,
            ret,
        })));
    }

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
            if key == Ty::Void {
                return Err(self.err("table key type cannot be void".into()));
            }
            let value = self.ty()?;
            if value == Ty::Void {
                return Err(self.err("table values cannot have type 'void'".into()));
            }
            return Ok(Ty::Map(Box::new(key), Box::new(value)));
        }
    }

    if self.eat_sym("<") {
        let mut arguments = vec![self.ty()?];
        while self.eat_sym(",") {
            arguments.push(self.ty()?);
        }
        self.expect_sym(">")?;
        return Ok(Ty::Applied(n.to_string(), arguments));
    }

    Ok(Ty::parse(&n).unwrap_or_else(|| Ty::Struct(n.to_string())))
}

    fn looks_like_declaration(&self) -> bool {
    fn skip_type(tokens: &[Token], index: usize) -> Option<usize> {
        let Tok::Ident(name) = &tokens.get(index)?.kind else {
            return None;
        };
        if name == "list" {
            return skip_type(tokens, index + 1);
        }
        if name == "fn" {
            let mut cursor = index + 1;
            if tokens.get(cursor).map(|token| &token.kind) != Some(&Tok::Sym("(".into())) {
                return None;
            }
            cursor += 1;
            if tokens.get(cursor).map(|token| &token.kind) == Some(&Tok::Sym(")".into())) {
                cursor += 1;
            } else {
                loop {
                    cursor = skip_type(tokens, cursor)?;
                    match tokens.get(cursor).map(|token| &token.kind) {
                        Some(Tok::Sym(comma)) if comma == "," => cursor += 1,
                        Some(Tok::Sym(end)) if end == ")" => {
                            cursor += 1;
                            break;
                        }
                        _ => return None,
                    }
                }
            }
            if tokens.get(cursor).map(|token| &token.kind) == Some(&Tok::Sym("->".into())) {
                cursor = skip_type(tokens, cursor + 1)?;
            }
            return Some(cursor);
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
        if tokens.get(index + 1).map(|token| &token.kind) == Some(&Tok::Sym("<".into())) {
            let mut cursor = index + 2;
            loop {
                cursor = skip_type(tokens, cursor)?;
                match tokens.get(cursor).map(|token| &token.kind) {
                    Some(Tok::Sym(comma)) if comma == "," => cursor += 1,
                    Some(Tok::Sym(end)) if end == ">" => return Some(cursor + 1),
                    _ => return None,
                }
            }
        }
        let mut cursor = index + 1;
        while tokens.get(cursor).map(|token| &token.kind) == Some(&Tok::Sym("?".into())) {
            cursor += 1;
        }
        if tokens.get(cursor).map(|token| &token.kind) == Some(&Tok::Sym(".".into())) {
            cursor += 1;
            if matches!(tokens.get(cursor).map(|token| &token.kind), Some(Tok::Ident(_))) {
                cursor += 1;
            } else {
                return None;
            }
        }
        Some(cursor)
    }
    let Some(index) = skip_type(&self.t, self.i) else {
        return false;
    };
    // Single name: Type name =
    if matches!(self.t.get(index).map(|token| &token.kind), Some(Tok::Ident(_)))
        && self.t.get(index + 1).map(|token| &token.kind) == Some(&Tok::Sym("=".into()))
    {
        return true;
    }
    // Multiple names: Type name, name, ... =
    if matches!(self.t.get(index).map(|token| &token.kind), Some(Tok::Ident(_))) {
        let mut cursor = index + 1;
        while self.t.get(cursor).map(|token| &token.kind) == Some(&Tok::Sym(",".into())) {
            cursor += 1;
            if !matches!(self.t.get(cursor).map(|token| &token.kind), Some(Tok::Ident(_))) {
                return false;
            }
            cursor += 1;
        }
        if self.t.get(cursor).map(|token| &token.kind) == Some(&Tok::Sym("=".into())) {
            return true;
        }
    }
    false
}
    fn eat_modifier(&mut self, modifier: &str) -> bool {
    if self.eat_kw(modifier) {
        return true;
    }
    let parenthesized = self.sym("(")
        && self.t.get(self.i + 1).map(|token| &token.kind) == Some(&Tok::Ident(modifier.into()))
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
            } else if self.eat_kw("type") {
                self.type_alias(pos).map(Top::TypeAlias)
            } else if self.kw("abstract") && self.next_is_kw("class") {
                self.bump();
                let class_pos = self.cur().pos;
                self.bump();
                self.class_top_with(class_pos, true, false)
            } else if self.kw("final") && self.next_is_kw("class") {
                self.bump();
                let class_pos = self.cur().pos;
                self.bump();
                self.class_top_with(class_pos, false, true)
            } else if self.eat_kw("class") {
                self.class_top(pos)
            } else if self.eat_kw("interface") {
                self.interface_def(pos).map(Top::Interface)
            } else if self.eat_kw("trait") {
                self.trait_def(pos).map(Top::Trait)
            } else if self.eat_kw("enum") {
                self.enum_def(pos).map(Top::Enum)
            } else if self.eat_kw("union") {
                self.union_def(pos).map(Top::Union)
            } else if self.eat_kw("mixin") {
                self.mixin_def(pos).map(Top::Mixin)
            } else if self.kw("async") && self.next_is_kw("fn") {
                self.bump();
                self.bump();
                (|| {
                    let mut function = self.function(pos)?;
                    function.is_async = true;
                    Ok(Top::Fun(function))
                })()
            } else if self.eat_kw("test") {
                self.test_block(pos).map(Top::Test)
            } else if self.kw("fn") && !self.next_is_sym("(") {
                self.bump();
                self.function(pos).map(Top::Fun)
            } else {
                self.stmt().map(Top::Stmt)
            };
            match item {
                Ok(top) => {
                    v.push(top);
                    v.append(&mut self.pending);
                }
                Err(error) => {
                    self.errors.push(error);
                    self.recover(true);
                }
            }
        }
        v
    }
    fn substitute(&self, ty: Ty, generics: &[String]) -> Ty {
        match ty {
            Ty::Struct(name) if generics.iter().any(|generic| generic == &name) => Ty::Generic(name),
            Ty::List(element) => Ty::List(Box::new(self.substitute(*element, generics))),
            Ty::Map(key, value) => Ty::Map(
                Box::new(self.substitute(*key, generics)),
                Box::new(self.substitute(*value, generics)),
            ),
            Ty::Tuple(Some(elements)) => Ty::Tuple(Some(
                elements
                    .into_iter()
                    .map(|element| self.substitute(element, generics))
                    .collect(),
            )),
            Ty::Applied(name, arguments) => Ty::Applied(
                name,
                arguments
                    .into_iter()
                    .map(|argument| self.substitute(argument, generics))
                    .collect(),
            ),
            ty => ty,
        }
    }
    fn type_uses_generic(&self, ty: &Ty, generics: &[String]) -> bool {
        match ty {
            Ty::Struct(name) => generics.contains(name),
            Ty::List(element) => self.type_uses_generic(element, generics),
            Ty::Map(key, value) => {
                self.type_uses_generic(key, generics) || self.type_uses_generic(value, generics)
            }
            Ty::Tuple(Some(elements)) | Ty::Applied(_, elements) => elements
                .iter()
                .any(|element| self.type_uses_generic(element, generics)),
            Ty::Generic(name) => generics.contains(name),
            _ => false,
        }
    }
    fn generic_params(&mut self) -> Result<Vec<String>, String> {
        if !self.eat_sym("<") {
            return Ok(Vec::new());
        }
        let mut generics = Vec::new();
        loop {
            let is_const = self.eat_kw("const");
            let param = self.expect_ident()?;
            if is_const {
                self.expect_sym(":")?;
                let ty = self.ty()?;
                if ty != Ty::Int {
                    return Err(self.err("const generic parameters must have type 'int'".into()));
                }
            }
            let stored = if is_const {
                format!("const:{param}")
            } else {
                param.clone()
            };
            if generics.contains(&stored) {
                return Err(self.err(format!("duplicate generic parameter '{param}'")));
            }
            generics.push(stored);
            if !self.eat_sym(",") {
                break;
            }
        }
        self.expect_sym(">")?;
        Ok(generics)
    }
    fn function(&mut self, pos: usize) -> Result<Fun, String> {
        let name = self.expect_ident()?;
        let generic = self.generic_params()?;
        let old_generic_len = self.generic_types.len();
        self.generic_types.extend(generic.iter().cloned());
        let result = (|| {
            let params = self.params()?;
            let params = params
                .into_iter()
                .map(|param| Param {
                    ty: self.substitute(param.ty, &generic),
                    name: param.name,
                    default: param.default,
                })
                .collect();
            let ret = if self.eat_sym("->") {
                let ty = self.ty()?;
                self.substitute(ty, &generic)
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
                generic,
                is_static: false,
                is_private: false,
                is_protected: false,
                is_abstract: false,
                is_final: false,
                is_getter: false,
                is_setter: false,
                is_async: false,
                trait_default: None,
                body,
            })
        })();
        self.generic_types.truncate(old_generic_len);
        result
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
                    return Err(
                        self.err("parameters with defaults must come after required parameters".into())
                    );
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
    fn anonymous_class(&mut self, pos: usize) -> Result<ClassDef, String> {
        self.anonymous_classes += 1;
        let name = format!("__tandem_anon_class_{}", self.anonymous_classes);
        self.class_def_body(name, Vec::new(), Vec::new(), Vec::new(), pos, false, false)
    }
    fn class_top(&mut self, pos: usize) -> Result<Top, String> {
        self.class_top_with(pos, false, false)
    }
    fn class_top_with(
        &mut self,
        pos: usize,
        is_abstract: bool,
        is_final: bool,
    ) -> Result<Top, String> {
        let name = self.expect_ident()?;
        if self.eat_sym("=") {
            let stmt = self.class_instance_after_name(name, false, false, pos)?;
            return Ok(Top::Stmt(stmt));
        }
        let generic = self.generic_params()?;
        let mut parents = Vec::new();
        if self.eat_kw("extends") {
            parents.push(self.expect_ident()?);
            while self.eat_sym(",") {
                parents.push(self.expect_ident()?);
            }
        }
    let mut interfaces = Vec::new();
    if self.eat_kw("implements") {
        interfaces.push(self.expect_ident()?);
        while self.eat_sym(",") {
            interfaces.push(self.expect_ident()?);
        }
    }
    self.class_def_after_name(name, parents, interfaces, generic, pos, is_abstract, is_final)
        .map(Top::Class)
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
    if self.kw("class") {
        self.bump();
        let definition = self.anonymous_class(pos)?;
        let class_name = definition.name.clone();
        self.pending.push(Top::Class(definition));
        self.expect_sym(";")?;
        return Ok(self.make_stmt(
            pos,
            StmtKind::Decl(VarDecl {
                ty: Ty::Struct(class_name.clone()),
                name,
                value: Expr::New(class_name, Vec::new()),
                is_const,
                is_local,
            }),
        ));
    }
    let value = self.expr(0)?;
    self.expect_sym(";")?;
    let class_ty = match &value {
        Expr::New(class_name, _) => Ty::Struct(class_name.clone()),
        Expr::NewTyped(class_name, type_args, _) => {
            Ty::Applied(class_name.clone(), type_args.clone())
        }
        _ => {
            return Err(
                self.err("a 'class' declaration must be initialized with new ClassName".into())
            )
        }
    };
    Ok(self.make_stmt(
        pos,
        StmtKind::Decl(VarDecl {
            ty: class_ty,
            name,
            value,
            is_const,
            is_local,
        }),
    ))
}
fn class_def_after_name(    &mut self,
    name: String,
    parents: Vec<String>,
    interfaces: Vec<String>,
    generic: Vec<String>,
    pos: usize,
    is_abstract: bool,
    is_final: bool,
) -> Result<ClassDef, String> {
    let old_generic_len = self.generic_types.len();
    self.generic_types.extend(generic.iter().cloned());
    let result = self.class_def_body(
        name,
        parents,
        interfaces,
        generic,
        pos,
        is_abstract,
        is_final,
    );
    self.generic_types.truncate(old_generic_len);
    result
}
fn class_def_body(
    &mut self,
    name: String,
    parents: Vec<String>,
    interfaces: Vec<String>,
    generic: Vec<String>,
    pos: usize,
    is_abstract: bool,
    is_final: bool,
) -> Result<ClassDef, String> {
    if parents.iter().any(|parent| parent == &name) {
        return Err(self.err(format!("class '{name}' cannot extend itself")));
    }
    self.expect_sym("{")?;
    let mut fields = Vec::new();
    let mut constructor = None;
    let mut methods = Vec::new();
    let mut mixins = Vec::new();
    let mut friends = Vec::new();
    while !self.sym("}") && !matches!(self.cur().kind, Tok::Eof) {
        let member_pos = self.cur().pos;
        let (mut is_static, mut is_private) = (false, false);
        let mut is_protected = false;
        let mut member_abstract = false;
        let mut member_final = false;
        let mut member_async = false;
        loop {
            if self.eat_kw("static") {
                is_static = true;
            } else if self.eat_kw("private") {
                is_private = true;
            } else if self.eat_kw("protected") {
                is_protected = true;
            } else if self.eat_kw("abstract") {
                member_abstract = true;
            } else if self.eat_kw("final") {
                member_final = true;
            } else if self.eat_kw("async") {
                member_async = true;
            } else {
                break;
            }
        }
        if self.eat_kw("friend") {
            let friend = match self.cur().kind.clone() {
                Tok::Str(value) => { self.bump(); value }
                Tok::Ident(value) => { self.bump(); value }
                _ => return Err(self.err("expected a module name after 'friend'".into())),
            };
            self.expect_sym(";")?;
            friends.push(friend);
            continue;
        }
        if self.eat_kw("use") {
            if is_static || is_private || is_protected {
                return Err(self.err("'use' cannot be combined with modifiers".into()));
            }
            let mixin_name = self.expect_ident()?;
            self.expect_sym(";")?;
            if mixins.contains(&mixin_name) {
                return Err(self.err(format!("duplicate mixin '{mixin_name}'")));
            }
            mixins.push(mixin_name);
            continue;
        }
        if self.eat_kw("get") || self.eat_kw("set") {
            let is_getter = self.t[self.i - 1].kind == Tok::Ident("get".into());
            let property_name = self.expect_ident()?;
            let params = self.params()?;
            if !is_getter && params.len() != 1 {
                return Err(self.err(format!(
                    "setter '{property_name}' must take exactly one parameter"
                )));
            }
            if is_getter && !params.is_empty() {
                return Err(self.err(format!(
                    "getter '{property_name}' cannot take parameters"
                )));
            }
            let ret = if is_getter {
                if self.eat_sym("->") {
                    let ty = self.ty()?;
                    self.substitute(ty, &generic)
                } else {
                    return Err(self.err(format!("getter '{property_name}' needs a return type")));
                }
            } else {
                Ty::Void
            };
            let body = self.block()?;
            let (line, column) = line_column(self.source, member_pos);
            methods.push(Fun {
                name: property_name,
                line,
                column,
                params,
                ret,
                generic: Vec::new(),
                is_static,
                is_private,
                is_protected,
                is_abstract: false,
                is_final: member_final,
                is_getter,
                is_setter: !is_getter,
                is_async: false,
                trait_default: None,
                body,
            });
            continue;
        }
        if self.eat_kw("constructor") {
            if is_static || is_private {
                return Err(self.err("constructors cannot be static or private".into()));
            }
            if constructor.is_some() {
                return Err(self.err("class can only define one constructor".into()));
            }
            let params = self.params()?;
            let params = params
                .into_iter()
                .map(|param| Param {
                    ty: self.substitute(param.ty, &generic),
                    name: param.name,
                    default: param.default,
                })
                .collect();
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
                generic: Vec::new(),
                is_static,
                is_private,
                is_protected,
                is_abstract: false,
                is_final: false,
                is_getter: false,
                is_setter: false,
                is_async: false,
                trait_default: None,
                body,
            });
        } else if self.eat_kw("fn") {
            let method_name = if self.sym("(") {
                self.bump();
                let name_expr = self.expr(0)?;
                self.expect_sym(")")?;
                match constant_string(&name_expr) {
                    Some(name) => name,
                    None => {
                        return Err(
                            self.err("computed method names must be constant strings".into())
                        )
                    }
                }
            } else if self.kw("operator") && self.next_is_operator() {
                self.bump();
                let operator = match self.bump().kind {
                    Tok::Sym(symbol) => symbol,
                    _ => unreachable!("operator names are followed by a symbol"),
                };
                format!("operator{operator}")
            } else {
                self.expect_ident()?
            };
            let method_generic = self.generic_params()?;
            let mut all_generic = if is_static {
                Vec::new()
            } else {
                generic.clone()
            };
            for parameter in &method_generic {
                if all_generic.contains(parameter) {
                    return Err(self.err(format!("duplicate generic parameter '{parameter}'")));
                }
                all_generic.push(parameter.clone());
            }
            if method_name == "constructor" {
                if is_static || is_private || !method_generic.is_empty() {
                    return Err(self.err("constructors cannot be modified or generic".into()));
                }
                if constructor.is_some() {
                    return Err(self.err("class can only define one constructor".into()));
                }
                let params = self.params()?;
                let params = params
                    .into_iter()
                    .map(|param| Param {
                        ty: self.substitute(param.ty, &generic),
                        name: param.name,
                        default: param.default,
                    })
                    .collect();
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
                    generic: Vec::new(),
                    is_static,
                    is_private,
                    is_protected,
                    is_abstract: false,
                    is_final: false,
                    is_getter: false,
                    is_setter: false,
                    is_async: false,
                    trait_default: None,
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
            let old_generic_len = self.generic_types.len();
            self.generic_types.extend(method_generic.iter().cloned());
            let params = self.params()?;
            let params = params
                .into_iter()
                .map(|param| Param {
                    ty: self.substitute(param.ty, &all_generic),
                    name: param.name,
                    default: param.default,
                })
                .collect();
            let ret = if self.eat_sym("->") {
                let ty = self.ty()?;
                self.substitute(ty, &all_generic)
            } else {
                Ty::Void
            };
            let body = if member_abstract {
                self.eat_sym(";");
                Vec::new()
            } else if self.sym(";") {
                self.bump();
                member_abstract = true;
                Vec::new()
            } else {
                self.block()?
            };
            self.generic_types.truncate(old_generic_len);
            let (line, column) = line_column(self.source, member_pos);
            methods.push(Fun {
                name: method_name,
                line,
                column,
                params,
                ret,
                generic: all_generic,
                is_static,
                is_private,
                is_protected,
                is_abstract: member_abstract,
                is_final: member_final,
                is_getter: false,
                is_setter: false,
                is_async: member_async,
                trait_default: None,
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
            let default = if self.eat_sym("=") {
                Some(self.expr(1)?)
            } else if !is_static && self.type_uses_generic(&ty, &generic) {
                None
            } else {
                return Err(self.err(format!(
                    "class field '{field_name}' requires an initializer"
                )));
            };
            self.expect_sym(";")?;
            let (line, column) = line_column(self.source, member_pos);
            fields.push(ClassField {
                ty: self.substitute(ty, &generic),
                name: field_name,
                default,
                line,
                column,
                is_static,
                is_private,
                is_protected,
            });
        }
    }
    self.expect_sym("}")?;
    let (line, column) = line_column(self.source, pos);
    Ok(ClassDef {
        name,
        parents,
        interfaces,
        mixins,
        generic,
        line,
        column,
        fields,
        constructor,
        methods,
        friends,
        is_abstract,
        is_final,
    })
}
fn interface_def(&mut self, pos: usize) -> Result<InterfaceDef, String> {
    let name = self.expect_ident()?;
    let mut parents = Vec::new();
    if self.eat_kw("extends") {
        parents.push(self.expect_ident()?);
        while self.eat_sym(",") {
            parents.push(self.expect_ident()?);
        }
    }
    self.expect_sym("{")?;
    let mut methods = Vec::new();
    while !self.sym("}") && !matches!(self.cur().kind, Tok::Eof) {
        let member_pos = self.cur().pos;
        if !self.eat_kw("fn") {
            return Err(self.err("interfaces only declare methods with 'fn'".into()));
        }
        let method_name = self.expect_ident()?;
        let params = self.params()?;
        let ret = if self.eat_sym("->") {
            self.ty()?
        } else {
            Ty::Void
        };
        self.eat_sym(";");
        let (line, column) = line_column(self.source, member_pos);
        methods.push(Fun {
            name: method_name,
            line,
            column,
            params,
            ret,
            generic: Vec::new(),
            is_static: false,
            is_private: false,
            is_protected: false,
            is_abstract: false,
            is_final: false,
            is_getter: false,
            is_setter: false,
            is_async: false,
            trait_default: None,
            body: Vec::new(),
        });
    }
    self.expect_sym("}")?;
    let (line, column) = line_column(self.source, pos);
    Ok(InterfaceDef {
        name,
        parents,
        methods,
        line,
        column,
    })
}
fn enum_def(&mut self, pos: usize) -> Result<EnumDef, String> {
    let name = self.expect_ident()?;
    self.expect_sym("{")?;
    let mut variants: Vec<EnumVariant> = Vec::new();
    let mut next_value: i64 = 0;
    while !self.sym("}") && !matches!(self.cur().kind, Tok::Eof) {
        let member_pos = self.cur().pos;
        let variant_name = self.expect_ident()?;
        let value = if self.eat_sym("=") {
            let literal = self.expr(1)?;
            let Expr::Int(value) = literal else {
                return Err(self.err("enum variant values must be integer literals".into()));
            };
            value
        } else {
            next_value
        };
        next_value = value + 1;
        if variants
            .iter()
            .any(|variant| variant.name == variant_name)
        {
            return Err(self.err(format!("duplicate enum variant '{variant_name}'")));
        }
        self.eat_sym(",");
        let (line, column) = line_column(self.source, member_pos);
        variants.push(EnumVariant {
            name: variant_name,
            value,
            line,
            column,
        });
    }
    self.expect_sym("}")?;
    let (line, column) = line_column(self.source, pos);
    Ok(EnumDef {
        name,
        line,
        column,
        variants,
    })
}

fn union_def(&mut self, pos: usize) -> Result<UnionDef, String> {
    let name = self.expect_ident()?;
    self.expect_sym("{")?;
    let mut variants: Vec<UnionVariant> = Vec::new();
    while !self.sym("}") && !matches!(self.cur().kind, Tok::Eof) {
        let member_pos = self.cur().pos;
        let variant_name = self.expect_ident()?;
        let payload = if self.eat_sym("(") {
            let ty = self.ty()?;
            if matches!(self.cur().kind, Tok::Ident(_)) {
                self.bump();
            }
            self.expect_sym(")")?;
            Some(ty)
        } else {
            None
        };
        if variants.iter().any(|variant| variant.name == variant_name) {
            return Err(self.err(format!("duplicate union variant '{variant_name}'")));
        }
        self.eat_sym(",");
        let (line, column) = line_column(self.source, member_pos);
        variants.push(UnionVariant {
            name: variant_name,
            payload,
            line,
            column,
        });
    }
    self.expect_sym("}")?;
    let (line, column) = line_column(self.source, pos);
    Ok(UnionDef {
        name,
        line,
        column,
        variants,
    })
}

fn mixin_def(&mut self, pos: usize) -> Result<MixinDef, String> {
    let name = self.expect_ident()?;
    self.expect_sym("{")?;
    let mut fields = Vec::new();
    let mut methods = Vec::new();
    while !self.sym("}") && !matches!(self.cur().kind, Tok::Eof) {
        let member_pos = self.cur().pos;
        if self.eat_kw("fn") {
            let method_name = self.expect_ident()?;
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
                generic: Vec::new(),
                is_static: false,
                is_private: false,
                is_protected: false,
                is_abstract: false,
                is_final: false,
                is_getter: false,
                is_setter: false,
                is_async: false,
                trait_default: None,
                body,
            });
            continue;
        }
        let ty = self.ty()?;
        let field_name = self.expect_ident()?;
        let default = if self.eat_sym("=") {
            Some(self.expr(1)?)
        } else {
            return Err(self.err(format!(
                "mixin field '{field_name}' requires an initializer"
            )));
        };
        self.expect_sym(";")?;
        let (line, column) = line_column(self.source, member_pos);
        fields.push(ClassField {
            ty,
            name: field_name,
            default,
            line,
            column,
            is_static: false,
            is_private: false,
            is_protected: false,
        });
    }
    self.expect_sym("}")?;
    let (line, column) = line_column(self.source, pos);
    Ok(MixinDef {
        name,
        line,
        column,
        fields,
        methods,
    })
}

fn trait_def(&mut self, pos: usize) -> Result<TraitDef, String> {
    let name = self.expect_ident()?;
    let mut parents = Vec::new();
    if self.eat_kw("extends") {
        parents.push(self.expect_ident()?);
        while self.eat_sym(",") {
            parents.push(self.expect_ident()?);
        }
    }
    self.expect_sym("{")?;
    let mut methods = Vec::new();
    while !self.sym("}") && !matches!(self.cur().kind, Tok::Eof) {
        let member_pos = self.cur().pos;
        if !self.eat_kw("fn") {
            return Err(self.err("traits only declare methods with 'fn'".into()));
        }
        let method_name = self.expect_ident()?;
        let params = self.params()?;
        let ret = if self.eat_sym("->") {
            self.ty()?
        } else {
            Ty::Void
        };
        let body = if self.sym("{") {
            self.block()?
        } else {
            self.eat_sym(";");
            Vec::new()
        };
        let (line, column) = line_column(self.source, member_pos);
        methods.push(Fun {
            name: method_name,
            line,
            column,
            params,
            ret,
            generic: Vec::new(),
            is_static: false,
            is_private: false,
            is_protected: false,
            is_abstract: false,
            is_final: false,
            is_getter: false,
            is_setter: false,
            is_async: false,
            trait_default: None,
            body,
        });
    }
    self.expect_sym("}")?;
    let (line, column) = line_column(self.source, pos);
    Ok(TraitDef {
        name,
        parents,
        line,
        column,
        methods,
    })
}

fn test_block(&mut self, pos: usize) -> Result<crate::compiler::ast::TestBlock, String> {
    let name = match self.cur().kind.clone() {
        Tok::Str(value) => { self.bump(); value }
        Tok::Ident(value) => { self.bump(); value }
        _ => return Err(self.err("expected a test name after 'test'".into())),
    };
    let body = self.block()?;
    let (line, column) = line_column(self.source, pos);
    Ok(crate::compiler::ast::TestBlock { name, line, column, body })
}

fn type_alias(&mut self, pos: usize) -> Result<TypeAliasDef, String> {
    let name = self.expect_ident()?;
    self.expect_sym("=")?;
    let ty = self.ty()?;
    self.expect_sym(";")?;
    let (line, column) = line_column(self.source, pos);
    Ok(TypeAliasDef { name, ty, line, column })
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
    if self.eat_kw("defer") {
        let inner = self.stmt()?;
        return Ok(self.make_stmt(pos, StmtKind::Defer(Box::new(inner))));
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
    let mut names = vec![self.expect_ident()?];
    if names[0] == "this" || names[0] == "self" || names[0] == "super" {
        return Err(self.err(format!(
            "'{}' is reserved and cannot be a variable name",
            names[0]
        )));
    }
    while self.eat_sym(",") {
        let next = self.expect_ident()?;
        if next == "this" || next == "self" || next == "super" {
            return Err(self.err(format!(
                "'{next}' is reserved and cannot be a variable name"
            )));
        }
        names.push(next);
    }
    self.expect_sym("=")?;
    let value = if self.sym("(") {
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
                        "a 'module' declaration must be initialized from require(\"path\")".into(),
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
    if names.len() == 1 {
        Ok(self.make_stmt(
            pos,
            StmtKind::Decl(VarDecl {
                ty,
                name: names.into_iter().next().unwrap(),
                value,
                is_const,
                is_local,
            }),
        ))
    } else {
        // Destructuring declaration: int a, b = (1, 2);
        let mut decls = Vec::new();
        for (idx, name) in names.iter().enumerate() {
            let index_expr = Expr::Index(Box::new(value.clone()), Box::new(Expr::Int((idx + 1) as i64)));
            decls.push(VarDecl {
                ty: ty.clone(),
                name: name.clone(),
                value: index_expr,
                is_const,
                is_local,
            });
        }
        Ok(self.make_stmt(pos, StmtKind::MultiDecl(decls)))
    }
}
fn prec(op: &str) -> Option<u8> {
    Some(match op {
        "," => 0,
        "??" => 1,
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
    let mut lhs = if self.kw("await") {
        self.bump();
        Expr::Await(Box::new(self.expr(7)?))
    } else if self.eat_sym("!") {
        Expr::Unary("!".into(), Box::new(self.expr(7)?))
    } else if self.eat_sym("-") {
        Expr::Unary("-".into(), Box::new(self.expr(7)?))
    } else if self.eat_sym("&") {
        let target = self.postfix()?;
        match target {
            Expr::Var(name) => Expr::FuncRef(name),
            other => Expr::MethodRef(Box::new(other)),
        }
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
        lhs = match op.as_str() {
            "??" => Expr::Coalesce(Box::new(lhs), Box::new(rhs)),
            _ => Expr::Bin(Box::new(lhs), op, Box::new(rhs)),
        };
    }
    Ok(lhs)
}
fn call_args(&mut self) -> Result<Vec<Expr>, String> {
    self.expect_sym("(")?;
    let mut a = vec![];
    if !self.sym(")") {
        loop {
            if self.eat_sym("...") {
                a.push(Expr::Spread(Box::new(self.expr(1)?)));
            } else {
                a.push(self.expr(1)?);
            }
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
                other => e = Expr::CallValue(Box::new(other), args),
            }
        } else if self.sym("!") {
            self.bump();
            e = Expr::Unwrap(Box::new(e));
        } else if self.sym("?") && self.next_is_sym(".") {
            self.bump();
            self.bump();
            let name = self.expect_ident()?;
            e = Expr::SafeMember(Box::new(e), name);
        } else if self.kw("instanceof") {
            self.bump();
            let class_name = self.expect_ident()?;
            e = Expr::InstanceOf(Box::new(e), class_name);
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
            if !s.contains("${") {
                return Ok(Expr::Str(s));
            }
            self.parse_interpolated_string(&s)
        }
        Tok::Ident(s) if s == "true" => {
            self.bump();
            Ok(Expr::Bool(true))
        }
        Tok::Ident(s) if s == "false" => {
            self.bump();
            Ok(Expr::Bool(false))
        }
        Tok::Ident(s) if s == "null" => {
            self.bump();
            Ok(Expr::Null)
        }
        Tok::Ident(s) if s == "comptime" => {
            self.bump();
            self.expect_sym("(")?;
            let value = self.expr(0)?;
            self.expect_sym(")")?;
            Ok(Expr::Comptime(Box::new(value)))
        }
        Tok::Ident(s) if s == "match" => {
            self.bump();
            self.match_expr()
        }
        Tok::Ident(s) if s == "fn" => {
            let pos = self.cur().pos;
            self.bump();
            let params = self.params()?;
            let ret = if self.eat_sym("->") {
                self.ty()?
            } else {
                Ty::Void
            };
            let body = self.block()?;
            let (line, column) = line_column(self.source, pos);
            Ok(Expr::Lambda(Box::new(Fun {
                name: String::new(),
                line,
                column,
                params,
                ret,
                generic: Vec::new(),
                is_static: false,
                is_private: false,
                is_protected: false,
                is_abstract: false,
                is_final: false,
                is_getter: false,
                is_setter: false,
                is_async: false,
                trait_default: None,
                body,
            })))
        }
        Tok::Ident(s) if s == "new" => {
            self.bump();
            let mut name = self.expect_ident()?;
            if self.eat_sym(".") {
                let member = self.expect_ident()?;
                name = format!("{name}.{member}");
            }
            let type_args = if self.eat_sym("<") {
                let mut arguments = vec![self.ty()?];
                while self.eat_sym(",") {
                    arguments.push(self.ty()?);
                }
                self.expect_sym(">")?;
                Some(arguments)
            } else {
                None
            };
            let args = if self.sym("(") {
                self.call_args()?
            } else {
                Vec::new()
            };
            Ok(match type_args {
                Some(type_args) => Expr::NewTyped(name, type_args, args),
                None => Expr::New(name, args),
            })
        }
        Tok::Ident(s) if s == "super" => {
            self.bump();
            if self.eat_sym(".") {
                let name = self.expect_ident()?;
                if self.sym("(") {
                    let args = self.call_args()?;
                    Ok(Expr::SuperMethod(name, args))
                } else {
                    Ok(Expr::SuperMember(name))
                }
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
    fn match_expr(&mut self) -> Result<Expr, String> {
        let subject = self.expr(1)?;
        self.expect_sym("{")?;
        let mut arms: Vec<MatchArm> = Vec::new();
        while !self.sym("}") && !matches!(self.cur().kind, Tok::Eof) {
            let pattern = self.pattern()?;
            self.expect_sym("=>")?;
            let body = self.expr(1)?;
            arms.push(MatchArm { pattern, body });
            self.eat_sym(",");
        }
        self.expect_sym("}")?;
        if arms.is_empty() {
            return Err(self.err("a 'match' expression needs at least one arm".into()));
        }
        Ok(Expr::Match(Box::new(MatchExpr { subject, arms })))
    }
    fn pattern(&mut self) -> Result<Pattern, String> {
        if self.sym("(") {
            self.bump();
            let mut patterns = Vec::new();
            if !self.sym(")") {
                loop {
                    patterns.push(self.pattern()?);
                    if !self.eat_sym(",") {
                        break;
                    }
                }
            }
            self.expect_sym(")")?;
            return Ok(Pattern::Tuple(patterns));
        }
        if let Tok::Ident(name) = self.cur().kind.clone() {
            if name == "_" {
                self.bump();
                return Ok(Pattern::Wildcard);
            }
            if self.t.get(self.i + 1).map(|token| &token.kind) == Some(&Tok::Sym(".".into())) {
                self.bump();
                self.bump();
                let variant = self.expect_ident()?;
                let payload = if self.sym("(") {
                    self.bump();
                    let inner = self.pattern()?;
                    self.expect_sym(")")?;
                    Some(Box::new(inner))
                } else {
                    None
                };
                return Ok(Pattern::Variant(name, variant, payload));
            }
            self.bump();
            return Ok(Pattern::Binding(name));
        }
        let expr = self.expr(1)?;
        Ok(Pattern::Literal(expr))
    }
    fn parse_interpolated_string(&mut self, s: &str) -> Result<Expr, String> {
        let chars: Vec<char> = s.chars().collect();
        let mut parts: Vec<Expr> = Vec::new();
        let mut current = String::new();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '$' && i + 1 < chars.len() && chars[i + 1] == '{' {
                // find matching }
                let mut depth = 0;
                let start = i + 2;
                let mut j = start;
                while j < chars.len() {
                    if chars[j] == '{' {
                        depth += 1;
                    } else if chars[j] == '}' {
                        if depth == 0 {
                            break;
                        }
                        depth -= 1;
                    }
                    j += 1;
                }
                if j >= chars.len() {
                    return Err(self.err("unclosed '${' in string".into()));
                }
                let expr_str: String = chars[start..j].iter().collect();
                if !current.is_empty() {
                    parts.push(Expr::Str(current.clone()));
                    current.clear();
                }
                // Parse the expression using a sub-parser.
                let tokens = crate::compiler::lexer::lex(&expr_str)
                    .map_err(|e| self.err(format!("in string interpolation: {e}")))?;
                let mut sub = Parser {
                    t: tokens,
                    i: 0,
                    source: &expr_str,
                    errors: Vec::new(),
                    generic_types: Vec::new(),
                    pending: Vec::new(),
                    anonymous_classes: 0,
                };
                let expr = sub.expr(0)?;
                if !sub.errors.is_empty() {
                    return Err(self.err(sub.errors.join("; ")));
                }
                parts.push(expr);
                i = j + 1;
                continue;
            } else {
                current.push(chars[i]);
                i += 1;
            }
        }
        if !current.is_empty() {
            parts.push(Expr::Str(current));
        }
        if parts.is_empty() {
            return Ok(Expr::Str(String::new()));
        }
        let mut result = parts.remove(0);
        for part in parts {
            let part = match &part {
                Expr::Str(_) => part,
                _ => Expr::Call("tostring".to_string(), vec![part]),
            };
            result = Expr::Bin(Box::new(result), "..".to_string(), Box::new(part));
        }
        Ok(result)
    }

fn list_values(&mut self) -> Result<Expr, String> {    let mut values = Vec::new();
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
                && self.t.get(self.i + 1).map(|token| &token.kind) == Some(&Tok::Sym("=".into()));
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

/// Evaluates a constant string expression used as a computed name.
fn constant_string(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Str(value) => Some(value.clone()),
        Expr::Bin(left, op, right) if op == "+" || op == ".." => {
            Some(format!(
                "{}{}",
                constant_string(left)?,
                constant_string(right)?
            ))
        }
        _ => None,
    }
}

/// Rewrites `Type` names that refer to enums or unions into their nominal types.
fn resolve_named_types(program: &mut [Top]) {
    let mut enums: HashSet<String> = HashSet::new();
    let mut unions: HashSet<String> = HashSet::new();
    for top in program.iter() {
        match top {
            Top::Enum(definition) => {
                enums.insert(definition.name.clone());
            }
            Top::Union(definition) => {
                unions.insert(definition.name.clone());
            }
            _ => {}
        }
    }
    if enums.is_empty() && unions.is_empty() {
        return;
    }

    fn resolve(ty: &mut Ty, enums: &HashSet<String>, unions: &HashSet<String>) {
        match ty {
            Ty::Struct(name) => {
                if enums.contains(name) {
                    *ty = Ty::Enum(name.clone());
                } else if unions.contains(name) {
                    *ty = Ty::Union(name.clone());
                }
            }
            Ty::List(element) => resolve(element, enums, unions),
            Ty::Map(key, value) => {
                resolve(key, enums, unions);
                resolve(value, enums, unions);
            }
            Ty::Tuple(Some(elements)) => {
                for element in elements {
                    resolve(element, enums, unions);
                }
            }
            Ty::Applied(_, arguments) => {
                for argument in arguments {
                    resolve(argument, enums, unions);
                }
            }
            Ty::Optional(inner) => resolve(inner, enums, unions),
            _ => {}
        }
    }

    fn resolve_callable(
        callable: &mut Fun,
        enums: &HashSet<String>,
        unions: &HashSet<String>,
    ) {
        for param in &mut callable.params {
            resolve(&mut param.ty, enums, unions);
        }
        resolve(&mut callable.ret, enums, unions);
    }

    fn resolve_expr(expr: &mut Expr, enums: &HashSet<String>, unions: &HashSet<String>) {
        match expr {
            Expr::Cast(value, ty) => {
                resolve(ty, enums, unions);
                resolve_expr(value, enums, unions);
            }
            Expr::Unary(_, value) => resolve_expr(value, enums, unions),
            Expr::Bin(left, _, right) => {
                resolve_expr(left, enums, unions);
                resolve_expr(right, enums, unions);
            }
            Expr::Call(_, args) => {
                for arg in args {
                    resolve_expr(arg, enums, unions);
                }
            }
            Expr::CallValue(callee, args) => {
                resolve_expr(callee, enums, unions);
                for arg in args {
                    resolve_expr(arg, enums, unions);
                }
            }
            Expr::New(_, args) | Expr::NewTyped(_, _, args) => {
                for arg in args {
                    resolve_expr(arg, enums, unions);
                }
            }
            Expr::SuperCall(args) | Expr::SuperMethod(_, args) => {
                for arg in args {
                    resolve_expr(arg, enums, unions);
                }
            }
            Expr::Member(base, _) | Expr::SafeMember(base, _) => {
                resolve_expr(base, enums, unions)
            }
            Expr::MethodCall(base, _, args) => {
                resolve_expr(base, enums, unions);
                for arg in args {
                    resolve_expr(arg, enums, unions);
                }
            }
            Expr::Index(base, index) => {
                resolve_expr(base, enums, unions);
                resolve_expr(index, enums, unions);
            }
            Expr::Table(fields) => {
                for (_, value) in fields {
                    resolve_expr(value, enums, unions);
                }
            }
            Expr::List(values) => {
                for value in values {
                    if let crate::compiler::ast::ListElement::Value(value) = value {
                        resolve_expr(value, enums, unions);
                    }
                }
            }
            Expr::Tuple(values) => {
                for value in values {
                    resolve_expr(value, enums, unions);
                }
            }
            Expr::Lambda(function) => {
                for statement in &mut function.body {
                    resolve_stmt(statement, enums, unions);
                }
            }
            Expr::FuncRef(_) | Expr::SuperMember(_) | Expr::Var(_) | Expr::Null => {}
            Expr::Int(_)
            | Expr::Float(_)
            | Expr::Str(_)
            | Expr::Bool(_)
            | Expr::MethodRef(_) => {}
            Expr::Coalesce(left, right) => {
                resolve_expr(left, enums, unions);
                resolve_expr(right, enums, unions);
            }
            Expr::Unwrap(value) => resolve_expr(value, enums, unions),
            Expr::InstanceOf(value, _) => resolve_expr(value, enums, unions),
            Expr::Spread(value) => resolve_expr(value, enums, unions),
            Expr::Await(value) => resolve_expr(value, enums, unions),
            Expr::Comptime(value) => resolve_expr(value, enums, unions),
            Expr::Match(expression) => {
                resolve_expr(&mut expression.subject, enums, unions);
                for arm in &mut expression.arms {
                    resolve_expr(&mut arm.body, enums, unions);
                }
            }
        }
    }

    fn resolve_stmt(stmt: &mut Stmt, enums: &HashSet<String>, unions: &HashSet<String>) {
        match &mut stmt.kind {
            StmtKind::Defer(inner) => resolve_stmt(inner, enums, unions),
            StmtKind::MultiDecl(decls) => {
                for decl in decls {
                    resolve(&mut decl.ty, enums, unions);
                }
            }
            StmtKind::Decl(decl) => {
                resolve(&mut decl.ty, enums, unions);
                resolve_expr(&mut decl.value, enums, unions);
            }
            StmtKind::Assign(_, _, value) => resolve_expr(value, enums, unions),
            StmtKind::AssignMember(base, _, _, value) => {
                resolve_expr(base, enums, unions);
                resolve_expr(value, enums, unions);
            }
            StmtKind::AssignIndex(base, index, _, value) => {
                resolve_expr(base, enums, unions);
                resolve_expr(index, enums, unions);
                resolve_expr(value, enums, unions);
            }
            StmtKind::Expr(expr) => resolve_expr(expr, enums, unions),
            StmtKind::Block(body) => {
                for statement in body {
                    resolve_stmt(statement, enums, unions);
                }
            }
            StmtKind::If(condition, yes, no) => {
                resolve_expr(condition, enums, unions);
                for statement in yes {
                    resolve_stmt(statement, enums, unions);
                }
                for statement in no {
                    resolve_stmt(statement, enums, unions);
                }
            }
            StmtKind::While(condition, body) => {
                resolve_expr(condition, enums, unions);
                for statement in body {
                    resolve_stmt(statement, enums, unions);
                }
            }
            StmtKind::For {
                start,
                stop,
                step,
                body,
                ..
            } => {
                resolve_expr(start, enums, unions);
                resolve_expr(stop, enums, unions);
                resolve_expr(step, enums, unions);
                for statement in body {
                    resolve_stmt(statement, enums, unions);
                }
            }
            StmtKind::Return(value) => {
                if let Some(value) = value {
                    resolve_expr(value, enums, unions);
                }
            }
        }
    }

    for top in program.iter_mut() {
        match top {
            Top::Test(test) => {
                for statement in &mut test.body {
                    resolve_stmt(statement, &enums, &unions);
                }
            }
            Top::Fun(function) => {
                resolve_callable(function, &enums, &unions);
                for statement in &mut function.body {
                    resolve_stmt(statement, &enums, &unions);
                }
            }
            Top::Stmt(statement) => resolve_stmt(statement, &enums, &unions),
            Top::Struct(definition) => {
                for field in &mut definition.fields {
                    resolve(&mut field.ty, &enums, &unions);
                }
            }
            Top::Interface(definition) => {
                for method in &mut definition.methods {
                    resolve_callable(method, &enums, &unions);
                }
            }
            Top::Trait(definition) => {
                for method in &mut definition.methods {
                    resolve_callable(method, &enums, &unions);
                }
            }
            Top::Class(definition) => {
                for field in &mut definition.fields {
                    resolve(&mut field.ty, &enums, &unions);
                    if let Some(default) = &mut field.default {
                        resolve_expr(default, &enums, &unions);
                    }
                }
                for method in &mut definition.methods {
                    resolve_callable(method, &enums, &unions);
                    for statement in &mut method.body {
                        resolve_stmt(statement, &enums, &unions);
                    }
                }
                if let Some(constructor) = &mut definition.constructor {
                    resolve_callable(constructor, &enums, &unions);
                    for statement in &mut constructor.body {
                        resolve_stmt(statement, &enums, &unions);
                    }
                }
            }
            Top::Enum(_) => {}
            Top::TypeAlias(_) => {}
            Top::Union(definition) => {
                for variant in &mut definition.variants {
                    if let Some(payload) = &mut variant.payload {
                        resolve(payload, &enums, &unions);
                    }
                }
            }
            Top::Mixin(definition) => {
                for field in &mut definition.fields {
                    resolve(&mut field.ty, &enums, &unions);
                }
                for method in &mut definition.methods {
                    resolve_callable(method, &enums, &unions);
                }
            }
            Top::Import { .. } => {}
        }
    }
}

/// Resolves type alias references into their underlying types.
fn resolve_type_aliases(program: &mut [Top]) {
    let mut aliases = std::collections::HashMap::new();
    for top in program.iter() {
        if let Top::TypeAlias(alias) = top {
            aliases.insert(alias.name.clone(), alias.ty.clone());
        }
    }
    if aliases.is_empty() { return; }
    fn resolve(ty: &mut Ty, aliases: &std::collections::HashMap<String, Ty>) {
        match ty {
            Ty::Struct(name) => { if let Some(a) = aliases.get(name) { *ty = a.clone(); } }
            Ty::List(e) => resolve(e, aliases),
            Ty::Map(k, v) => { resolve(k, aliases); resolve(v, aliases); }
            Ty::Tuple(Some(es)) => { for e in es { resolve(e, aliases); } }
            Ty::Applied(_, args) => { for a in args { resolve(a, aliases); } }
            Ty::Optional(i) => resolve(i, aliases),
            _ => {}
        }
    }
    for top in program.iter_mut() {
        match top {
            Top::Stmt(statement) => {
                // inline resolve for statements
                if let StmtKind::Decl(decl) = &mut statement.kind {
                    resolve(&mut decl.ty, &aliases);
                }
            }
            Top::Fun(f) => { for p in &mut f.params { resolve(&mut p.ty, &aliases); } resolve(&mut f.ret, &aliases); }
            Top::Struct(d) => { for f in &mut d.fields { resolve(&mut f.ty, &aliases); } }
            Top::TypeAlias(a) => resolve(&mut a.ty, &aliases),
            Top::Interface(d) => { for m in &mut d.methods { for p in &mut m.params { resolve(&mut p.ty, &aliases); } resolve(&mut m.ret, &aliases); } }
            Top::Trait(d) => { for m in &mut d.methods { for p in &mut m.params { resolve(&mut p.ty, &aliases); } resolve(&mut m.ret, &aliases); } }
            Top::Class(d) => {
                for f in &mut d.fields { resolve(&mut f.ty, &aliases); }
                for m in &mut d.methods { for p in &mut m.params { resolve(&mut p.ty, &aliases); } resolve(&mut m.ret, &aliases); }
                if let Some(c) = &mut d.constructor { for p in &mut c.params { resolve(&mut p.ty, &aliases); } }
            }
            Top::Union(d) => { for v in &mut d.variants { if let Some(p) = &mut v.payload { resolve(p, &aliases); } } }
            Top::Mixin(d) => { for f in &mut d.fields { resolve(&mut f.ty, &aliases); } for m in &mut d.methods { for p in &mut m.params { resolve(&mut p.ty, &aliases); } resolve(&mut m.ret, &aliases); } }
            _ => {}
        }
    }
}

/// Copies mixin members and default trait methods into the classes that use them.
fn expand_program(program: &mut [Top]) {
    let mixins: std::collections::HashMap<String, MixinDef> = program
        .iter()
        .filter_map(|top| match top {
            Top::Mixin(mixin) => Some((mixin.name.clone(), mixin.clone())),
            _ => None,
        })
        .collect();
    let traits: std::collections::HashMap<String, TraitDef> = program
        .iter()
        .filter_map(|top| match top {
            Top::Trait(trait_def) => Some((trait_def.name.clone(), trait_def.clone())),
            _ => None,
        })
        .collect();

    fn collect_defaults(
        traits: &std::collections::HashMap<String, TraitDef>,
        name: &str,
        collected: &mut Vec<Fun>,
        visited: &mut std::collections::HashSet<String>,
    ) {
        if !visited.insert(name.to_owned()) {
            return;
        }
        let Some(trait_def) = traits.get(name) else {
            return;
        };
        for parent in &trait_def.parents {
            collect_defaults(traits, parent, collected, visited);
        }
        for method in &trait_def.methods {
            if method.body.is_empty() {
                continue;
            }
            collected.retain(|existing| existing.name != method.name);
            let mut method = method.clone();
            method.trait_default = Some(name.to_owned());
            collected.push(method);
        }
    }

    for top in program.iter_mut() {
        let Top::Class(class) = top else {
            continue;
        };
        let mut inherited: Vec<Fun> = Vec::new();
        for name in &class.interfaces {
            if traits.contains_key(name) {
                let mut visited = std::collections::HashSet::new();
                collect_defaults(&traits, name, &mut inherited, &mut visited);
            }
        }
        for name in &class.mixins {
            let Some(mixin) = mixins.get(name) else {
                continue;
            };
            for field in &mixin.fields {
                if !class.fields.iter().any(|existing| existing.name == field.name) {
                    class.fields.push(field.clone());
                }
            }
            for method in &mixin.methods {
                if !inherited.iter().any(|existing| existing.name == method.name) {
                    inherited.push(method.clone());
                }
            }
        }
        for method in inherited {
            if !class.methods.iter().any(|existing| existing.name == method.name) {
                class.methods.push(method);
            }
        }
    }
}

pub(crate) fn parse(tokens: Vec<Token>, source: &str) -> Result<Vec<Top>, Vec<String>> {
    let mut parser = Parser {
        t: tokens,
        i: 0,
        source,
        errors: Vec::new(),
        generic_types: Vec::new(),
        pending: Vec::new(),
        anonymous_classes: 0,
    };
    let mut program = parser.program();
    program.extend(parser.pending.drain(..));
    expand_program(&mut program);
    resolve_named_types(&mut program);
    resolve_type_aliases(&mut program);
    if parser.errors.is_empty() {
        Ok(program)
    } else {
        Err(parser.errors)
    }
}
