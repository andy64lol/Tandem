# 11. Rust API

TypeLua uses [`mlua`](https://docs.rs/mlua/) with Lua 5.4 and its vendored Lua build. The Rust library exposes the compiler and runtime as separate modules:

- `typelua::compiler::compile(&str) -> Result<String, Vec<Diagnostic>>` parses and type-checks TypeLua source, then returns generated Lua. On any diagnostic it returns the diagnostics instead; because warnings are fatal, a single warning also fails compilation.
- `typelua::compiler::Diagnostic` carries a `severity` (`Severity::Error` or `Severity::Warning`) and a `message`.
- `typelua::runtime::run_file(path)` runs an existing Lua file.
- `typelua::runtime::run_source(source, chunk_name)` runs Lua source held in memory.

## Run a Lua file from Rust

```rust
fn main() -> Result<(), String> {
    typelua::runtime::run_file("script.lua")
}
```

For a Lua script that needs command-line arguments, use `run_file_with_args`:

```rust
fn main() -> Result<(), String> {
    typelua::runtime::run_file_with_args(
        "script.lua",
        &["first-arg".to_owned(), "second-arg".to_owned()],
    )
}
```

The runtime sets `arg[0]` to the chunk name or file path and puts supplied script arguments at `arg[1]`, `arg[2]`, and so on.

## Compile and run source from Rust

```rust
fn main() -> Result<(), String> {
    let source = r#"
        fn greet(str name) {
            print("Hello, " + name);
        }
        greet("Lua");
    "#;

    let lua_source = typelua::compiler::compile(source).map_err(|diagnostics| {
        diagnostics
            .iter()
            .map(|diagnostic| format!("{}: {}", diagnostic.severity.label(), diagnostic.message))
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    typelua::runtime::run_source(&lua_source, "generated.lua")
}
```

The CLI uses the same API: `.tlua` input is compiled to a `.lua` file and then run, while `.lua` input is executed directly. `--check` compiles for validation but does not invoke the runtime.
