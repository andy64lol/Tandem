mod ast;
mod checker;
mod codegen;
mod lexer;
mod parser;

pub fn compile(source: &str) -> Result<String, String> {
    let tokens = lexer::lex(source)?;
    let program = parser::parse(tokens)?;
    checker::check(&program)?;
    Ok(codegen::generate(&program))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transpiles() {
        let s = "int x = 2; if (x > 1) { print(\"yes\"); }";
        let lua = compile(s).unwrap();
        assert!(lua.contains("x = 2"));
        assert!(lua.contains("if (x > 1) then"));
    }
    #[test]
    fn catches_type_mismatch() {
        assert!(compile("int x = \"no\";")
            .unwrap_err()
            .contains("initializer"));
    }
    #[test]
    fn function() {
        let s = "fn add(int a, int b) -> int { return a + b; } int x = add(2, 3);";
        assert!(compile(s).unwrap().contains("function add(a, b)"));
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

        let lua = compile("for (j, 2, 10, 2) {} for (k, 10, 0, -2) {}").unwrap();
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
}
