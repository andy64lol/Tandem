# 2. Getting started

## Prerequisites

Install Rust and a working C build toolchain. The C toolchain builds the vendored Lua 5.4 runtime.

## Build

From the project directory:

```sh
cargo build --release
```

This creates `target/release/typelua`.

## Compile and run a program

TypeLua source files use the `.tlua` extension. Compilation writes a `.lua` file next to the source and then executes it:

```sh
cargo run -- examples/hello.tlua
# compiled examples/hello.tlua -> examples/hello.lua
```

Or invoke the compiled binary directly:

```sh
./target/release/typelua examples/hello.tlua
```

Choose another output path by passing it as the second argument; the output directory must already exist:

```sh
cargo run -- examples/hello.tlua build/hello.lua
```

## Check without running

`--check` parses and type-checks the file without writing or executing any Lua. On success it prints `{path}: OK`:

```sh
cargo run -- --check examples/hello.tlua
# examples/hello.tlua: OK
```

To install dependencies declared in `mlua-pkg.toml`, run `typelua pkg install` from the project directory. See [LuaRocks packages](10-cli.md#luarocks-packages) for the manifest format and runtime behavior.

## Run ordinary Lua files

Pass a `.lua` file to execute it directly in the embedded runtime. Extra arguments reach the script through Lua's `arg` table:

```sh
cargo run -- examples/script.lua first-arg second-arg
```

See [Command line](10-cli.md) for the full CLI reference, including error handling and exit codes.
