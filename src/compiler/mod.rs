mod ast;
mod checker;
mod codegen;
mod lexer;
mod modules;
mod parser;
mod stdlib;
mod warnings;

use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

fn extract_location(message: &str) -> (Option<usize>, Option<usize>) {
    let Some((_, suffix)) = message.rsplit_once(" at line ") else {
        return (None, None);
    };
    let Some((line, column)) = suffix.split_once(", column ") else {
        return (None, None);
    };
    match (line.parse(), column.parse()) {
        (Ok(line), Ok(column)) => (Some(line), Some(column)),
        _ => (None, None),
    }
}

impl Diagnostic {
    fn error(message: impl Into<String>) -> Self {
        let message = message.into();
        let (line, column) = extract_location(&message);
        Self {
            severity: Severity::Error,
            message,
            line,
            column,
        }
    }

    fn warning(message: impl Into<String>) -> Self {
        let message = message.into();
        let (line, column) = extract_location(&message);
        Self {
            severity: Severity::Warning,
            message,
            line,
            column,
        }
    }
}

pub fn compile(source: &str) -> Result<String, Vec<Diagnostic>> {
    compile_with_dir(source, Path::new("."))
}

pub fn compile_with_dir(source: &str, base: &Path) -> Result<String, Vec<Diagnostic>> {
    let tokens = lexer::lex(source).map_err(|message| vec![Diagnostic::error(message)])?;
    let program = parser::parse(tokens, source).map_err(|messages| {
        messages
            .into_iter()
            .map(Diagnostic::error)
            .collect::<Vec<_>>()
    })?;

    let loaded =
        modules::load(&program, base).map_err(|message| vec![Diagnostic::error(message)])?;

    let mut errors: Vec<Diagnostic> = Vec::new();
    for (id, module_program) in &loaded.programs {
        if let Err(message) = checker::check(module_program, &loaded.exports) {
            errors.push(Diagnostic::error(format!("in module '{id}': {message}")));
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    if let Err(message) = checker::check(&program, &loaded.exports) {
        return Err(vec![Diagnostic::error(message)]);
    }

    let mut warnings: Vec<Diagnostic> = Vec::new();
    for (id, module_program) in &loaded.programs {
        for message in warnings::check(module_program) {
            warnings.push(Diagnostic::warning(format!("in module '{id}': {message}")));
        }
    }
    for message in warnings::check(&program) {
        warnings.push(Diagnostic::warning(message));
    }
    if !warnings.is_empty() {
        return Err(warnings);
    }

    Ok(codegen::generate(&program, &loaded.programs))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn error_message(source: &str) -> String {
        compile(source)
            .unwrap_err()
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn transpiles() {
        let s = "int x = 2; if (x > 1) { print(\"yes\"); }";
        let lua = compile(s).unwrap();
        assert!(lua.contains("x = 2"));
        assert!(lua.contains("if (x > 1) then"));
    }

    #[test]
    fn catches_type_mismatch() {
        assert!(error_message("int x = \"no\";").contains("initializer"));
    }

    #[test]
    fn function() {
        let s = "fn add(int a, int b) -> int { return a + b; } int x = add(2, 3);";
        assert!(compile(s).unwrap().contains("function add(a, b)"));
    }

    #[test]
    fn const_declarations_are_immutable() {
        let source = r#"
            const int maxLives = 3;

            fn max_lives() -> int {
                const int localMax = maxLives;
                return localMax;
            }

            int currentMax = max_lives();
        "#;
        let lua = compile(source).unwrap();
        assert!(lua.contains("maxLives = 3"));
        assert!(lua.contains("local localMax = maxLives"));

        assert!(error_message("const int limit = 10; limit = 11;")
            .contains("cannot assign to constant 'limit'"));
        assert!(error_message("const int limit = 10; limit += 1;")
            .contains("cannot assign to constant 'limit'"));
        assert!(error_message("const int limit = \"wrong type\";").contains("initializer is Str"));
    }

    #[test]
    fn supports_explicit_locals_and_standalone_scopes() {
        let source = r#"
            (local) (const) int maxLives = 3;

            fn max_lives() -> int {
                return maxLives;
            }

            {
                int temporary = max_lives();
                print(temporary);
            }
        "#;
        let lua = compile(source).unwrap();
        assert!(lua.contains("local maxLives\n"));
        assert!(lua.contains("maxLives = 3"));
        assert!(lua.contains("function max_lives"));
        assert!(lua.contains("do -- tlua:"));
        assert!(lua.contains("local temporary = max_lives()"));
        assert!(lua.contains("end\n"));

        let err = error_message("{ int scoped = 1; } print(scoped);");
        assert!(err.contains("unknown variable 'scoped'"));
    }

    #[test]
    fn concatenates_strings_with_plus_and_comma() {
        let source = r#"
            str first = "Hello";
            str second = " world";
            str joined_plus = first + second;
            str joined_comma = first, second;
            int sum = 1 + 2;

            fn join(str left, str right) -> str {
                return left + right;
            }
        "#;
        let lua = compile(source).unwrap();

        assert!(lua.contains("(first .. second)"));
        assert!(lua.contains("(left .. right)"));
        assert!(lua.contains("(1 + 2)"));
        assert!(!lua.contains("(first + second)"));

        let nested = compile("print((\"hello\", \" world\"));").unwrap();
        assert!(nested.contains("print((\"hello\" .. \" world\"))"));
    }

    #[test]
    fn numeric_for_and_printf() {
        let lua = compile(r#"for (i, 10) { printf("Hello world! {i}"); }"#).unwrap();
        assert!(lua.contains("for i = 0, 10, 1 do"));
        assert!(lua.contains("print(\"Hello world! \" .. tostring(i))"));

        let lua =
            compile("for (j, 2, 10, 2) { print(j); } for (k, 10, 0, -2) { print(k); }").unwrap();
        assert!(lua.contains("for j = 2, 10, 2 do"));
        assert!(lua.contains("for k = 10, 0, (-2) do"));
    }

    #[test]
    fn supports_type_and_lua_comments() {
        let source = r#"
            // line comment
            /* C-style
               block comment */
            -- Lua line comment
            --[[ Lua long comment ]]
            --[=[ Lua long comment with a level-one delimiter ]=]
            int answer = 42;
        "#;
        assert!(compile(source).unwrap().contains("answer = 42"));
    }

    #[test]
    fn missing_semicolon_is_an_error() {
        let diagnostics = compile("int x = 1").unwrap_err();
        assert_eq!(diagnostics[0].severity, Severity::Error);
        assert!(diagnostics[0].message.contains("expected ';'"));
    }

    #[test]
    fn warnings_are_fatal() {
        let diagnostics = compile("fn f() -> int { int unused = 1; return 0; }").unwrap_err();
        assert_eq!(diagnostics[0].severity, Severity::Warning);
        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("unused variable 'unused'")));
    }

    #[test]
    fn warns_on_shadowing_and_unreachable_code() {
        let diagnostics =
            compile("int x = 1; fn f() -> int { int x = 2; return x; print(x); }").unwrap_err();
        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("shadows an outer declaration")));
        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("unreachable statement after return")));
    }

    #[test]
    fn uses_printf_placeholders_count_as_use() {
        let source = "fn f() -> int { int n = 7; printf(\"{n}\"); return n; }";
        assert!(compile(source).is_ok());
    }

    #[test]
    fn unused_parameters_warn() {
        let diagnostics = compile("fn f(int unused) -> int { return 0; }").unwrap_err();
        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("unused variable 'unused'")));
    }

    fn with_modules(files: &[(&str, &str)], main_src: &str) -> Result<String, Vec<Diagnostic>> {
        use std::fs;
        use std::path::PathBuf;
        use std::sync::atomic::{AtomicUsize, Ordering};
        static TEST_DIR: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "typelua-test-{}",
            TEST_DIR.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for (name, src) in files {
            let path = PathBuf::from(&dir).join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, src).unwrap();
        }
        let result = compile_with_dir(main_src, &dir);
        let _ = fs::remove_dir_all(&dir);
        result
    }

    #[test]
    fn requires_typed_modules() {
        let lua = with_modules(
            &[(
                "greetings.tlua",
                "fn hello(str name) -> str { return \"hi \" + name; }",
            )],
            "local greetings = require(\"greetings\");\nstr msg = greetings.hello(\"lua\");\nprint(msg);",
        )
        .unwrap();
        assert!(lua.contains("package.preload[\"greetings\"]"));
        assert!(lua.contains("local greetings"));
        assert!(lua.contains("greetings = require(\"greetings\")"));
        assert!(lua.contains("greetings.hello(\"lua\")"));
    }

    #[test]
    fn supports_lua_require_forms() {
        let files = &[(
            "greetings.tlua",
            "fn hello(str name) -> str { return \"hi \" + name; }",
        )];
        let no_parens = with_modules(
            files,
            "local greetings = require \"greetings\";\nprint(greetings.hello('lua'));",
        )
        .unwrap();
        assert!(no_parens.contains("greetings = require(\"greetings\")"));

        let inline = with_modules(files, "print(require(\"greetings\").hello(\"lua\"));").unwrap();
        assert!(inline.contains("require(\"greetings\").hello(\"lua\")"));
    }

    #[test]
    fn require_statement_has_no_implicit_binding() {
        let diagnostics = with_modules(
            &[("greetings.tlua", "fn hello() -> str { return \"hi\"; }")],
            "require(\"greetings\");\nprint(greetings.hello());",
        )
        .unwrap_err();
        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("unknown variable 'greetings'")));
    }

    #[test]
    fn standalone_require_still_compiles_module() {
        let lua = with_modules(
            &[("greetings.tlua", "fn hello() -> str { return \"hi\"; }")],
            "require(\"greetings\");",
        )
        .unwrap();
        assert!(lua.contains("package.preload[\"greetings\"]"));
        assert!(lua.contains("require(\"greetings\")"));
    }

    #[test]
    fn module_assignment_form_declares_namespace() {
        let lua = with_modules(
            &[("inspect.tlua", "fn show() -> void { print(\"x\"); }")],
            "inspect = require(\"inspect\");\ninspect.show();",
        )
        .unwrap();
        assert!(lua.contains("inspect = require(\"inspect\")"));
    }

    #[test]
    fn dotted_module_names_resolve_like_lua() {
        let lua = with_modules(
            &[(
                "pkg/greetings.tlua",
                "fn hello(str name) -> str { return \"hi \" + name; }",
            )],
            "local greetings = require(\"pkg.greetings\");\nprint(greetings.hello(\"lua\"));",
        )
        .unwrap();
        assert!(lua.contains("package.preload[\"pkg.greetings\"]"));
    }

    #[test]
    fn slash_paths_with_explicit_extensions_resolve() {
        let files = &[(
            "modules/greetings.tlua",
            "fn hello(str name) -> str { return \"hi \" + name; }",
        )];
        let lua = with_modules(
            files,
            "module greetings = require(\"modules/greetings.tlua\");\nprint(greetings.hello(\"lua\"));",
        )
        .unwrap();
        assert!(lua.contains("package.preload[\"modules/greetings.tlua\"]"));
        assert!(lua.contains("greetings = require(\"modules/greetings.tlua\")"));

        let lua = with_modules(
            files,
            "module greetings = require(\"modules/greetings\");\nprint(greetings.hello(\"lua\"));",
        )
        .unwrap();
        assert!(lua.contains("package.preload[\"modules/greetings\"]"));
    }

    #[test]
    fn module_declarations_use_the_module_type() {
        let lua = with_modules(
            &[(
                "greetings.tlua",
                "fn hello(str name) -> str { return \"hi \" + name; }",
            )],
            "module greetings = require(\"greetings.tlua\");\nprint(greetings.hello(\"lua\"));",
        )
        .unwrap();
        assert!(lua.contains("greetings = require(\"greetings.tlua\")"));
    }

    #[test]
    fn module_declarations_require_a_require_call() {
        let error = error_message("module m = 1;");
        assert!(error.contains("'module' declaration must be initialized"));
        let error = error_message("fn f(module m) -> void {}");
        assert!(error.contains("'module' is only valid"));
    }

    #[test]
    fn explicit_lua_paths_are_imported_at_runtime() {
        let lua = with_modules(
            &[("helper.lua", "return { answer = function() return 42 end }")],
            "module helper = require(\"helper.lua\");\nint answer = helper.answer() as int;",
        )
        .unwrap();
        assert!(!lua.contains("package.preload[\"helper.lua\"]"));
        assert!(lua.contains("helper = require(\"helper.lua\")"));
    }

    #[test]
    fn missing_explicit_modules_error() {
        let diagnostics = with_modules(&[], "module nope = require(\"nope.tlua\");").unwrap_err();
        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("cannot find module 'nope.tlua'")));
    }

    #[test]
    fn syntax_errors_describe_what_was_found() {
        assert!(error_message("int x = ;").contains("expected expression, found ';'"));
        assert!(error_message("local int x 1;").contains("expected '=', found number '1'"));
        assert!(error_message("fn f(list) -> void {}").contains("expected type name, found ')'"));
        assert!(error_message("fn f(int) -> void {}").contains("expected identifier, found ')'"));
    }

    #[test]
    fn strings_concatenate_with_dot_dot() {
        let lua = compile(r#"str name = "an" .. "dy";"#).unwrap();
        assert!(lua.contains(r#"name = ("an" .. "dy")"#));
        assert!(error_message("int x = 1 .. 2;").contains("operator .. cannot combine Int and Int"));
        assert!(error_message(r#"str s = "a" .. 1;"#)
            .contains("operator .. cannot combine Str and Int"));
    }

    #[test]
    fn auto_infers_a_locked_type() {
        let lua = compile("auto count = 2 + 3; count += 1; print(count);").unwrap();
        assert!(lua.contains("count = (2 + 3)"));
        assert!(compile("auto x = 1; x = 2;").is_ok());
        assert!(error_message(r#"auto count = 1; count = "two" as str;"#)
            .contains("cannot change the type of 'auto' variable 'count'"));
        assert!(error_message("auto count = 1; auto count = 2;")
            .contains("variable 'count' was declared with 'auto' and cannot be redefined"));
        assert!(compile(r#"import string; auto x = string.upper("a"); x = "b";"#).is_ok());
        assert!(
            error_message(r#"import string; auto x = string.upper("a"); x = 1 as int;"#)
                .contains("cannot change the type of 'auto' variable 'x'")
        );
        assert!(error_message("fn f(auto x) -> int { return x; }").contains("'auto' is only valid"));
        assert!(error_message("fn f() -> auto { return 1; }").contains("'auto' is only valid"));
    }

    #[test]
    fn stdlib_packages_require_imports() {
        assert!(error_message(r#"str s = string.upper("a");"#)
            .contains("Unrecognised package 'string': import it with 'import string;'"));
        assert!(error_message("import nope;").contains("Unrecognised package 'nope'"));
        assert!(error_message("str b = tostring(1);").contains(
            "Unrecognised package 'base': import it with 'import base;' to use 'tostring'"
        ));
        let lua = compile(r#"import string; str s = string.upper("a") as str; print(s);"#).unwrap();
        assert!(lua.contains(r#"s = string.upper("a")"#));
        assert!(!lua.contains("import string"));
        assert!(compile("import base; str b = tostring(1) as str;").is_ok());
        assert!(compile("import string, math;").is_ok());
        assert!(error_message(r#"import string; str s = string.nope("a");"#)
            .contains("package 'string' has no member 'nope'"));
    }

    #[test]
    fn non_void_functions_must_return_on_every_path() {
        assert!(compile("fn f() -> int { return 1; }").is_ok());
        assert!(compile("fn f() -> int { if (true) { return 1; } else { return 2; } }").is_ok());
        assert!(compile("fn f() -> int { { return 1; } }").is_ok());
        assert!(compile("fn f() -> int { while (true) { print(\"x\"); } }").is_ok());
        assert!(compile("fn f() -> void { print(\"x\"); }").is_ok());
        assert!(error_message("fn f() -> int { }")
            .contains("function 'f' must return a value on every path"));
        assert!(error_message("fn f() -> int { if (true) { return 1; } }")
            .contains("function 'f' must return a value on every path"));
        assert!(
            error_message("fn f() -> int { while (false) { return 1; } }")
                .contains("function 'f' must return a value on every path")
        );
        assert!(
            error_message("fn f() -> int { for (i, 1, 3) { return i; } }")
                .contains("function 'f' must return a value on every path")
        );
    }

    #[test]
    fn stdlib_members_have_static_types() {
        assert!(compile(r#"import string; int n = string.len("abc");"#).is_ok());
        assert!(compile(r#"import string; str s = string.upper("a");"#).is_ok());
        assert!(compile(r#"import string; str s = string.upper("a") + "!";"#).is_ok());
        assert!(compile(r#"import math; float x = math.sqrt(4.0);"#).is_ok());
        assert!(compile("import math; float pi = math.pi;").is_ok());
        assert!(compile("import os; int t = os.time();").is_ok());
        assert!(compile(r#"import utf8; str p = utf8.charpattern;"#).is_ok());
        assert!(compile(r#"import base; str v = _VERSION;"#).is_ok());
        assert!(compile(r#"import base; int n = rawlen({});"#).is_ok());
        assert!(compile(r#"import base; str s = tostring(1) + "!";"#).is_ok());
        assert!(compile(r#"import string; str s = string.format("%d", 1);"#).is_ok());
        let lua = compile(r#"import base; str s = tostring(1) + "!";"#).unwrap();
        assert!(lua.contains(r#"tostring(1) .. "!""#));
        let lua = compile(r#"import string; str s = string.upper("a") + "!";"#).unwrap();
        assert!(lua.contains(r#"string.upper("a") .. "!""#));
        let lua = compile("import math; float f = math.sqrt(4.0) + 1.0;").unwrap();
        assert!(lua.contains("math.sqrt(4) + 1"));
        assert!(
            error_message(r#"import string; int n = string.upper("a");"#)
                .contains("declared Int, initializer is Str")
        );
        assert!(error_message(r#"import string; str s = string.upper(1);"#)
            .contains("argument to 'string.upper' expects Str, got Int"));
        assert!(error_message("import string; str s = string.upper();")
            .contains("'string.upper' expects 1..1 arguments, got 0"));
        assert!(error_message("import string; str s = string.nope(\"a\");")
            .contains("package 'string' has no member 'nope'"));
    }

    #[test]
    fn generic_table_reads_use_known_literal_shapes() {
        assert!(compile(
            r#"table t = { count = 1, label = "hi" }; int n = t.count; str s = t.label + "!";"#
        )
        .is_ok());
        assert!(compile(r#"table t = { 1, 2, 3 }; int n = t[1];"#).is_ok());
        assert!(compile(r#"auto t = { label = "hi" }; str s = t.label;"#).is_ok());
        assert!(compile(r#"table t = { count = 1 }; print(t.other);"#).is_ok());
        assert!(compile(r#"table t = { count = 1 }; t.other = "x";"#).is_ok());
        let lua = compile(r#"table t = { label = "a" }; str s = t.label + "!";"#).unwrap();
        assert!(lua.contains(r#"t.label .. "!""#));
        let lua = compile(r#"auto t = { "a" }; str s = t[1] + "!";"#).unwrap();
        assert!(lua.contains(r#"t[1] .. "!""#));
        assert!(
            error_message(r#"table t = { count = 1 }; str s = t.count;"#)
                .contains("declared Str, initializer is Int")
        );
        assert!(error_message(r#"table t = { 1, 2 }; str s = t[2];"#)
            .contains("declared Str, initializer is Int"));
        assert!(error_message(r#"table t = { count = 1 }; t.count = "x";"#)
            .contains("assignment to field 'count' expects Int, got Str"));
        assert!(error_message(r#"table t = { 1 }; t[1] = "x";"#)
            .contains("assignment to table value expects Int, got Str"));
        assert!(
            error_message(r#"table t = { count = 1 }; t = { count = 2 }; int n = t.count;"#)
                .contains("cannot use a dynamically typed value")
        );
    }

    #[test]
    fn imports_are_top_level_only() {
        assert!(error_message("fn f() -> void { import string; }")
            .contains("'import' is only allowed at the top level"));
    }

    #[test]
    fn typed_tables_check_keys_and_values() {
        let source = r#"
            table str int ages = { alice = 21, bob = 32 };
            ages["carol"] = 17;
            ages.dave = 4;
            int bob = ages.bob;

            table int str names = { "zero", "one" };
            str one = names[1];

            table str str tags = { lang = "lua" };
            str tag = tags.lang + "!";
        "#;
        let lua = compile(source).unwrap();
        assert!(lua.contains("ages = { alice = 21, bob = 32 }"));
        assert!(lua.contains(r#"names = { "zero", "one" }"#));
        assert!(lua.contains(r#"(tags.lang .. "!")"#));

        assert!(error_message(r#"table str int ages = { alice = "old" };"#)
            .contains("table value for key 'alice' expects Int, got Str"));
        assert!(
            error_message("table str int ages = { alice = 1, alice = 2 };")
                .contains("table key 'alice' is initialized more than once")
        );
        assert!(error_message(r#"table int str names = { zero = "x" };"#)
            .contains("tables with int keys do not accept named field 'zero'"));
        assert!(error_message("table str int ages = { 1 };")
            .contains("tables with str keys require named fields"));
        assert!(
            error_message("table float int t = {};").contains("table keys must be 'str' or 'int'")
        );
        assert!(error_message("table str void t = {};")
            .contains("table values cannot have type 'void'"));

        assert!(
            error_message("table str int ages = { alice = 1 }; int x = ages[1];")
                .contains("table key expects Str, got Int")
        );
        assert!(
            error_message(r#"table str int ages = { alice = 1 }; ages.alice = "old";"#)
                .contains("assignment to field 'alice' expects Int, got Str")
        );
        assert!(
            error_message(r#"table str int ages = { alice = 1 }; ages = { bob = "old" };"#)
                .contains("table value for key 'bob' expects Int, got Str")
        );
    }

    #[test]
    fn collections_transpile_to_lua_tables() {
        let source = r#"
            struct Point { float x; float y; }

            list int values = [4, 8, 15];
            tuple position = (0, 10, 11);
            dict player = { name = "Andy", age = 15 };
            table config = { retries = 3 };
            Point origin = { x = 0.5, y = 1.5 };
            table str int ages = { alice = 21 };
            table int str names = { "zero", "one" };
        "#;
        let lua = compile(source).unwrap();
        for expected in [
            "values = { 4, 8, 15 }",
            "position = { 0, 10, 11 }",
            r#"player = { name = "Andy", age = 15 }"#,
            "config = { retries = 3 }",
            "origin = { x = 0.5, y = 1.5 }",
            "ages = { alice = 21 }",
            r#"names = { "zero", "one" }"#,
        ] {
            assert!(lua.contains(expected), "expected {expected:?} in:\n{lua}");
        }
    }

    #[test]
    fn compiles_classes_with_fn_methods_and_self() {
        let lua = compile(
            r#"
            class hero {
                str name = "Ada";
                int health = 100;
                constructor(str name) {
                    self.name = name;
                }
                fn isAlive() -> bool {
                    return health > 0;
                }
                fn damage(int amount) {
                    health -= amount;
                }
                fn greeting() -> str {
                    return "hi " + name;
                }
            }
            class Hero = new hero("Ada");
            print(Hero.isAlive());
            Hero.damage(200);
            print(Hero.isAlive());
            "#,
        )
        .unwrap();
        for expected in [
            "hero = {}",
            "hero.__index = hero",
            "setmetatable({}, hero)",
            "function hero.new(name)",
            "function hero:isAlive()",
            "function hero:damage(amount)",
            r#"Hero = hero.new("Ada")"#,
            "Hero:isAlive()",
            "Hero:damage(200)",
            "self.health = self.health - amount",
            r#"("hi " .. self.name)"#,
        ] {
            assert!(lua.contains(expected), "expected {expected:?} in:\n{lua}");
        }
    }

    #[test]
    fn compiles_inheritance_with_super_and_metatables() {
        let lua = compile(
            r#"
            class Animal {
                str name = "animal";
                constructor(str name) { self.name = name; }
                fn speak() -> str { return "..."; }
            }
            class Dog extends Animal {
                constructor(str name) { super(name); }
                fn speak() -> str { return "Woof"; }
                fn parent_speak() -> str { return super.speak(); }
            }
            local class pet = new Dog("Rex");
            print(pet.speak());
            print(pet.parent_speak());
            "#,
        )
        .unwrap();
        for expected in [
            "Dog = setmetatable({}, { __index = Animal })",
            "Dog.__index = Dog",
            "function Dog.new(name)",
            "function Dog:__init(name)",
            "local super = Animal",
            "super.__init(self, name)",
            "function Dog:speak()",
            "return super.speak(self)",
            r#"pet = Dog.new("Rex")"#,
            "pet:speak()",
            "pet:parent_speak()",
        ] {
            assert!(lua.contains(expected), "expected {expected:?} in:\n{lua}");
        }
    }

    #[test]
    fn emits_parent_classes_before_children() {
        let lua = compile(
            r#"
            class Child extends Parent {
                constructor() { super(); }
            }
            class Parent {
                constructor() {}
            }
            local class item = new Child;
            "#,
        )
        .unwrap();
        let parent = lua.find("Parent = {}").unwrap();
        let child = lua
            .find("Child = setmetatable({}, { __index = Parent })")
            .unwrap();
        assert!(parent < child, "expected parent before child in:\n{lua}");
    }

    #[test]
    fn derived_classes_inherit_constructors_and_fields() {
        let lua = compile(
            r#"
            class Animal {
                str name = "animal";
                constructor(str name) { self.name = name; }
            }
            class Dog extends Animal {
                str breed = "mutt";
            }
            local class pet = new Dog("Rex");
            print(pet.name, pet.breed);
            "#,
        )
        .unwrap();
        assert!(lua.contains("function Dog.new(name)"));
        assert!(lua.contains("super.__init(self, name)"));
        assert!(lua.contains(r#"self.breed = "mutt""#));
    }

    #[test]
    fn rejects_invalid_inheritance() {
        assert!(error_message(
            "class A { constructor() {} }\nclass B extends A { constructor() {} }"
        )
        .contains("must call 'super(...)' as its first statement"));
        assert!(
            error_message("class A { constructor() { super(); } }").contains("has no base class")
        );
        assert!(
            error_message(
                "class A { fn value() -> int { return 1; } }\nclass B extends A { fn value() -> str { return \"x\"; } }"
            )
            .contains("different signature")
        );
        assert!(error_message("class B extends Missing {}").contains("extends unknown class"));
        assert!(error_message("class A extends B {}\nclass B extends A {}")
            .contains("inheritance cycle"));
        assert!(
            error_message("class A { int hp = 1; }\nclass B extends A { str hp = \"x\"; }")
                .contains("redefines inherited field")
        );
        assert!(
            error_message("class A { fn hp() {} }\nclass B extends A { int hp = 1; }")
                .contains("conflicts with a method")
        );
        assert!(
            error_message(
                "class A { fn value() -> int { return 1; } }\nclass B extends A { fn call() { super.missing(); } }"
            )
            .contains("has no method 'missing'")
        );
    }

    #[test]
    fn accepts_subclasses_where_a_parent_class_is_expected() {
        let lua = compile(
            r#"
            class Animal {
                fn kind() -> str { return "animal"; }
            }
            class Dog extends Animal {
                fn kind() -> str { return "dog"; }
            }
            fn describe(Animal animal) -> str { return animal.kind(); }
            Animal pet = new Dog;
            print(describe(new Dog));
            "#,
        )
        .unwrap();
        assert!(lua.contains("pet = Dog.new()"));
        assert!(lua.contains("describe(Dog.new())"));
    }

    #[test]
    fn classes_require_new_initializers() {
        assert!(error_message("class Hero = 1;").contains("new ClassName"));
        assert!(error_message("class Hero = new missing;").contains("unknown struct type"));
        assert!(
            error_message("class hero { fn greet() {} }\nclass Hero = new hero(1);")
                .contains("expects 0")
        );
    }

    #[test]
    fn class_methods_require_fn_and_use_self() {
        assert!(error_message("class hero { greet() {} }").contains("expected identifier"));
        assert!(
            error_message("class hero { fn greet() -> int { return this.health; } }")
                .contains("unknown variable 'this'")
        );
        assert!(error_message("class hero { fn __init() {} }").contains("reserved for class"));
        assert!(error_message("class hero { fn super() {} }").contains("reserved"));
    }

    #[test]
    fn rejects_duplicate_class_members() {
        assert!(error_message("class hero {\n int hp = 1;\n fn hp() {}\n}")
            .contains("duplicate member"));
        assert!(error_message("class hero { fn a() {} fn a() {} }").contains("duplicate member"));
    }

    #[test]
    fn compiles_class_typed_parameters_and_returns() {
        let lua = compile(
            r#"
            class hero {
                int hp = 10;
                fn isAlive() -> bool { return self.hp > 0; }
            }
            fn alive(hero h) -> bool { return h.isAlive(); }
            local class Hero = new hero;
            print(alive(Hero));
            "#,
        )
        .unwrap();
        assert!(lua.contains("function alive(h)"));
        assert!(lua.contains("return h:isAlive()"));
    }

    #[test]
    fn class_instances_are_not_struct_literals() {
        assert!(
            error_message("class hero { int hp = 1; }\nhero h = { hp = 2 };")
                .contains("must be constructed with 'new hero'")
        );
    }

    #[test]
    fn lua_modules_are_imported_dynamically() {
        let lua = with_modules(
            &[(
                "plain.lua",
                "return { hi = function(name) return \"lua hi \" .. name end }",
            )],
            "local plain = require(\"plain\");\nstr msg = plain.hi(\"lua\") as str;\nprint(msg);",
        )
        .unwrap();
        assert!(!lua.contains("package.preload[\"plain\"]"));
        assert!(lua.contains("plain = require(\"plain\")"));
    }

    #[test]
    fn compiles_transitive_tlua_modules() {
        let lua = with_modules(
            &[
                ("a.tlua", "fn name() -> str { return \"a\"; }"),
                (
                    "b.tlua",
                    "local a = require(\"a\");\nfn name() -> str { return a.name() + \"b\"; }",
                ),
            ],
            "local b = require(\"b\");\nprint(b.name());",
        )
        .unwrap();
        assert!(lua.contains("package.preload[\"a\"]"));
        assert!(lua.contains("package.preload[\"b\"]"));
    }

    #[test]
    fn cyclic_imports_error() {
        let diagnostics = with_modules(
            &[("a.tlua", "require(\"b\");"), ("b.tlua", "require(\"a\");")],
            "require(\"a\");",
        )
        .unwrap_err();
        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("cyclic import")));
    }

    #[test]
    fn missing_relative_modules_error() {
        let diagnostics =
            with_modules(&[], "local nope = require(\"./nope\");\nprint(nope);").unwrap_err();
        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("cannot find module './nope'")));
    }

    #[test]
    fn missing_modules_are_dynamic() {
        let lua = with_modules(
            &[],
            "local nope = require(\"nope\");\nprint(nope.anything);",
        )
        .unwrap();
        assert!(lua.contains("require(\"nope\")"));
    }
}
