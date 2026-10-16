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

See the [documentation index](docs/README.md) for the [CLI guide](docs/cli.md), [language guide](docs/language.md), and [runtime/API guide](docs/runtime.md).

## Language specification (0.1)

### Types

- `int`: integer values
- `float`: numeric values with a decimal point
- `bool`: `true` or `false`
- `str`: double-quoted strings
- `void`: function return type only

There are no implicit conversions. `int` and `float` are distinct static types even though both are represented by Lua numbers at runtime. Types are checked at compile time and erased in generated Lua.

### Declarations and functions

```cpp
int score = 0;
float gravity = 9.8;
bool alive = true;
str playerName = "Ada";

fn add(int a, int b) -> int {
    return a + b;
}

fn greet(str name = "world") -> void {
    print("hello");
}
```

Functions may declare a return type after `->`; if omitted, the return type defaults to `void`. Use `-> void` explicitly for functions that do not return a value when desired. Default arguments must be trailing. Default values are emitted as `if parameter == nil then ... end` checks in Lua. At this stage, a default expression should be a literal or another expression that is valid in the function's parameter scope; defaults are type-checked.

### Statements

```cpp
if (alive) {
    score += 1;
} else {
    score = 0;
}

while (score < 10) {
    score += 1;
}

for (i, 10) {
    printf("Hello world! {i}");
}

return;
```

Numeric loops include the stop value. `for (i, stop)` uses a start of `0` and step of `1`; `for (i, start, stop)` sets the start; and `for (i, start, stop, step)` sets all three. Start, stop, and step must be `int`. `printf` prints a string with `{variable}` interpolation; use `{{` or `}}` for literal braces.

Supported assignment operators: `=`, `+=`, `-=`, `*=`, `/=`. Every statement ends in `;`. Blocks use braces. TypeLua accepts `//` line comments and `/* ... */` block comments, as well as Lua's `--` line comments and long comments such as `--[[ ... ]]` and `--[=[ ... ]=]`.

### Expressions

- Literals: integer, decimal float, string, `true`, `false`
- Variables and function calls
- Unary `!` and `-`
- Arithmetic: `+ - * / %`
- Comparisons: `== != < > <= >=`
- Boolean logic: `&& ||`
- Parentheses

`+` supports two numbers of the same type or two strings. Comparisons require compatible operand types. Boolean operators require booleans. `print(...)` maps to Lua's `print` and accepts any number of arguments. `printf("Hello {name}")` accepts one string literal and interpolates variable names in braces.

### Scope and restrictions

- Variables declared in a block are local to that block in the static checker.
- Function parameters and declarations are statically checked.
- Function declarations may appear anywhere in the source and are resolved before checking function bodies, so forward calls work.
- Current limitations: no arrays/objects, imports/modules, structs, classes, generics, closures, inferred types, optional types, user-defined operators, or full Lua standard library typings. No definite-return analysis yet. The runtime `while (true)` loop is supported, but a game engine must provide its own frame/update loop and input API.
- Current implementation has a simple parser and checker; error recovery and source maps are not implemented.

## Example

```cpp
fn movePlayer(int speed = 10) -> int {
    return speed + 1;
}

int playerX = 100;
playerX += movePlayer(20);
if (playerX > 100) {
    print("Player moved!");
}
```

TypeLua embeds Lua 5.4 for running generated or existing `.lua` files. It does not bundle a game engine. `--check` only checks the source and does not run Lua.
