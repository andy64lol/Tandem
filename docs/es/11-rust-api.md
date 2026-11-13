# 11. API de Rust

Tandem usa [`mlua`](https://docs.rs/mlua/) con Lua 5.4 y su build de Lua vendido. La biblioteca de Rust expone el compilador y el runtime como módulos separados:

- `tandem::compiler::compile(&str) -> Result<String, Vec<Diagnostic>>` analiza y comprueba los tipos del código fuente de Tandem y devuelve el Lua generado. Ante cualquier diagnóstico devuelve los diagnósticos; como las advertencias son fatales, una sola advertencia también falla la compilación.
- `tandem::compiler::Diagnostic` lleva una `severity` (`Severity::Error` o `Severity::Warning`), un `message` y valores opcionales `line` y `column`.
- `tandem::runtime::run_file(path)` ejecuta un archivo Lua existente.
- `tandem::runtime::run_source(source, chunk_name)` ejecuta código fuente Lua mantenido en memoria.
- `tandem::runtime::RuntimeConfig` establece límites opcionales de memoria e instrucciones de Lua.
- `tandem::runtime::run_source_with_setup(...)` ejecuta Lua después de que un callback anfitrión haya instalado APIs de Lua adicionales.
- `tandem::mlua` re-exporta el crate `mlua` para que anfitriones y runtime usen los mismos tipos de Lua.

## Ejecutar un archivo Lua desde Rust

```rust
fn main() -> Result<(), String> {
    tandem::runtime::run_file("script.lua")
}
```

Para un script Lua que necesita argumentos de línea de comandos, usa `run_file_with_args`:

```rust
fn main() -> Result<(), String> {
    tandem::runtime::run_file_with_args(
        "script.lua",
        &["first-arg".to_owned(), "second-arg".to_owned()],
    )
}
```

El runtime establece `arg[0]` en la ruta o nombre del chunk y coloca los argumentos del script suministrados en `arg[1]`, `arg[2]`, etc.

## Límites de runtime

Usa `RuntimeConfig` con las funciones `*_with_config` para acotar la ejecución de Lua embebido:

```rust
use tandem::runtime::{self, RuntimeConfig};

fn main() -> Result<(), String> {
    let config = RuntimeConfig {
        memory_limit: Some(64 * 1024 * 1024),
        instruction_limit: Some(10_000_000),
        sandbox: false,
    };
    runtime::run_file_with_config("script.lua", &[], &config)
}
```

`memory_limit` está en bytes y lo hace cumplir el asignador de Lua. `instruction_limit` cuenta instrucciones de la VM de Lua y aborta scripts desbocados; el runtime comprueba en lotes de como máximo 10,000 instrucciones. Ambos fallos se devuelven como `Err(String)`.

## Proporcionar APIs anfitrionas

`run_source_with_setup` llama a un closure con el valor `Lua` vivo antes de que se ejecute el chunk. El callback puede instalar funciones, tablas o globales y debe devolver `Result<(), String>`:

```rust
use tandem::runtime::{self, RuntimeConfig};

fn main() -> Result<(), String> {
    runtime::run_source_with_setup(
        "print(greet(\"Lua\"))",
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

Las rutas de búsqueda de paquetes estándar y los paquetes `mlua-pkg` instalados se configuran antes de que se ejecute el callback.

## Compilar y ejecutar código fuente desde Rust

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

La CLI usa la misma API: la entrada `.tdm` se compila a un archivo `.lua` y luego se ejecuta, mientras que la entrada `.lua` se ejecuta directamente. `--check` compila para validar pero no invoca el runtime.
