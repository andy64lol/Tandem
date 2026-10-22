mod ast;
mod checker;
mod codegen;
mod lexer;
mod modules;
mod parser;
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
}

impl Diagnostic {
    fn error(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
        }
    }

    fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
        }
    }
}

pub fn compile(source: &str) -> Result<String, Vec<Diagnostic>> {
    compile_with_dir(source, Path::new("."))
}

pub fn compile_with_dir(source: &str, base: &Path) -> Result<String, Vec<Diagnostic>> {
    let tokens = lexer::lex(source).map_err(|message| vec![Diagnostic::error(message)])?;
    let program = parser::parse(tokens).map_err(|message| vec![Diagnostic::error(message)])?;

    let loaded = modules::load(&program, base).map_err(|message| vec![Diagnostic::error(message)])?;

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
        assert!(lua.contains("local maxLives\nmaxLives = 3\nfunction max_lives"));
        assert!(lua.contains("do\n    local temporary = max_lives()"));
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

        let lua = compile("for (j, 2, 10, 2) { print(j); } for (k, 10, 0, -2) { print(k); }")
            .unwrap();
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
        assert!(diagnostics.iter().any(|d| d.message.contains("unused variable 'unused'")));
    }

    #[test]
    fn warns_on_shadowing_and_unreachable_code() {
        let diagnostics = compile(
            "int x = 1; fn f() -> int { int x = 2; return x; print(x); }",
        )
        .unwrap_err();
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
        let dir = std::env::temp_dir()
            .join(format!("typelua-test-{}", TEST_DIR.fetch_add(1, Ordering::SeqCst)));
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
            "require(\"greetings\");\nstr msg = greetings.hello(\"lua\");\nprint(msg);",
        )
        .unwrap();
        assert!(lua.contains("package.preload[\"greetings\"]"));
        assert!(lua.contains("local greetings = require(\"greetings\")"));
        assert!(lua.contains("greetings.hello(\"lua\")"));
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
    fn missing_modules_are_dynamic() {
        let lua = with_modules(&[], "require(\"nope\");\nprint(nope.anything);").unwrap();
        assert!(lua.contains("require(\"nope\")"));
    }
}
