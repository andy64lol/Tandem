use std::collections::{HashMap, HashSet};

use crate::compiler::ast::{
    module_basename, parse_printf_format, retyped_global_names, Export, Expr, FnSig, FormatPart,
    Fun, ListElement, ModuleExports, Param, Stmt, StmtKind, StructDef, TableShape, Top, Ty,
};
use crate::compiler::stdlib;

#[derive(Clone)]
struct Binding {
    ty: Ty,
    mutable: bool,
    inferred: bool,
    shape: Option<TableShape>,
}
type Structs = HashMap<String, HashMap<String, Ty>>;
type Classes = HashMap<String, ClassInfo>;

#[derive(Clone)]
struct ClassInfo {
    parent: Option<String>,
    own_fields: HashMap<String, Ty>,
    fields: HashMap<String, Ty>,
    methods: HashMap<String, FnSig>,
    constructor: Option<FnSig>,
    line: usize,
    column: usize,
}

struct Checker {
    scopes: Vec<HashMap<String, Binding>>,
    funcs: HashMap<String, FnSig>,
    modules: HashMap<String, ModuleExports>,
    structs: Structs,
    classes: Classes,
    current_class: Option<String>,
    unstable_globals: HashSet<String>,
    imports: HashSet<String>,
    function_depth: usize,
    current_constructor: bool,
    allow_super_call: bool,
    position: Option<(usize, usize)>,
}
impl Checker {
    fn new(
        modules: HashMap<String, ModuleExports>,
        structs: Structs,
        classes: Classes,
        unstable_globals: HashSet<String>,
        imports: HashSet<String>,
    ) -> Self {
        Self {
            scopes: vec![HashMap::new()],
            funcs: HashMap::new(),
            modules,
            structs,
            classes,
            current_class: None,
            unstable_globals,
            imports,
            function_depth: 0,
            current_constructor: false,
            allow_super_call: false,
            position: None,
        }
    }
    fn at(&self, message: String) -> String {
        if message.contains(" at line ") {
            return message;
        }
        match self.position {
            Some((line, column)) => format!("{message} at line {line}, column {column}"),
            None => message,
        }
    }
    fn module_exports(&self, base: &Expr) -> Result<(&ModuleExports, String), String> {
        match base {
            Expr::Var(namespace) => {
                let Some(binding) = self.lookup(namespace) else {
                    if stdlib::package(namespace).is_some() || namespace == "base" {
                        if self.imports.contains(namespace.as_str()) {
                            return Err(format!(
                                "'{namespace}' is a standard library package and cannot be used as a value"
                            ));
                        }
                        return Err(format!(
                            "Unrecognised package '{namespace}': import it with 'import {namespace};'"
                        ));
                    }
                    return Err(format!("unknown variable '{namespace}'"));
                };
                let Ty::Module(id) = binding.ty else {
                    return Err(format!("'{namespace}' is not a module"));
                };
                let exports = self
                    .modules
                    .get(&id)
                    .ok_or_else(|| format!("unknown module '{id}'"))?;
                Ok((exports, namespace.clone()))
            }
            Expr::Call(name, args) if name == "require" => {
                let [Expr::Str(id)] = args.as_slice() else {
                    return Err("require expects one string literal argument".into());
                };
                let exports = self
                    .modules
                    .get(id)
                    .ok_or_else(|| format!("unknown module '{id}'"))?;
                Ok((exports, module_basename(id)))
            }
            _ => Err("module access requires a module value".into()),
        }
    }
    fn check_signature_call(
        &self,
        label: &str,
        signature: &FnSig,
        args: &[Expr],
    ) -> Result<Ty, String> {
        if args.len() < signature.required
            || (!signature.varargs && args.len() > signature.args.len())
        {
            let expected = if signature.varargs {
                format!("at least {}", signature.required)
            } else {
                format!("{}..{}", signature.required, signature.args.len())
            };
            return Err(format!(
                "'{label}' expects {expected} arguments, got {}",
                args.len()
            ));
        }
        for (index, arg) in args.iter().enumerate() {
            let Some(expected) = signature.args.get(index) else {
                self.expr(arg)?;
                continue;
            };
            let got = self.expr_as(arg, expected)?;
            if expected != &Ty::Any && &got != expected {
                return Err(format!(
                    "argument to '{label}' expects {expected:?}, got {got:?}"
                ));
            }
        }
        Ok(signature.ret.clone())
    }
    fn member_type(&self, base: &Expr, name: &str) -> Result<Ty, String> {
        if let Some(shape) = self.table_shape_of(base) {
            return Ok(shape.named.get(name).cloned().unwrap_or(Ty::Any));
        }
        match self.expr(base)? {
            Ty::Struct(struct_name) => {
                if let Some(field) = self
                    .structs
                    .get(&struct_name)
                    .and_then(|fields| fields.get(name))
                {
                    return Ok(field.clone());
                }
                if class_method(&self.classes, &struct_name, name).is_some() {
                    return Err(format!(
                        "class '{struct_name}' has method '{name}'; call it with '{name}(...)'"
                    ));
                }
                Err(format!("struct '{struct_name}' has no field '{name}'"))
            }
            Ty::Map(key_ty, value_ty) if *key_ty == Ty::Str => Ok(*value_ty),
            Ty::Table | Ty::Dict => Ok(Ty::Any),
            Ty::Module(_) => {
                let (exports, namespace) = self.module_exports(base)?;
                if exports.dynamic {
                    if let Some(members) = &exports.members {
                        if !members.contains(name) {
                            return Err(format!("package '{namespace}' has no member '{name}'"));
                        }
                    }
                }
                match exports.items.get(name) {
                    Some(Export::Var(ty)) => Ok(ty.clone()),
                    Some(Export::Fn(_)) => {
                        Err(format!("function '{namespace}.{name}' must be called"))
                    }
                    None if exports.dynamic => Ok(Ty::Any),
                    None => Err(format!("module has no member '{name}'")),
                }
            }
            ty => Err(format!(
                "cannot access field '{name}' on value of type {ty:?}"
            )),
        }
    }
    fn is_subclass(&self, child: &str, parent: &str) -> bool {
        let mut current = Some(child);
        while let Some(name) = current {
            if name == parent {
                return true;
            }
            current = self
                .classes
                .get(name)
                .and_then(|class| class.parent.as_deref());
        }
        false
    }
    fn accepts(&self, expected: &Ty, got: &Ty) -> bool {
        expected == got
            || match (expected, got) {
                (Ty::Struct(parent), Ty::Struct(child))
                    if self.classes.contains_key(parent) && self.classes.contains_key(child) =>
                {
                    self.is_subclass(child, parent)
                }
                _ => false,
            }
    }
    fn class_of(&self, base: &Expr) -> Option<String> {
        match self.expr(base).ok()? {
            Ty::Struct(name) if self.classes.contains_key(&name) => Some(name),
            _ => None,
        }
    }
    fn method_return(&self, base: &Expr, name: &str, args: &[Expr]) -> Result<Ty, String> {
        if let Some(class_name) = self.class_of(base) {
            let Some(signature) = class_method(&self.classes, &class_name, name) else {
                return Err(format!("class '{class_name}' has no method '{name}'"));
            };
            return self.check_signature_call(&format!("{class_name}.{name}"), signature, args);
        }
        let (exports, namespace) = self.module_exports(base)?;
        if exports.dynamic {
            if let Some(members) = &exports.members {
                if !members.contains(name) {
                    return Err(format!("package '{namespace}' has no member '{name}'"));
                }
            }
        }
        match exports.items.get(name) {
            Some(Export::Var(_)) => Err(format!("'{namespace}.{name}' is not a function")),
            Some(Export::Fn(signature)) => {
                self.check_signature_call(&format!("{namespace}.{name}"), signature, args)
            }
            None if exports.dynamic => {
                for arg in args {
                    self.expr(arg)?;
                }
                Ok(Ty::Any)
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
    fn field_type(&self, name: &str) -> Option<Ty> {
        let class = self.current_class.as_ref()?;
        self.classes
            .get(class)
            .and_then(|class| class.fields.get(name))
            .cloned()
    }
    fn implicit_field(&self, name: &str) -> Option<Ty> {
        if matches!(self.lookup_scope(name), Some(scope) if scope > 0) {
            return None;
        }
        self.field_type(name)
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
                binding.shape = None;
                return;
            }
        }
        unreachable!("type update requires a declared variable")
    }
    fn set_shape(&mut self, name: &str, shape: Option<TableShape>) {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(binding) = scope.get_mut(name) {
                binding.shape = if binding.ty == Ty::Table { shape } else { None };
                return;
            }
        }
    }
    fn table_shape(&self, value: &Expr) -> Option<TableShape> {
        let Expr::Table(fields) = value else {
            return None;
        };
        let mut shape = TableShape::default();
        for (name, value) in fields {
            let ty = self.expr(value).ok()?;
            match name {
                Some(name) => {
                    shape.named.insert(name.clone(), ty);
                }
                None => shape.positional.push(ty),
            }
        }
        Some(shape)
    }
    fn table_shape_of(&self, base: &Expr) -> Option<TableShape> {
        let Expr::Var(name) = base else {
            return None;
        };
        let binding = self.lookup(name)?;
        if binding.ty == Ty::Table {
            binding.shape
        } else {
            None
        }
    }
    fn declare(&mut self, n: &str, t: Ty, mutable: bool) -> Result<(), String> {
        self.declare_with(n, t, mutable, false)
    }
    fn declare_inferred(&mut self, n: &str, t: Ty, mutable: bool) -> Result<(), String> {
        self.declare_with(n, t, mutable, true)
    }
    fn declare_with(
        &mut self,
        n: &str,
        t: Ty,
        mutable: bool,
        inferred: bool,
    ) -> Result<(), String> {
        let s = self.scopes.last_mut().unwrap();
        if s.contains_key(n) {
            return Err(format!("duplicate variable '{n}' in this scope"));
        }
        s.insert(
            n.into(),
            Binding {
                ty: t,
                mutable,
                inferred,
                shape: None,
            },
        );
        Ok(())
    }
    fn mark_inferred(&mut self, n: &str) {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(binding) = scope.get_mut(n) {
                binding.inferred = true;
                return;
            }
        }
        unreachable!("inferred mark requires a declared variable")
    }
    fn current_binding(&self, name: &str) -> Option<Binding> {
        self.scopes
            .last()
            .and_then(|scope| scope.get(name))
            .cloned()
    }
    fn collection_compatible(declared: &Ty, existing: &Ty) -> bool {
        match (declared, existing) {
            (Ty::Dict, Ty::Dict) => true,
            (Ty::Map(declared_key, declared_value), Ty::Map(existing_key, existing_value)) => {
                declared_key == existing_key && declared_value == existing_value
            }
            (Ty::List(declared), Ty::List(existing)) => declared == existing,
            (Ty::Tuple(_), Ty::Tuple(_)) => true,
            _ => false,
        }
    }
    fn contains_cast(expr: &Expr) -> bool {
        match expr {
            Expr::Cast(_, _) => true,
            Expr::Unary(_, value) => Self::contains_cast(value),
            Expr::Bin(left, _, right) => Self::contains_cast(left) || Self::contains_cast(right),
            Expr::Call(_, args) | Expr::New(_, args) => args.iter().any(Self::contains_cast),
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
            Expr::SuperCall(args) | Expr::SuperMethod(_, args) => {
                args.iter().any(Self::contains_cast)
            }
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
        if binding.inferred {
            return Err(format!(
                "variable '{}' was declared with 'auto' and cannot be redefined",
                decl.name
            ));
        }
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
                    if self.classes.contains_key(name) {
                        return Err(format!(
                            "class '{name}' must be constructed with 'new {name}'"
                        ));
                    }
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
                        return Err(format!(
                            "initializer for '{name}' is missing field '{missing}'"
                        ));
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
            (Ty::Map(key_ty, value_ty), Expr::Table(fields)) => {
                let mut keys = HashMap::new();
                for (index, (key, value)) in fields.iter().enumerate() {
                    match key_ty.as_ref() {
                        Ty::Str => {
                            let Some(key) = key else {
                                return Err("tables with str keys require named fields".into());
                            };
                            if keys.insert(key, ()).is_some() {
                                return Err(format!(
                                    "table key '{key}' is initialized more than once"
                                ));
                            }
                            let got = self.expr_as(value, value_ty)?;
                            if &got != value_ty.as_ref() {
                                return Err(format!(
                                    "table value for key '{key}' expects {value_ty:?}, got {got:?}"
                                ));
                            }
                        }
                        Ty::Int => {
                            if let Some(key) = key {
                                return Err(format!(
                                    "tables with int keys do not accept named field '{key}'"
                                ));
                            }
                            let got = self.expr_as(value, value_ty)?;
                            if &got != value_ty.as_ref() {
                                return Err(format!(
                                    "table value {} expects {value_ty:?}, got {got:?}",
                                    index + 1
                                ));
                            }
                        }
                        other => {
                            return Err(format!("table keys must be 'str' or 'int', got {other:?}"))
                        }
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
        if self.accepts(expected, &got) {
            return Ok(expected.clone());
        }
        Ok(got)
    }
    fn expr(&self, e: &Expr) -> Result<Ty, String> {
        match e {
            Expr::Int(_) => Ok(Ty::Int),
            Expr::Float(_) => Ok(Ty::Float),
            Expr::Str(_) => Ok(Ty::Str),
            Expr::Bool(_) => Ok(Ty::Bool),
            Expr::Var(n) => {
                if let Some(ty) = self.implicit_field(n) {
                    return Ok(ty);
                }
                if let Some(binding) = self.lookup(n) {
                    return Ok(binding.ty);
                }
                if stdlib::package(n).is_some() || n == "base" {
                    if self.imports.contains(n) {
                        return Err(format!(
                            "'{n}' is a standard library package and cannot be used as a value"
                        ));
                    }
                    return Err(format!(
                        "Unrecognised package '{n}': import it with 'import {n};'"
                    ));
                }
                if stdlib::BASE_VALUES.contains(&n.as_str()) {
                    if self.imports.contains("base") {
                        return Ok(stdlib::base_value(n).unwrap_or(Ty::Any));
                    }
                    return Err("Unrecognised package 'base': import it with 'import base;'".into());
                }
                Err(format!("unknown variable '{n}'"))
            }
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
                    ".." if x == Ty::Str && y == Ty::Str => Ok(Ty::Str),
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
                if let Some(shape) = self.table_shape_of(base) {
                    match (index.as_ref(), &index_ty) {
                        (Expr::Str(key), Ty::Str) => {
                            return Ok(shape.named.get(key).cloned().unwrap_or(Ty::Any));
                        }
                        (Expr::Int(index), Ty::Int)
                            if *index > 0 && (*index as usize) <= shape.positional.len() =>
                        {
                            return Ok(shape.positional[*index as usize - 1].clone());
                        }
                        _ => {}
                    }
                }
                match base_ty {
                    Ty::Table => {
                        if index_ty != Ty::Int && index_ty != Ty::Str {
                            return Err(format!(
                                "table index must be int or str, got {index_ty:?}"
                            ));
                        }
                        Ok(Ty::Any)
                    }
                    Ty::Dict => {
                        if index_ty != Ty::Str {
                            return Err(format!("dictionary index must be str, got {index_ty:?}"));
                        }
                        Ok(Ty::Any)
                    }
                    Ty::Map(key_ty, value_ty) => {
                        if index_ty != *key_ty {
                            return Err(format!("table key expects {key_ty:?}, got {index_ty:?}"));
                        }
                        Ok(*value_ty)
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
            Expr::New(name, args) => {
                if self.lookup(name).is_some() {
                    return Err(format!(
                        "class '{name}' is shadowed by a variable with the same name"
                    ));
                }
                let Some(class) = self.classes.get(name) else {
                    return Err(format!("unknown class '{name}'"));
                };
                let constructor = class
                    .constructor
                    .clone()
                    .or_else(|| {
                        class
                            .parent
                            .as_ref()
                            .map(|parent| effective_constructor(&self.classes, parent))
                    })
                    .unwrap_or_else(empty_signature);
                self.check_signature_call(&format!("{name}.constructor"), &constructor, args)?;
                Ok(Ty::Struct(name.clone()))
            }
            Expr::SuperCall(args) => {
                let Some(class_name) = self.current_class.clone() else {
                    return Err("'super(...)' can only be used inside a class".into());
                };
                if !self.current_constructor {
                    return Err("'super(...)' can only be called from a constructor".into());
                }
                let Some(parent) = self.classes[&class_name].parent.clone() else {
                    return Err(format!(
                        "class '{class_name}' has no base class to call with 'super(...)'"
                    ));
                };
                if !self.allow_super_call {
                    return Err(
                        "'super(...)' must be the first statement in a derived constructor".into(),
                    );
                }
                let constructor = effective_constructor(&self.classes, &parent);
                self.check_signature_call(&format!("{parent}.constructor"), &constructor, args)?;
                Ok(Ty::Void)
            }
            Expr::SuperMethod(name, args) => {
                let Some(class_name) = self.current_class.clone() else {
                    return Err(format!(
                        "'super.{name}(...)' can only be used inside a class"
                    ));
                };
                let Some(parent) = self.classes[&class_name].parent.clone() else {
                    return Err(format!(
                        "class '{class_name}' has no base class to call with 'super.{name}(...)'"
                    ));
                };
                let Some(signature) = class_method(&self.classes, &parent, name) else {
                    return Err(format!("class '{parent}' has no method '{name}'"));
                };
                self.check_signature_call(&format!("{parent}.{name}"), signature, args)
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
                if let Some(f) = self.funcs.get(n) {
                    if args.len() < f.required || (!f.varargs && args.len() > f.args.len()) {
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
                    return Ok(f.ret.clone());
                }
                if stdlib::BASE_FUNCTIONS.contains(&n.as_str()) {
                    if !self.imports.contains("base") {
                        return Err(format!(
                            "Unrecognised package 'base': import it with 'import base;' to use '{n}'"
                        ));
                    }
                    if let Some(signature) = stdlib::base_signature(n) {
                        let signature = FnSig {
                            args: signature.args.to_vec(),
                            required: signature.required,
                            ret: signature.ret.clone(),
                            varargs: signature.varargs,
                        };
                        return self.check_signature_call(n, &signature, args);
                    }
                    for arg in args {
                        self.expr(arg)?;
                    }
                    return Ok(Ty::Any);
                }
                Err(format!("unknown function '{n}'"))
            }
        }
    }
    fn stmt(&mut self, s: &Stmt, ret: &Ty) -> Result<(), String> {
        let previous = self.position;
        self.position = Some((s.line, s.column));
        let result = self.stmt_inner(s, ret);
        self.position = previous;
        result.map_err(|message| self.at(message))
    }
    fn stmt_inner(&mut self, s: &Stmt, ret: &Ty) -> Result<(), String> {
        match &s.kind {
            StmtKind::Decl(decl) => {
                if self.current_binding(&decl.name).is_some() {
                    return self.check_redefinition(decl);
                }
                if decl.ty == Ty::Auto {
                    let got = self.expr(&decl.value)?;
                    if got == Ty::Void {
                        return Err(format!(
                            "'auto' variable '{}' cannot have type 'void'",
                            decl.name
                        ));
                    }
                    if got == Ty::Any {
                        return Err(format!(
                            "cannot infer a static type for 'auto' variable '{}'; use an explicit type",
                            decl.name
                        ));
                    }
                    let shape = if got == Ty::Table {
                        self.table_shape(&decl.value)
                    } else {
                        None
                    };
                    self.declare_inferred(&decl.name, got, !decl.is_const)?;
                    self.set_shape(&decl.name, shape);
                    return Ok(());
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
                let shape = if decl.ty == Ty::Table {
                    self.table_shape(&decl.value)
                } else {
                    None
                };
                self.declare(&decl.name, got, !decl.is_const)?;
                self.set_shape(&decl.name, shape);
                Ok(())
            }
            StmtKind::Assign(n, op, e) => {
                if n == "this" || n == "self" {
                    return Err(format!("cannot assign to '{n}'"));
                }
                if let Some(field_ty) = self.implicit_field(n) {
                    if op != "=" && !matches!(field_ty, Ty::Int | Ty::Float | Ty::Any) {
                        return Err(format!("compound assignment {op} requires a numeric field"));
                    }
                    let got = self.expr_as(e, &field_ty)?;
                    if got != field_ty {
                        return Err(format!(
                            "assignment to field '{n}' expects {field_ty:?}, got {got:?}"
                        ));
                    }
                    return Ok(());
                }
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
                    return Err(
                        "tuples can only be changed by whole-tuple or same-type element assignment"
                            .into(),
                    );
                }
                let retype = if op == "=" {
                    match e {
                        Expr::Cast(_, ty) => Some(ty.clone()),
                        _ => None,
                    }
                } else {
                    None
                };
                if binding.inferred && retype.is_some() {
                    return Err(format!("cannot change the type of 'auto' variable '{n}'"));
                }
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
                if op == "=" {
                    self.set_shape(n, None);
                }
                Ok(())
            }
            StmtKind::AssignMember(base, name, op, value) => {
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
            StmtKind::AssignIndex(base, index, op, value) => {
                let base_ty = self.expr(base)?;
                let index_ty = self.expr(index)?;
                let target_ty = match base_ty {
                    Ty::Table => {
                        if index_ty != Ty::Int && index_ty != Ty::Str {
                            return Err(format!(
                                "table index must be int or str, got {index_ty:?}"
                            ));
                        }
                        let got = self.expr(value)?;
                        if got == Ty::Void {
                            return Err("table values cannot have type 'void'".into());
                        }
                        if let Some(shape) = self.table_shape_of(base) {
                            let expected = match (index.as_ref(), &index_ty) {
                                (Expr::Str(key), Ty::Str) => shape.named.get(key),
                                (Expr::Int(index), Ty::Int)
                                    if *index > 0
                                        && (*index as usize) <= shape.positional.len() =>
                                {
                                    shape.positional.get(*index as usize - 1)
                                }
                                _ => None,
                            };
                            if let Some(expected) = expected {
                                if &got != expected {
                                    return Err(format!(
                                        "assignment to table value expects {expected:?}, got {got:?}"
                                    ));
                                }
                            }
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
                    Ty::Map(key_ty, value_ty) => {
                        if index_ty != *key_ty {
                            return Err(format!("table key expects {key_ty:?}, got {index_ty:?}"));
                        }
                        *value_ty
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
                    return Err(format!(
                        "compound assignment {op} requires a numeric target"
                    ));
                }
                let got = self.expr_as(value, &target_ty)?;
                if got != target_ty {
                    return Err(format!(
                        "index assignment expects {target_ty:?}, got {got:?}"
                    ));
                }
                Ok(())
            }
            StmtKind::Expr(e) => {
                self.expr(e)?;
                Ok(())
            }
            StmtKind::Block(body) => {
                self.push();
                for stmt in body {
                    self.stmt(stmt, ret)?;
                }
                self.pop();
                Ok(())
            }
            StmtKind::If(c, a, b) => {
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
            StmtKind::While(c, b) => {
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
            StmtKind::For {
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
            StmtKind::Return(e) => match (e, ret) {
                (None, Ty::Void) => Ok(()),
                (Some(x), r) if self.expr_as(x, r)? == *r => Ok(()),
                (None, _) => Err("return value required".into()),
                (Some(_), Ty::Void) => Err("void function cannot return a value".into()),
                (Some(x), r) => Err(format!("return expects {r:?}, got {:?}", self.expr(x)?)),
            },
        }
    }
}

fn check_callable(
    c: &mut Checker,
    label: &str,
    function: &Fun,
    class: Option<&str>,
    constructor: bool,
) -> Result<(), String> {
    c.position = Some((function.line, function.column));
    let previous_class = c.current_class.clone();
    let previous_constructor = c.current_constructor;
    let previous_super_call = c.allow_super_call;
    c.current_class = class.map(str::to_string);
    c.current_constructor = constructor;
    let derived_constructor = constructor
        && class.is_some_and(|name| {
            c.classes
                .get(name)
                .is_some_and(|info| info.parent.is_some())
        });
    c.allow_super_call = derived_constructor;
    if derived_constructor
        && !matches!(
            function.body.first(),
            Some(Stmt {
                kind: StmtKind::Expr(Expr::SuperCall(_)),
                ..
            })
        )
    {
        return Err(format!(
            "constructor for '{}' must call 'super(...)' as its first statement",
            class.unwrap_or_default()
        ));
    }
    let outer_types = c.type_snapshot();
    for name in c.unstable_globals.clone() {
        c.set_type(&name, Ty::Any);
    }
    c.function_depth += 1;
    c.push();
    if let Some(class_name) = class {
        c.declare("self", Ty::Struct(class_name.to_string()), false)?;
    }
    c.allow_super_call = false;
    for p in &function.params {
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
    c.allow_super_call = derived_constructor;
    for (index, statement) in function.body.iter().enumerate() {
        c.stmt(statement, &function.ret)?;
        if index == 0 {
            c.allow_super_call = false;
        }
    }
    if function.ret != Ty::Void && !block_returns(&function.body) {
        return Err(format!("{label} must return a value on every path"));
    }
    c.pop();
    c.function_depth -= 1;
    c.restore_types(&outer_types);
    c.current_class = previous_class;
    c.current_constructor = previous_constructor;
    c.allow_super_call = previous_super_call;
    Ok(())
}

fn collect_structs(program: &[Top]) -> Result<Structs, String> {
    let mut structs = HashMap::new();
    for top in program {
        let Top::Struct(definition) = top else {
            continue;
        };
        let mut fields = HashMap::new();
        for field in &definition.fields {
            if field.ty == Ty::Void {
                return Err(format!(
                    "field '{}.{}' cannot have type 'void' at line {}, column {}",
                    definition.name, field.name, definition.line, definition.column
                ));
            }
            if fields
                .insert(field.name.clone(), field.ty.clone())
                .is_some()
            {
                return Err(format!(
                    "duplicate field '{}.{}' at line {}, column {}",
                    definition.name, field.name, definition.line, definition.column
                ));
            }
        }
        if structs.insert(definition.name.clone(), fields).is_some() {
            return Err(format!(
                "duplicate struct '{}' at line {}, column {}",
                definition.name, definition.line, definition.column
            ));
        }
    }
    Ok(structs)
}

fn signature(params: &[Param], ret: Ty) -> FnSig {
    FnSig {
        args: params.iter().map(|param| param.ty.clone()).collect(),
        required: params
            .iter()
            .take_while(|param| param.default.is_none())
            .count(),
        ret,
        varargs: false,
    }
}

fn empty_signature() -> FnSig {
    FnSig {
        args: Vec::new(),
        required: 0,
        ret: Ty::Void,
        varargs: false,
    }
}

fn signatures_match(left: &FnSig, right: &FnSig) -> bool {
    left.args == right.args
        && left.required == right.required
        && left.ret == right.ret
        && left.varargs == right.varargs
}

fn effective_constructor(classes: &Classes, class_name: &str) -> FnSig {
    let mut current = Some(class_name);
    while let Some(name) = current {
        let Some(class) = classes.get(name) else {
            break;
        };
        if let Some(constructor) = &class.constructor {
            return constructor.clone();
        }
        current = class.parent.as_deref();
    }
    empty_signature()
}

fn class_method<'a>(classes: &'a Classes, class_name: &str, method: &str) -> Option<&'a FnSig> {
    let mut current = Some(class_name);
    while let Some(name) = current {
        let class = classes.get(name)?;
        if let Some(signature) = class.methods.get(method) {
            return Some(signature);
        }
        current = class.parent.as_deref();
    }
    None
}

fn class_field(classes: &Classes, class_name: &str, field: &str) -> Option<Ty> {
    let class = classes.get(class_name)?;
    class.fields.get(field).cloned()
}

fn collect_classes(program: &[Top]) -> Result<Classes, String> {
    let mut classes = HashMap::new();
    for top in program {
        let Top::Class(definition) = top else {
            continue;
        };
        let mut fields = HashMap::new();
        let mut members = HashSet::new();
        for field in &definition.fields {
            if field.ty == Ty::Void {
                return Err(format!(
                    "field '{}.{}' cannot have type 'void' at line {}, column {}",
                    definition.name, field.name, field.line, field.column
                ));
            }
            if !members.insert(field.name.clone()) {
                return Err(format!(
                    "duplicate member '{}.{}' at line {}, column {}",
                    definition.name, field.name, field.line, field.column
                ));
            }
            fields.insert(field.name.clone(), field.ty.clone());
        }
        let mut methods = HashMap::new();
        for method in &definition.methods {
            if !members.insert(method.name.clone()) {
                return Err(format!(
                    "duplicate member '{}.{}' at line {}, column {}",
                    definition.name, method.name, method.line, method.column
                ));
            }
            methods.insert(
                method.name.clone(),
                signature(&method.params, method.ret.clone()),
            );
        }
        let constructor = definition
            .constructor
            .as_ref()
            .map(|constructor| signature(&constructor.params, Ty::Void));
        if classes
            .insert(
                definition.name.clone(),
                ClassInfo {
                    parent: definition.parent.clone(),
                    own_fields: fields.clone(),
                    fields,
                    methods,
                    constructor,
                    line: definition.line,
                    column: definition.column,
                },
            )
            .is_some()
        {
            return Err(format!(
                "duplicate class '{}' at line {}, column {}",
                definition.name, definition.line, definition.column
            ));
        }
    }
    Ok(classes)
}

fn resolve_class_fields(
    name: &str,
    classes: &Classes,
    visiting: &mut HashSet<String>,
    resolved: &mut HashMap<String, HashMap<String, Ty>>,
) -> Result<HashMap<String, Ty>, String> {
    if let Some(fields) = resolved.get(name) {
        return Ok(fields.clone());
    }
    let Some(class) = classes.get(name) else {
        return Err(format!("unknown class '{name}'"));
    };
    if !visiting.insert(name.to_string()) {
        return Err(format!(
            "class inheritance cycle involving '{name}' at line {}, column {}",
            class.line, class.column
        ));
    }
    let line = class.line;
    let column = class.column;
    let parent = class.parent.clone();
    let own_fields = class.own_fields.clone();
    let mut fields = if let Some(parent) = &parent {
        if !classes.contains_key(parent) {
            return Err(format!(
                "class '{name}' extends unknown class '{parent}' at line {line}, column {column}"
            ));
        }
        resolve_class_fields(parent, classes, visiting, resolved)?
    } else {
        HashMap::new()
    };
    for (field_name, field_ty) in own_fields {
        if let Some(inherited) = fields.get(&field_name) {
            if inherited != &field_ty {
                return Err(format!(
                    "class '{name}' redefines inherited field '{field_name}' as {field_ty:?} at line {line}, column {column}; inherited type is {inherited:?}"
                ));
            }
        }
        fields.insert(field_name, field_ty);
    }
    visiting.remove(name);
    resolved.insert(name.to_string(), fields.clone());
    Ok(fields)
}

fn resolve_classes(classes: &mut Classes) -> Result<(), String> {
    let mut resolved = HashMap::new();
    let mut names = classes.keys().cloned().collect::<Vec<_>>();
    names.sort();
    for name in &names {
        let fields = resolve_class_fields(name, classes, &mut HashSet::new(), &mut resolved)?;
        if let Some(class) = classes.get_mut(name) {
            class.fields = fields;
        }
    }
    for name in names {
        let Some(class) = classes.get(&name) else {
            continue;
        };
        let line = class.line;
        let column = class.column;
        let parent = class.parent.clone();
        let own_fields = class.own_fields.clone();
        let methods = class.methods.clone();
        let Some(parent_name) = parent else {
            continue;
        };
        for field_name in own_fields.keys() {
            if class_method(classes, &parent_name, field_name).is_some() {
                return Err(format!(
                    "field '{name}.{field_name}' conflicts with a method of the same name at line {line}, column {column}"
                ));
            }
        }
        for (method_name, method_signature) in &methods {
            if class_field(classes, &parent_name, method_name).is_some() {
                return Err(format!(
                    "method '{name}.{method_name}' conflicts with an inherited field of the same name at line {line}, column {column}"
                ));
            }
            if let Some(inherited) = class_method(classes, &parent_name, method_name) {
                if !signatures_match(inherited, method_signature) {
                    return Err(format!(
                        "method '{name}.{method_name}' overrides an inherited method with a different signature at line {line}, column {column}"
                    ));
                }
            }
        }
    }
    Ok(())
}

fn collect_types(program: &[Top]) -> Result<(Structs, Classes), String> {
    let mut structs = collect_structs(program)?;
    let mut classes = collect_classes(program)?;
    resolve_classes(&mut classes)?;
    for (name, class) in &classes {
        if structs.insert(name.clone(), class.fields.clone()).is_some() {
            return Err(format!(
                "class '{name}' conflicts with a struct of the same name"
            ));
        }
    }
    for fields in structs.values() {
        for ty in fields.values() {
            validate_type(ty, &structs)?;
        }
    }
    Ok((structs, classes))
}

fn validate_type(ty: &Ty, structs: &Structs) -> Result<(), String> {
    match ty {
        Ty::Struct(name) if !structs.contains_key(name) => {
            Err(format!("unknown struct type '{name}'"))
        }
        Ty::Module(path) if path.is_empty() => {
            Err("'module' is only valid in a declaration initialized from require(\"path\")".into())
        }
        Ty::Auto => {
            Err("'auto' is only valid in a variable declaration with an initializer".into())
        }
        Ty::Map(key, value) => {
            if !matches!(key.as_ref(), Ty::Str | Ty::Int) {
                return Err("table keys must be 'str' or 'int'".into());
            }
            if value.as_ref() == &Ty::Void {
                return Err("table values cannot have type 'void'".into());
            }
            validate_type(value, structs)
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

fn validate_program_types(program: &[Top], structs: &Structs) -> Result<(), String> {
    fn validate_stmts(
        statements: &[Stmt],
        structs: &HashMap<String, HashMap<String, Ty>>,
    ) -> Result<(), String> {
        for statement in statements {
            let result = match &statement.kind {
                StmtKind::Decl(decl) => {
                    if decl.ty == Ty::Auto {
                        Ok(())
                    } else {
                        validate_type(&decl.ty, structs)
                    }
                }
                StmtKind::Block(body) | StmtKind::While(_, body) => validate_stmts(body, structs),
                StmtKind::If(_, yes, no) => {
                    validate_stmts(yes, structs)?;
                    validate_stmts(no, structs)
                }
                StmtKind::For { body, .. } => validate_stmts(body, structs),
                _ => Ok(()),
            };
            result.map_err(|message| {
                format!(
                    "{message} at line {}, column {}",
                    statement.line, statement.column
                )
            })?;
        }
        Ok(())
    }

    for top in program {
        match top {
            Top::Fun(function) => {
                let result = (|| {
                    validate_type(&function.ret, structs)?;
                    for param in &function.params {
                        validate_type(&param.ty, structs)?;
                    }
                    validate_stmts(&function.body, structs)
                })();
                result.map_err(|message| {
                    format!(
                        "{message} at line {}, column {}",
                        function.line, function.column
                    )
                })?;
            }
            Top::Stmt(statement) => validate_stmts(std::slice::from_ref(statement), structs)?,
            Top::Struct(StructDef { .. }) => {}
            Top::Class(class) => {
                let result = (|| {
                    for field in &class.fields {
                        validate_type(&field.ty, structs)?;
                    }
                    for callable in class.methods.iter().chain(class.constructor.iter()) {
                        validate_type(&callable.ret, structs)?;
                        for param in &callable.params {
                            validate_type(&param.ty, structs)?;
                        }
                    }
                    Ok(())
                })();
                result.map_err(|message: String| {
                    format!("{message} at line {}, column {}", class.line, class.column)
                })?;
            }
            Top::Import { .. } => {}
        }
    }
    Ok(())
}

fn stmt_returns(statement: &Stmt) -> bool {
    match &statement.kind {
        StmtKind::Return(_) => true,
        StmtKind::Block(body) => block_returns(body),
        StmtKind::If(_, yes, no) => block_returns(yes) && block_returns(no),
        StmtKind::While(Expr::Bool(true), _) => true,
        _ => false,
    }
}

fn block_returns(statements: &[Stmt]) -> bool {
    statements.iter().any(stmt_returns)
}

fn collect_imports(program: &[Top]) -> Result<HashSet<String>, String> {
    let mut imports = HashSet::new();
    for top in program {
        let Top::Import {
            packages,
            line,
            column,
        } = top
        else {
            continue;
        };
        for package in packages {
            if package != "base" && stdlib::package(package).is_none() {
                return Err(format!(
                    "Unrecognised package '{package}' at line {line}, column {column}"
                ));
            }
            imports.insert(package.clone());
        }
    }
    Ok(imports)
}

pub(crate) fn check(
    program: &[Top],
    modules: &HashMap<String, ModuleExports>,
) -> Result<(), String> {
    let (structs, classes) = collect_types(program)?;
    validate_program_types(program, &structs)?;
    let unstable_globals = retyped_global_names(program);
    let imports = collect_imports(program)?;
    let mut c = Checker::new(
        modules.clone(),
        structs,
        classes,
        unstable_globals,
        imports.clone(),
    );
    c.funcs.insert(
        "print".into(),
        FnSig {
            args: vec![],
            required: 0,
            ret: Ty::Void,
            varargs: true,
        },
    );
    c.funcs.insert(
        "printf".into(),
        FnSig {
            args: vec![Ty::Str],
            required: 1,
            ret: Ty::Void,
            varargs: false,
        },
    );
    for package in &imports {
        if package == "base" {
            continue;
        }
        let definition = stdlib::package(package)
            .expect("imported stdlib packages are validated by collect_imports");
        let mut items = HashMap::new();
        for signature in definition.functions {
            items.insert(
                signature.name.to_string(),
                Export::Fn(FnSig {
                    args: signature.args.to_vec(),
                    required: signature.required,
                    ret: signature.ret.clone(),
                    varargs: signature.varargs,
                }),
            );
        }
        for (name, ty) in definition.values {
            items.insert((*name).to_string(), Export::Var(ty.clone()));
        }
        c.modules.insert(
            package.clone(),
            ModuleExports {
                items,
                dynamic: true,
                members: Some(
                    definition
                        .members
                        .iter()
                        .map(|name| (*name).to_string())
                        .collect(),
                ),
            },
        );
        c.declare(package, Ty::Module(package.clone()), false)?;
    }
    let result = (|| -> Result<(), String> {
        let mut global_declarations = HashSet::new();
        for t in program {
            match t {
                Top::Stmt(Stmt {
                    kind: StmtKind::Decl(decl),
                    line,
                    column,
                }) => {
                    c.position = Some((*line, *column));
                    if global_declarations.insert(decl.name.clone()) {
                        if c.imports.contains(&decl.name) {
                            return Err(format!(
                                "cannot declare variable '{}': '{}' is an imported package",
                                decl.name, decl.name
                            ));
                        }
                        if c.classes.contains_key(&decl.name) {
                            return Err(format!(
                                "cannot declare variable '{}': a class with that name already exists",
                                decl.name
                            ));
                        }
                        let declared = if decl.ty == Ty::Auto {
                            Ty::Any
                        } else {
                            decl.ty.clone()
                        };
                        c.declare(&decl.name, declared, !decl.is_const)?;
                        if decl.ty == Ty::Auto {
                            c.mark_inferred(&decl.name);
                        }
                    }
                }
                _ => {}
            }
        }
        for t in program {
            if let Top::Fun(f) = t {
                if c.classes.contains_key(&f.name) {
                    return Err(format!(
                        "function '{}' conflicts with a class of the same name",
                        f.name
                    ));
                }
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
                        varargs: false,
                    },
                );
            }
        }
        let mut checked_globals = HashSet::new();
        for t in program {
            match t {
                Top::Stmt(Stmt {
                    kind: StmtKind::Decl(decl),
                    line,
                    column,
                }) => {
                    c.position = Some((*line, *column));
                    if checked_globals.insert(decl.name.clone()) {
                        if decl.ty == Ty::Auto {
                            let got = c.expr(&decl.value)?;
                            if got == Ty::Void {
                                return Err(format!(
                                    "'auto' variable '{}' cannot have type 'void'",
                                    decl.name
                                ));
                            }
                            if got == Ty::Any {
                                return Err(format!(
                                "cannot infer a static type for 'auto' variable '{}'; use an explicit type",
                                decl.name
                            ));
                            }
                            let shape = if got == Ty::Table {
                                c.table_shape(&decl.value)
                            } else {
                                None
                            };
                            c.set_type(&decl.name, got);
                            c.set_shape(&decl.name, shape);
                            continue;
                        }
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
                        let shape = if decl.ty == Ty::Table {
                            c.table_shape(&decl.value)
                        } else {
                            None
                        };
                        c.set_type(&decl.name, got);
                        c.set_shape(&decl.name, shape);
                    } else {
                        c.check_redefinition(decl)?;
                    }
                }
                Top::Stmt(s) => c.stmt(s, &Ty::Void)?,
                Top::Fun(_) | Top::Struct(_) | Top::Class(_) => {}
                Top::Import { .. } => {}
            }
        }
        for t in program {
            let Top::Fun(f) = t else {
                continue;
            };
            check_callable(&mut c, &format!("function '{}'", f.name), f, None, false)?;
        }
        for t in program {
            let Top::Class(class) = t else {
                continue;
            };
            for field in &class.fields {
                c.position = Some((field.line, field.column));
                let got = c.expr_as(&field.default, &field.ty)?;
                if got != field.ty {
                    return Err(format!(
                        "field '{}.{}' expects {:?}, got {:?}",
                        class.name, field.name, field.ty, got
                    ));
                }
            }
            if let Some(constructor) = &class.constructor {
                check_callable(
                    &mut c,
                    &format!("constructor '{}'", class.name),
                    constructor,
                    Some(&class.name),
                    true,
                )?;
            }
            for method in &class.methods {
                check_callable(
                    &mut c,
                    &format!("method '{}.{}'", class.name, method.name),
                    method,
                    Some(&class.name),
                    false,
                )?;
            }
        }
        Ok(())
    })();
    result.map_err(|message| c.at(message))
}
