# Command line guide

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
# writes examples/hello.lua and runs it
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

`--check` parses and type-checks the file. It does not write or execute generated Lua:

```sh
typelua --check examples/hello.tlua
cargo run -- --check examples/hello.tlua
```

## Run ordinary Lua files

Pass a `.lua` file to execute it directly in the embedded Lua 5.4 runtime. Additional arguments are available to the script through Lua's `arg` table (`arg[0]` is the path, and script arguments start at `arg[1]`):

```sh
typelua examples/script.lua first-arg second-arg
cargo run -- examples/script.lua first-arg second-arg
```

The `.lua` file is run as written; it is not parsed as TypeLua or transpiled.

## Errors

Source and Lua runtime errors are printed to standard error, and the command exits with a nonzero status. A TypeLua compile error prevents output generation. A Lua runtime error happens after the output file has been written, so the generated file remains available for inspection.
