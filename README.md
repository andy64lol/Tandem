# Tandem

**A language, compiler, package workflow, and embedded runtime for Lua programs.**

Tandem gives you a statically checked language for writing Lua, then carries that code through compilation and execution. It emits readable Lua 5.4, runs Lua inside its own executable, can run existing `.lua` scripts, and installs pure-Lua dependencies declared in a project manifest. The result is a small, self-contained toolchain around Lua—not only a layer of type annotations.

Tandem is an experimental proof of concept (version 0.1.1), and is not production-ready.

## What you can do

- **Write Tandem:** use C-style declarations, functions, control flow, structs, classes with `fn` methods, `self`, and inheritance, dictionaries, homogeneous typed lists, fixed-shape tuples, statically typed key-value tables, and compile-time checks.
- **Retype deliberately:** use an explicit `as type` assertion when assigning a new type to a variable, or declare with `auto` to infer the type once and then lock it.
- **Import the standard library:** bring Lua standard library packages into scope with `import string, math;` before using them.
- **Compile and inspect:** generate ordinary Lua 5.4 files that you can keep, inspect, or use with Lua tools, with `-- tdm:line:column` source-map comments.
- **Get precise diagnostics:** parser recovery reports multiple syntax errors, and syntax and type errors include line and column locations.
- **Run Lua programs:** execute generated output or existing `.lua` scripts with the embedded Lua 5.4 runtime; you do not need a separate Lua executable. Optional memory and instruction limits guard against runaway scripts.
- **Add Lua packages:** install pure-Lua packages from Git repositories using `mlua-pkg` and an `mlua-pkg.toml` manifest.
- **Split programs across files:** bind compiled `.tdm` modules with `module name = require("path.tdm");`, or `require` ordinary `.lua` modules that are loaded by Lua at runtime.
- **Embed the runtime:** call Tandem's Lua execution API from another Rust program, configure resource limits, and install host APIs with a setup callback.

## Get started

Install Rust and a working C build toolchain, then build Tandem:

```sh
cargo build --release
```

Compile a `.tdm` file. Tandem writes a `.lua` file beside it and runs that output:

```sh
cargo run -- examples/hello.tdm
```

Check a program without writing or running Lua:

```sh
cargo run -- --check examples/hello.tdm
```

Run an existing Lua file directly, passing any script arguments after its path:

```sh
cargo run -- examples/script.lua first-arg second-arg
```

## A Tandem program

```tdm
int playerX = 100;

fn movePlayer(int speed = 10) -> int {
    return speed + 1;
}

playerX += movePlayer(20);
if (playerX > 100) {
    print("Player moved!");
}
```

Type annotations are checked before code is emitted and erased from the generated Lua. Tandem adds fixed variable types with explicit `as type` retyping, named dictionaries, homogeneous lists with whole-list updates, and tuples with fixed length and positional types. It also includes `int`, `float`, `bool`, `str`, generic `table`, structs, and `void` return types. See the [types reference](docs/03-types.md) for details.

## Lua package workflow

Declare a LuaRocks package by its Git repository in `mlua-pkg.toml`:

```toml
[package]
name = "my-project"
version = "0.1.0"

[deps]
penlight = { git = "https://github.com/lunarmodules/Penlight", tag = "1.14.0" }
```

Install dependencies from the project directory:

```sh
cargo run -- pkg install
```

At runtime, `require("pl.path")` can resolve Penlight through the generated `mlua-pkg.lock` lockfile. Package sources are fetched from Git repositories and described by rockspecs. The current integration supports pure-Lua packages; it does not search LuaRocks by package name or build C modules.

## Use Lua from Rust

The library exposes `tandem::runtime::run_file("script.lua")` and `tandem::runtime::run_source(source, "chunk-name")` for running Lua from a Rust host. `RuntimeConfig` adds memory and instruction limits, and `run_source_with_setup` lets a host install Lua functions before the chunk runs. See the [Rust API](docs/11-rust-api.md) for details.

## Documentation

Read [The Tandem Book](docs/README.md), starting with the [introduction](docs/01-introduction.md) and [getting started](docs/02-getting-started.md). It covers the [language](docs/03-types.md), [classes](docs/12-classes.md), [CLI](docs/10-cli.md), [Rust API](docs/11-rust-api.md), and [current limitations](docs/13-limitations.md). Planned work is tracked in the [TODO list](todo.md).
