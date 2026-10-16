# TypeLua

TypeLua is an experimental, statically typed, C-style language that transpiles to Lua 5.4. It is inspired by the idea of TypeScript for Lua: keep Lua as the runtime target, but give authors familiar syntax and compile-time type checks.

**Status:** proof-of-concept compiler and runtime, version 0.1.0. This is not production-ready. Lua 5.4 is embedded through `mlua` with its vendored Lua feature, so a separate Lua executable is not needed.

## Build

Install Rust and a working C build toolchain for the vendored Lua runtime, then run:

```sh
cargo build --release
```

## Use

```sh
cargo run -- examples/hello.tlua
# writes examples/hello.lua and runs it in the embedded Lua 5.4 runtime

cargo run -- --check examples/hello.tlua
```

Run an existing Lua file directly, optionally passing script arguments:

```sh
cargo run -- examples/script.lua first-arg second-arg
```

Or invoke the compiled binary directly:

```sh
./target/release/typelua examples/hello.tlua output.lua
```

The compiled Lua is run after it is written. The Rust library also exposes `typelua::runtime::run_file("script.lua")` and `typelua::runtime::run_source(source, "chunk-name")` for embedding Lua execution in another Rust program.

See [The TypeLua Book](docs/README.md) for the full documentation, starting with the [introduction](docs/01-introduction.md) and [getting started](docs/02-getting-started.md). The reference chapters cover [types](docs/03-types.md), [variables](docs/04-variables.md), [functions](docs/05-functions.md), [statements](docs/06-statements.md), [expressions](docs/07-expressions.md), [comments](docs/08-comments.md), [diagnostics](docs/09-diagnostics.md), the [command line](docs/10-cli.md), the [Rust API](docs/11-rust-api.md), and [limitations](docs/12-limitations.md). The [TODO list](todo.md) tracks planned work.

## Language at a glance

- **Types:** `int`, `float`, `bool`, `str`, and `void` (return type only). No implicit conversions; annotations are checked at compile time and erased in generated Lua.
- **Declarations:** `type name = value;`, optionally marked `local` (a Lua local instead of a top-level global) and/or `const` (immutable in source).
- **Functions:** `fn name(type param = default) -> type { ... }`, with trailing default parameters and optional return types.
- **Statements:** `if`/`else`, `while`, numeric `for` (stop value included), assignment (`=`, `+=`, `-=`, `*=`, `/=`), `return`, and standalone blocks. Every statement ends in `;`.
- **Expressions:** arithmetic, string concatenation with `+` or `,`, comparisons, `&&`/`||` logic, plus the built-ins `print(...)` and `printf("Hello {name}")`.

```tlua
int playerX = 100;

fn movePlayer(int speed = 10) -> int {
    return speed + 1;
}

playerX += movePlayer(20);
if (playerX > 100) {
    print("Player moved!");
}
```

TypeLua embeds Lua 5.4 for running generated or existing `.lua` files. It does not bundle a game engine. `--check` only checks the source and does not run Lua.
