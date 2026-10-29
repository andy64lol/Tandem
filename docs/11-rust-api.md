# 11. Rust API

Tandem uses [`mlua`](https://docs.rs/mlua/) with Lua 5.4 and its vendored Lua build. The Rust library exposes the compiler and runtime as separate modules:

- `tandem::compiler::compile(&str) -> Result<String, Vec<Diagnostic>>` parses and type-checks Tandem source, then returns generated Lua. On any diagnostic it returns the diagnostics instead; because warnings are fatal, a single warning also fails compilation.
- `tandem::compiler::Diagnostic` carries a `severity` (`Severity::Error` or `Severity::Warning`), a `message`, and optional `line` and `column` values.
- `tandem::runtime::run_file(path)` runs an existing Lua file.
- `tandem::runtime::run_source(source, chunk_name)` runs Lua source held in memory.
- `tandem::runtime::RuntimeConfig` sets optional Lua memory and instruction limits.
- `tandem::runtime::run_source_with_setup(...)` runs Lua after a host callback has installed additional Lua APIs.
- `tandem::mlua` re-exports the `mlua` crate so hosts and the runtime use the same Lua types.

## Run a Lua file from Rust

```rust
fn main() -> Result<(), String> {
    tandem::runtime::run_file("script.lua")
}
```

For a Lua script that needs command-line arguments, use `run_file_with_args`:

```rust
fn main() -> Result<(), String> {
    tandem::runtime::run_file_with_args(
        "script.lua",
        &["first-arg".to_owned(), "second-arg".to_owned()],
    )
}
```

The runtime sets `arg[0]` to the chunk name or file path and puts supplied script arguments at `arg[1]`, `arg[2]`, and so on.

## Runtime limits

Use `RuntimeConfig` with the `*_with_config` functions to bound embedded Lua execution:

```rust
use tandem::runtime::{self, RuntimeConfig};

fn main() -> Result<(), String> {
    let config = RuntimeConfig {
        memory_limit: Some(64 * 1024 * 1024),
        instruction_limit: Some(10_000_000),
    };
    runtime::run_file_with_config("script.lua", &[], &config)
}
```

`memory_limit` is in bytes and is enforced by the Lua allocator. `instruction_limit` counts Lua VM instructions and aborts runaway scripts; the runtime checks in batches of at most 10,000 instructions. Both failures are returned as `Err(String)`.

## Provide host APIs

`run_source_with_setup` calls a closure with the live `Lua` value before the chunk runs. The callback can install functions, tables, or globals, and must return `Result<(), String>`:

```rust
use tandem::runtime::{self, RuntimeConfig};

fn main() -> Result<(), String> {
    runtime::run_source_with_setup(
        "print(greet("Lua"))",
        "host.lua",
        &[],
        std::path::Path::new("."),
        &RuntimeConfig::default(),
        |lua| {
            let greet = lua
                .create_function(|_, name: String| Ok(format!("Hello, {name}!")))
                .map_err(|error| error.to_string())?;
            lua.globals()
                .set("greet", greet)
                .map_err(|error| error.to_string())
        },
    )
}
```

The standard package search paths and installed `mlua-pkg` packages are configured before the callback runs.

## Compile and run source from Rust

```rust
fn main() -> Result<(), String> {
    let source = r#"
        fn greet(str name) {
            print("Hello, " + name);
        }
        greet("Lua");
    "#;

    let lua_source = tandem::compiler::compile(source).map_err(|diagnostics| {
        diagnostics
            .iter()
            .map(|diagnostic| format!("{}: {}", diagnostic.severity.label(), diagnostic.message))
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    tandem::runtime::run_source(&lua_source, "generated.lua")
}
```

The CLI uses the same API: `.tdm` input is compiled to a `.lua` file and then run, while `.lua` input is executed directly. `--check` compiles for validation but does not invoke the runtime.
