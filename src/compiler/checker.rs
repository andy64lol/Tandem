use std::collections::{HashMap, HashSet};

use crate::compiler::ast::{
    module_basename, parse_printf_format, retyped_global_names, Export, Expr, FnSig, FormatPart,
    ListElement, ModuleExports, Stmt, StructDef, Top, Ty,
};

#[derive(Clone)]
struct Binding {
    ty: Ty,
    mutable: bool,
}
struct Checker {
    scopes: Vec<HashMap<String, Binding>>,
    funcs: HashMap<String, FnSig>,
    modules: HashMap<String, ModuleExports>,
    structs: HashMap<String, HashMap<String, Ty>>,
    unstable_globals: HashSet<String>,
    function_depth: usize,
}
impl Checker {
    fn new(
        modules: HashMap<String, ModuleExports>,
        structs: HashMap<String, HashMap<String, Ty>>,
        unstable_globals: HashSet<String>,
    ) -> Self {
        Self {
            scopes: vec![HashMap::new()],
            funcs: HashMap::new(),
            modules,
            structs,
            unstable_globals,
            function_depth: 0,
        }
    }
    fn module_exports(&self, base: &Expr) -> Result<(&ModuleExports, String), String> {
        let Expr::Var(namespace) = base else {
            return Err("module access requires a module variable".into());
        };
        let binding = self
            .lookup(namespace)
            .ok_or_else(|| format!("unknown variable '{namespace}'"))?;
        let Ty::Module(id) = binding.ty else {
            return Err(format!("'{namespace}' is not a module"));
        };
        let exports = self
            .modules
            .get(&id)
            .ok_or_else(|| format!("unknown module '{id}'"))?;
        Ok((exports, namespace.clone()))
    }
    fn member_type(&self, base: &Expr, name: &str) -> Result<Ty, String> {
        match self.expr(base)? {
            Ty::Struct(struct_name) => self
                .structs
                .get(&struct_name)
                .and_then(|fields| fields.get(name))
                .cloned()
                .ok_or_else(|| format!("struct '{struct_name}' has no field '{name}'")),
            Ty::Table | Ty::Dict => Ok(Ty::Any),
            Ty::Module(_) => {
                let (exports, namespace) = self.module_exports(base)?;
                if exports.dynamic {
                    return Ok(Ty::Any);
                }
                match exports.items.get(name) {
                    Some(Export::Var(ty)) => Ok(ty.clone()),
                    Some(Export::Fn(_)) => {
                        Err(format!("function '{namespace}.{name}' must be called"))
                    }
                    None => Err(format!("module has no member '{name}'")),
                }
            }
            ty => Err(format!("cannot access field '{name}' on value of type {ty:?}")),
        }
    }
    fn method_return(&self, base: &Expr, name: &str, args: &[Expr]) -> Result<Ty, String> {
        let (exports, namespace) = self.module_exports(base)?;
        if exports.dynamic {
            for arg in args {
                self.expr(arg)?;
            }
            return Ok(Ty::Any);
        }
        match exports.items.get(name) {
            Some(Export::Var(_)) => Err(format!("'{namespace}.{name}' is not a function")),
            Some(Export::Fn(signature)) => {
                if args.len() < signature.required || args.len() > signature.args.len() {
                    return Err(format!(
                        "'{namespace}.{name}' expects {}..{} arguments, got {}",
                        signature.required,
                        signature.args.len(),
                        args.len()
                    ));
                }
                for (arg, ty) in args.iter().zip(&signature.args) {
                    let got = self.expr(arg)?;
                    if &got != ty {
                        return Err(format!(
                            "argument to '{namespace}.{name}' expects {ty:?}, got {got:?}"
                        ));
                    }
                }
                Ok(signature.ret.clone())
            }
            None => Err(format!("module has no member '{name}'")),
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
    fn lookup_scope(&self, n: &str) -> Option<usize> {
        self.scopes.iter().rposition(|scope| scope.contains_key(n))
    }
    fn type_snapshot(&self) -> Vec<HashMap<String, Ty>> {
        self.scopes
            .iter()
            .map(|scope| {
                scope
                    .iter()
                    .map(|(name, binding)| (name.clone(), binding.ty.clone()))
                    .collect()
            })
            .collect()
    }
    fn restore_types(&mut self, snapshot: &[HashMap<String, Ty>]) {
        for (scope, types) in self.scopes.iter_mut().zip(snapshot) {
            for (name, binding) in scope {
                if let Some(ty) = types.get(name) {
                    binding.ty = ty.clone();
                }
            }
        }
    }
    fn merge_types(&mut self, left: &[HashMap<String, Ty>], right: &[HashMap<String, Ty>]) {
        for (index, scope) in self.scopes.iter_mut().enumerate() {
            for (name, binding) in scope {
                let left_ty = left.get(index).and_then(|types| types.get(name));
                let right_ty = right.get(index).and_then(|types| types.get(name));
                binding.ty = match (left_ty, right_ty) {
                    (Some(left_ty), Some(right_ty)) if left_ty == right_ty => left_ty.clone(),
                    (Some(_), Some(_)) => Ty::Any,
                    _ => binding.ty.clone(),
                };
            }
        }
    }
    fn set_type(&mut self, name: &str, ty: Ty) {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(binding) = scope.get_mut(name) {
                binding.ty = ty;
                return;
            }
        }
        unreachable!("type update requires a declared variable")
    }
    fn declare(&mut self, n: &str, t: Ty, mutable: bool) -> Result<(), String> {
        let s = self.scopes.last_mut().unwrap();
        if s.contains_key(n) {
            return Err(format!("duplicate variable '{n}' in this scope"));
        }
        s.insert(n.into(), Binding { ty: t, mutable });
        Ok(())
    }
    fn current_binding(&self, name: &str) -> Option<Binding> {
        self.scopes.last().and_then(|scope| scope.get(name)).cloned()
    }
    fn collection_compatible(declared: &Ty, existing: &Ty) -> bool {
        match (declared, existing) {
            (Ty::Dict, Ty::Dict) => true,
            (Ty::List(declared), Ty::List(existing)) => declared == existing,
            (Ty::Tuple(_), Ty::Tuple(_)) => true,
            _ => false,
        }
    }
    fn contains_cast(expr: &Expr) -> bool {
        match expr {
            Expr::Cast(_, _) => true,
            Expr::Unary(_, value) => Self::contains_cast(value),
            Expr::Bin(left, _, right) => {
                Self::contains_cast(left) || Self::contains_cast(right)
            }
            Expr::Call(_, args) => args.iter().any(Self::contains_cast),
            Expr::MethodCall(base, _, args) => {
                Self::contains_cast(base) || args.iter().any(Self::contains_cast)
            }
            Expr::Member(base, _) => Self::contains_cast(base),
            Expr::Index(base, index) => Self::contains_cast(base) || Self::contains_cast(index),
            Expr::Table(fields) => fields.iter().any(|(_, value)| Self::contains_cast(value)),
            Expr::List(items) => items.iter().any(|item| match item {
                ListElement::Value(value) => Self::contains_cast(value),
                ListElement::Previous => false,
            }),
            Expr::Tuple(values) => values.iter().any(Self::contains_cast),
            Expr::Int(_) | Expr::Float(_) | Expr::Str(_) | Expr::Bool(_) | Expr::Var(_) => false,
        }
    }
    fn check_list_replacement(&self, expr: &Expr, element_ty: &Ty) -> Result<Ty, String> {
        let Expr::List(values) = expr else {
            return self.expr_as(expr, &Ty::List(Box::new(element_ty.clone())));
        };
        for (index, value) in values.iter().enumerate() {
            match value {
                ListElement::Previous => {}
                ListElement::Value(value) => {
                    let got = self.expr_as(value, element_ty)?;
                    if &got != element_ty {
                        return Err(format!(
                            "list element {} expects {element_ty:?}, got {got:?}",
                            index + 1
                        ));
                    }
                }
            }
        }
        Ok(Ty::List(Box::new(element_ty.clone())))
    }
    fn check_redefinition(&mut self, decl: &crate::compiler::ast::VarDecl) -> Result<(), String> {
        let Some(binding) = self.current_binding(&decl.name) else {
            return Err("internal error: missing declaration binding".into());
        };
        if !Self::collection_compatible(&decl.ty, &binding.ty) {
            return Err(format!(
                "variable '{}' is already declared as {:?}; collection redefinition must keep its type",
                decl.name, binding.ty
            ));
        }
        if !binding.mutable {
            return Err(format!("cannot redefine constant '{}'", decl.name));
        }
        if decl.is_const {
            return Err(format!(
                "cannot make existing variable '{}' const",
                decl.name
            ));
        }
        if binding.ty == Ty::Tuple(None) {
            return Err(format!(
                "cannot redefine '{}': its tuple shape is not statically known",
                decl.name
            ));
        }
        if let Expr::Tuple(values) = &decl.value {
            if Self::contains_cast(&decl.value) {
                return Err("'as' is not allowed inside a tuple".into());
            }
            if let Ty::Tuple(Some(expected)) = &binding.ty {
                self.check_tuple_values(values, expected)?;
                return Ok(());
            }
        }
        let got = match &binding.ty {
            Ty::List(element) => self.check_list_replacement(&decl.value, element)?,
            ty => self.expr_as(&decl.value, ty)?,
        };
        if got != binding.ty {
            return Err(format!(
                "redefinition of '{}' expects {:?}, got {got:?}",
                decl.name, binding.ty
            ));
        }
        Ok(())
    }
    fn check_tuple_values(&self, values: &[Expr], expected: &[Ty]) -> Result<(), String> {
        if values.iter().any(Self::contains_cast) {
            return Err("'as' is not allowed inside a tuple".into());
        }
        let mut mismatch = None;
        for (index, (value, expected_ty)) in values.iter().zip(expected).enumerate() {
            let got = self.expr(value)?;
            if got != expected_ty.clone() && mismatch.is_none() {
                mismatch = Some(format!(
                    "element {} expects {expected_ty:?}, got {got:?}",
                    index + 1
                ));
            }
        }
        if values.len() != expected.len() || mismatch.is_some() {
            let count = format!(
                "tuple expects {} values, got {}",
                expected.len(),
                values.len()
            );
            return Err(match mismatch {
                Some(mismatch) => format!("{count}; {mismatch}"),
                None => count,
            });
        }
        Ok(())
    }
    fn expr_as(&self, e: &Expr, expected: &Ty) -> Result<Ty, String> {
        if *expected == Ty::Any {
            if self.expr(e)? == Ty::Void {
                return Err("table values cannot have type 'void'".into());
            }
            return Ok(Ty::Any);
        }
        if let Expr::Table(fields) = e {
            match expected {
                Ty::Struct(name) => {
                    let struct_fields = self
                        .structs
                        .get(name)
                        .ok_or_else(|| format!("unknown struct type '{name}'"))?;
                    let mut provided = HashMap::new();
                    for (key, value) in fields {
                        let Some(key) = key else {
                            return Err(format!("initializer for '{name}' requires named fields"));
                        };
                        let Some(field_ty) = struct_fields.get(key) else {
                            return Err(format!("struct '{name}' has no field '{key}'"));
                        };
                        if provided.insert(key.clone(), ()).is_some() {
                            return Err(format!("field '{key}' is initialized more than once"));
                        }
                        let got = self.expr_as(value, field_ty)?;
                        if &got != field_ty {
                            return Err(format!(
                                "field '{name}.{key}' expects {field_ty:?}, got {got:?}"
                            ));
                        }
                    }
                    if let Some(missing) = struct_fields
                        .keys()
                        .find(|field| !provided.contains_key(*field))
                    {
                        return Err(format!("initializer for '{name}' is missing field '{missing}'"));
                    }
                    return Ok(expected.clone());
                }
                Ty::Table => {
                    for (_, value) in fields {
                        if self.expr(value)? == Ty::Void {
                            return Err("table values cannot have type 'void'".into());
                        }
                    }
                    return Ok(Ty::Table);
                }
                Ty::Dict => {
                    let mut keys = HashMap::new();
                    for (key, value) in fields {
                        let Some(key) = key else {
                            return Err("dictionary initializers require named keys".into());
                        };
                        if keys.insert(key, ()).is_some() {
                            return Err(format!(
                                "dictionary key '{key}' is initialized more than once"
                            ));
                        }
                        if self.expr(value)? == Ty::Void {
                            return Err("dictionary values cannot have type 'void'".into());
                        }
                    }
                    return Ok(Ty::Dict);
                }
                _ => {}
            }
        }
        match (expected, e) {
            (Ty::List(element_ty), Expr::List(values)) => {
                for (index, value) in values.iter().enumerate() {
                    let ListElement::Value(value) = value else {
                        return Err("'...' can only be used when redefining a list".into());
                    };
                    let got = self.expr_as(value, element_ty)?;
                    if &got != element_ty.as_ref() {
                        return Err(format!(
                            "list element {} expects {element_ty:?}, got {got:?}",
                            index + 1
                        ));
                    }
                }
                return Ok(expected.clone());
            }
            (Ty::Tuple(expected_types), Expr::Tuple(values)) => {
                if let Some(expected_types) = expected_types {
                    self.check_tuple_values(values, expected_types)?;
                    return Ok(expected.clone());
                }
                for value in values {
                    let ty = self.expr(value)?;
                    if ty == Ty::Any || ty == Ty::Void {
                        return Err("tuple values need statically known, non-void types".into());
                    }
                }
                return Ok(expected.clone());
            }
            _ => {}
        }
        let got = self.expr(e)?;
        if matches!(expected, Ty::Tuple(None)) && matches!(got, Ty::Tuple(_)) {
            return Ok(expected.clone());
        }
        if got == Ty::Any {
            return Err(format!(
                "cannot use a dynamically typed value where {expected:?} is required"
            ));
        }
        Ok(got)
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
            Expr::Cast(value, ty) => {
                if *ty == Ty::Void {
                    return Err("cannot cast to 'void'".into());
                }
                validate_type(ty, &self.structs)?;
                let source_ty = self.expr(value)?;
                if source_ty != Ty::Any && source_ty != *ty {
                    return Err(format!("cannot cast {source_ty:?} to {ty:?}"));
                }
                Ok(ty.clone())
            }
            Expr::Member(base, name) => self.member_type(base, name),
            Expr::MethodCall(base, name, args) => self.method_return(base, name, args),
            Expr::Index(base, index) => {
                let base_ty = self.expr(base)?;
                let index_ty = self.expr(index)?;
                match base_ty {
                    Ty::Table => {
                        if index_ty != Ty::Int && index_ty != Ty::Str {
                            return Err(format!("table index must be int or str, got {index_ty:?}"));
                        }
                        Ok(Ty::Any)
                    }
                    Ty::Dict => {
                        if index_ty != Ty::Str {
                            return Err(format!("dictionary index must be str, got {index_ty:?}"));
                        }
                        Ok(Ty::Any)
                    }
                    Ty::List(element_ty) => {
                        if index_ty != Ty::Int {
                            return Err(format!("list index must be int, got {index_ty:?}"));
                        }
                        Ok(*element_ty)
                    }
                    Ty::Tuple(Some(types)) => {
                        if index_ty != Ty::Int {
                            return Err(format!("tuple index must be int, got {index_ty:?}"));
                        }
                        if let Expr::Int(index) = index.as_ref() {
                            if *index < 1 || *index as usize > types.len() {
                                return Err(format!(
                                    "tuple index {index} is out of range for {} values",
                                    types.len()
                                ));
                            }
                            Ok(types[*index as usize - 1].clone())
                        } else {
                            Ok(Ty::Any)
                        }
                    }
                    Ty::Tuple(None) => {
                        if index_ty != Ty::Int {
                            Err(format!("tuple index must be int, got {index_ty:?}"))
                        } else {
                            Ok(Ty::Any)
                        }
                    }
                    ty => Err(format!("cannot index value of type {ty:?}")),
                }
            }
            Expr::Table(fields) => {
                for (_, value) in fields {
                    if self.expr(value)? == Ty::Void {
                        return Err("table values cannot have type 'void'".into());
                    }
                }
                Ok(Ty::Table)
            }
            Expr::List(values) => {
                let mut element_ty = None;
                for value in values {
                    let ListElement::Value(value) = value else {
                        return Err("'...' can only be used when redefining a list".into());
                    };
                    let got = self.expr(value)?;
                    if got == Ty::Void {
                        return Err("list values cannot have type 'void'".into());
                    }
                    if let Some(expected) = &element_ty {
                        if *expected != got {
                            return Err(format!(
                                "list values must all have one type, got {expected:?} and {got:?}"
                            ));
                        }
                    } else {
                        element_ty = Some(got);
                    }
                }
                Ok(Ty::List(Box::new(element_ty.unwrap_or(Ty::Any))))
            }
            Expr::Tuple(values) => {
                let mut types = Vec::with_capacity(values.len());
                for value in values {
                    let ty = self.expr(value)?;
                    if ty == Ty::Any || ty == Ty::Void {
                        return Err("tuple values need statically known, non-void types".into());
                    }
                    types.push(ty);
                }
                Ok(Ty::Tuple(Some(types)))
            }
            Expr::Call(n, args) => {
                if n == "require" {
                    let [Expr::Str(path)] = args.as_slice() else {
                        return Err("require expects one string literal argument".into());
                    };
                    if !self.modules.contains_key(path) {
                        return Err(format!("unknown module '{path}'"));
                    }
                    return Ok(Ty::Module(path.clone()));
                }
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
                    let got = self.expr_as(a, t)?;
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
                if self.current_binding(&decl.name).is_some() {
                    return self.check_redefinition(decl);
                }
                if !decl.is_const
                    && matches!(decl.ty, Ty::Tuple(_))
                    && Self::contains_cast(&decl.value)
                {
                    return Err("'as' inside a tuple requires a const tuple".into());
                }
                let got = if matches!(decl.ty, Ty::Tuple(None)) {
                    self.expr(&decl.value)?
                } else {
                    self.expr_as(&decl.value, &decl.ty)?
                };
                let valid = match (&decl.ty, &got) {
                    (Ty::Tuple(None), Ty::Tuple(Some(_))) => true,
                    (declared, got) => declared == got,
                };
                if !valid {
                    return Err(format!(
                        "variable '{}' declared {:?}, initializer is {got:?}",
                        decl.name, decl.ty
                    ));
                }
                self.declare(&decl.name, got, !decl.is_const)
            }
            Stmt::Assign(n, op, e) => {
                let Some(binding) = self.lookup(n) else {
                    let inferred = self.expr(e)?;
                    if op == "=" {
                        if let Ty::Module(_) = inferred {
                            return self.declare(n, inferred, false);
                        }
                    }
                    return Err(format!("assignment to unknown variable '{n}'"));
                };
                if !binding.mutable {
                    return Err(format!("cannot assign to constant '{n}'"));
                }
                if binding.ty == Ty::Tuple(None) && op == "=" {
                    return Err(format!(
                        "cannot redefine '{n}': its tuple shape is not statically known"
                    ));
                }
                if let Ty::List(element_ty) = &binding.ty {
                    if op != "=" {
                        return Err("lists can only be changed by whole-list assignment".into());
                    }
                    let got = self.check_list_replacement(e, element_ty)?;
                    if got != binding.ty {
                        return Err(format!("list assignment to '{n}' changed its element type"));
                    }
                    return Ok(());
                }
                if matches!(binding.ty, Ty::Tuple(_)) && op != "=" {
                    return Err("tuples can only be changed by whole-tuple or same-type element assignment".into());
                }
                let retype = if op == "=" {
                    match e {
                        Expr::Cast(_, ty) => Some(ty.clone()),
                        _ => None,
                    }
                } else {
                    None
                };
                let expected = if op == "=" {
                    if let Some(ty) = &retype {
                        ty.clone()
                    } else if binding.ty == Ty::Any {
                        return Err(format!(
                            "assignment to dynamically typed variable '{n}' requires an explicit 'as' type"
                        ));
                    } else {
                        binding.ty.clone()
                    }
                } else {
                    if binding.ty != Ty::Int && binding.ty != Ty::Float {
                        return Err(format!(
                            "compound assignment {op} requires a numeric variable"
                        ));
                    }
                    binding.ty.clone()
                };
                let r = self.expr_as(e, &expected)?;
                if r != expected {
                    return Err(format!(
                        "assignment to '{n}' expects {expected:?}, got {r:?}"
                    ));
                }
                if let Some(ty) = retype {
                    if self.function_depth > 0 && self.lookup_scope(n) == Some(0) {
                        return Err(format!(
                            "cannot change the type of global variable '{n}' inside a function"
                        ));
                    }
                    self.set_type(n, ty);
                }
                Ok(())
            }
            Stmt::Require(path) => {
                if !self.modules.contains_key(path) {
                    return Err(format!("unknown module '{path}'"));
                }
                self.declare(&module_basename(path), Ty::Module(path.clone()), false)
            }
            Stmt::AssignMember(base, name, op, value) => {
                let target_ty = self.member_type(base, name)?;
                if op != "=" && !matches!(target_ty, Ty::Int | Ty::Float | Ty::Any) {
                    return Err(format!("compound assignment {op} requires a numeric field"));
                }
                let got = self.expr_as(value, &target_ty)?;
                if got != target_ty {
                    return Err(format!(
                        "assignment to field '{name}' expects {target_ty:?}, got {got:?}"
                    ));
                }
                Ok(())
            }
            Stmt::AssignIndex(base, index, op, value) => {
                let base_ty = self.expr(base)?;
                let index_ty = self.expr(index)?;
                let target_ty = match base_ty {
                    Ty::Table => {
                        if index_ty != Ty::Int && index_ty != Ty::Str {
                            return Err(format!("table index must be int or str, got {index_ty:?}"));
                        }
                        let got = self.expr(value)?;
                        if got == Ty::Void {
                            return Err("table values cannot have type 'void'".into());
                        }
                        return Ok(());
                    }
                    Ty::Dict => {
                        if index_ty != Ty::Str {
                            return Err(format!("dictionary index must be str, got {index_ty:?}"));
                        }
                        let got = self.expr(value)?;
                        if got == Ty::Void {
                            return Err("dictionary values cannot have type 'void'".into());
                        }
                        return Ok(());
                    }
                    Ty::List(_) => {
                        return Err("list indexes cannot be assigned; replace the whole list and use '...' to keep values".into());
                    }
                    Ty::Tuple(Some(types)) => {
                        if index_ty != Ty::Int {
                            return Err(format!("tuple index must be int, got {index_ty:?}"));
                        }
                        let Expr::Int(index) = index.as_ref() else {
                            return Err("tuple assignment requires a constant index".into());
                        };
                        if *index < 1 || *index as usize > types.len() {
                            return Err(format!(
                                "tuple index {index} is out of range for {} values",
                                types.len()
                            ));
                        }
                        if let Expr::Var(name) = base.as_ref() {
                            if self.lookup(name).is_some_and(|binding| !binding.mutable) {
                                return Err(format!("cannot assign to constant tuple '{name}'"));
                            }
                        } else {
                            return Err("tuple element assignment requires a tuple variable".into());
                        }
                        types[*index as usize - 1].clone()
                    }
                    Ty::Tuple(None) => {
                        return Err("tuple shape is not known for element assignment".into())
                    }
                    ty => return Err(format!("cannot index value of type {ty:?}")),
                };
                if op != "=" && !matches!(target_ty, Ty::Int | Ty::Float) {
                    return Err(format!("compound assignment {op} requires a numeric target"));
                }
                let got = self.expr_as(value, &target_ty)?;
                if got != target_ty {
                    return Err(format!(
                        "index assignment expects {target_ty:?}, got {got:?}"
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
                let before = self.type_snapshot();
                self.push();
                for x in a {
                    self.stmt(x, ret)?;
                }
                self.pop();
                let yes_types = self.type_snapshot();
                self.restore_types(&before);
                self.push();
                for x in b {
                    self.stmt(x, ret)?;
                }
                self.pop();
                let no_types = self.type_snapshot();
                self.merge_types(&yes_types, &no_types);
                Ok(())
            }
            Stmt::While(c, b) => {
                if self.expr(c)? != Ty::Bool {
                    return Err("while condition must be bool".into());
                }
                let before = self.type_snapshot();
                self.push();
                for x in b {
                    self.stmt(x, ret)?;
                }
                self.pop();
                let after = self.type_snapshot();
                self.merge_types(&before, &after);
                if self.expr(c)? != Ty::Bool {
                    return Err("while condition must remain bool throughout the loop".into());
                }
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
                let before = self.type_snapshot();
                self.push();
                self.declare(variable, Ty::Int, true)?;
                for stmt in body {
                    self.stmt(stmt, ret)?;
                }
                self.pop();
                let after = self.type_snapshot();
                self.merge_types(&before, &after);
                Ok(())
            }
            Stmt::Return(e) => match (e, ret) {
                (None, Ty::Void) => Ok(()),
                (Some(x), r) if self.expr_as(x, r)? == *r => Ok(()),
                (None, _) => Err("return value required".into()),
                (Some(_), Ty::Void) => Err("void function cannot return a value".into()),
                (Some(x), r) => Err(format!("return expects {r:?}, got {:?}", self.expr(x)?)),
            },
        }
    }
}

fn collect_structs(program: &[Top]) -> Result<HashMap<String, HashMap<String, Ty>>, String> {
    let mut structs = HashMap::new();
    for top in program {
        let Top::Struct(definition) = top else {
            continue;
        };
        let mut fields = HashMap::new();
        for field in &definition.fields {
            if field.ty == Ty::Void {
                return Err(format!(
                    "field '{}.{}' cannot have type 'void'",
                    definition.name, field.name
                ));
            }
            if fields.insert(field.name.clone(), field.ty.clone()).is_some() {
                return Err(format!(
                    "duplicate field '{}.{}'",
                    definition.name, field.name
                ));
            }
        }
        if structs.insert(definition.name.clone(), fields).is_some() {
            return Err(format!("duplicate struct '{}'", definition.name));
        }
    }
    for fields in structs.values() {
        for ty in fields.values() {
            validate_type(ty, &structs)?;
        }
    }
    Ok(structs)
}

fn validate_type(ty: &Ty, structs: &HashMap<String, HashMap<String, Ty>>) -> Result<(), String> {
    match ty {
        Ty::Struct(name) if !structs.contains_key(name) => {
            Err(format!("unknown struct type '{name}'"))
        }
        Ty::List(element) => validate_type(element, structs),
        Ty::Tuple(Some(elements)) => {
            for element in elements {
                validate_type(element, structs)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_program_types(
    program: &[Top],
    structs: &HashMap<String, HashMap<String, Ty>>,
) -> Result<(), String> {
    fn validate_stmts(
        statements: &[Stmt],
        structs: &HashMap<String, HashMap<String, Ty>>,
    ) -> Result<(), String> {
        for statement in statements {
            match statement {
                Stmt::Decl(decl) => validate_type(&decl.ty, structs)?,
                Stmt::Block(body) | Stmt::While(_, body) => validate_stmts(body, structs)?,
                Stmt::If(_, yes, no) => {
                    validate_stmts(yes, structs)?;
                    validate_stmts(no, structs)?;
                }
                Stmt::For { body, .. } => validate_stmts(body, structs)?,
                _ => {}
            }
        }
        Ok(())
    }

    for top in program {
        match top {
            Top::Fun(function) => {
                validate_type(&function.ret, structs)?;
                for param in &function.params {
                    validate_type(&param.ty, structs)?;
                }
                validate_stmts(&function.body, structs)?;
            }
            Top::Stmt(statement) => validate_stmts(std::slice::from_ref(statement), structs)?,
            Top::Struct(StructDef { .. }) => {}
        }
    }
    Ok(())
}

pub(crate) fn check(program: &[Top], modules: &HashMap<String, ModuleExports>) -> Result<(), String> {
    let structs = collect_structs(program)?;
    validate_program_types(program, &structs)?;
    let unstable_globals = retyped_global_names(program);
    let mut c = Checker::new(modules.clone(), structs, unstable_globals);
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
    let mut global_declarations = HashSet::new();
    for t in program {
        match t {
            Top::Stmt(Stmt::Decl(decl)) => {
                if global_declarations.insert(decl.name.clone()) {
                    c.declare(&decl.name, decl.ty.clone(), !decl.is_const)?;
                }
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
    let mut checked_globals = HashSet::new();
    for t in program {
        match t {
            Top::Stmt(Stmt::Decl(decl)) => {
                if checked_globals.insert(decl.name.clone()) {
                    if !decl.is_const
                        && matches!(decl.ty, Ty::Tuple(_))
                        && Checker::contains_cast(&decl.value)
                    {
                        return Err("'as' inside a tuple requires a const tuple".into());
                    }
                    let got = if matches!(decl.ty, Ty::Tuple(None)) {
                        c.expr(&decl.value)?
                    } else {
                        c.expr_as(&decl.value, &decl.ty)?
                    };
                    let valid = match (&decl.ty, &got) {
                        (Ty::Tuple(None), Ty::Tuple(Some(_))) => true,
                        (declared, got) => declared == got,
                    };
                    if !valid {
                        return Err(format!(
                            "variable '{}' declared {:?}, initializer is {got:?}",
                            decl.name, decl.ty
                        ));
                    }
                    c.set_type(&decl.name, got);
                } else {
                    c.check_redefinition(decl)?;
                }
            }
            Top::Stmt(s) => c.stmt(s, &Ty::Void)?,
            Top::Fun(_) => {}
            Top::Struct(_) => {}
        }
    }
    for t in program {
        let Top::Fun(f) = t else {
            continue;
        };
        let outer_types = c.type_snapshot();
        for name in c.unstable_globals.clone() {
            c.set_type(&name, Ty::Any);
        }
        c.function_depth += 1;
        c.push();
        for p in &f.params {
            if let Some(d) = &p.default {
                let got = c.expr_as(d, &p.ty)?;
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
        c.function_depth -= 1;
        c.restore_types(&outer_types);
    }
    Ok(())
}
