# 10. Command line

## LuaRocks packages

TypeLua uses `mlua-pkg` to install pure-Lua LuaRocks packages from their Git repositories. Add a manifest to the project directory:

```toml
[package]
name = "my-project"
version = "0.1.0"

[deps]
penlight = { git = "https://github.com/lunarmodules/Penlight", tag = "1.14.0" }
```

Then run:

```sh
typelua pkg install
```

At runtime, TypeLua loads installed modules from `mlua-pkg.lock`. For example, `require("pl.path")` resolves Penlight's path module. Package dependencies are pinned in the lockfile. `mlua-pkg` reads rockspecs from Git repositories; it does not look packages up by name on luarocks.org, and C modules are not built as part of installation.

## Synopsis

```
typelua <input.tlua|script.lua> [output.lua or script arguments...]
typelua --check <input.tlua>
typelua pkg install [project-dir]
```

## Build

Install Rust and a working C build toolchain, then build the compiler and its embedded Lua 5.4 runtime from the project directory:

```sh
cargo build --release
```

This creates `target/release/typelua`. The Lua runtime is built through the `mlua` dependency with vendored Lua, so a separate `lua` executable is not required.

## Compile and run TypeLua source

TypeLua source files use the `.tlua` extension. Compilation writes a `.lua` file next to the source and then executes it:

```sh
typelua examples/hello.tlua
# compiled examples/hello.tlua -> examples/hello.lua
```

Choose another output path by passing it as the second argument:

```sh
typelua examples/hello.tlua build/hello.lua
```

The output directory must already exist. When developing in this repository, use Cargo to run the CLI:

```sh
cargo run -- examples/hello.tlua
cargo run -- examples/hello.tlua examples/hello-custom.lua
```

## Check without running

`--check` parses and type-checks the file. It does not write or execute generated Lua, and on success prints `{path}: OK`:

```sh
typelua --check examples/hello.tlua
# examples/hello.tlua: OK
cargo run -- --check examples/hello.tlua
```

## Run ordinary Lua files

Pass a `.lua` file to execute it directly in the embedded Lua 5.4 runtime. Additional arguments are available to the script through Lua's `arg` table (`arg[0]` is the path, and script arguments start at `arg[1]`):

```sh
typelua examples/script.lua first-arg second-arg
cargo run -- examples/script.lua first-arg second-arg
```

The `.lua` file is run as written; it is not parsed as TypeLua or transpiled.

## Diagnostics

Source and Lua runtime errors are printed to standard error, and the command exits with a nonzero status:

- A TypeLua diagnostic (error or warning) is printed as `TypeLua error: ...` or `TypeLua warning: ...`. Because warnings are fatal, either form prevents output generation and exits with status 1. See [Diagnostics](09-diagnostics.md).
- A Lua runtime error happens after the output file has been written, so the generated file remains available for inspection. The command exits with status 1.
- An invalid command line prints the usage message and exits with status 2.

On success the command exits with status 0.
