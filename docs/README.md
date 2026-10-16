# The TypeLua Book

TypeLua is an experimental, statically typed, C-style language that transpiles to Lua 5.4. It compiles `.tlua` source files to plain Lua and can run either the generated Lua or ordinary `.lua` files through its embedded runtime. No separate Lua executable is required.

This book is the complete reference for TypeLua 0.1. Chapters are ordered so a first-time reader can learn the language from the beginning and jump into the reference chapters as needed.

## Part I — Getting started

1. [Introduction](01-introduction.md): what TypeLua is, how compilation works, and a first taste of the syntax.
2. [Getting started](02-getting-started.md): build the compiler, run a first program, and check a file without running it.

## Part II — The language

3. [Types](03-types.md): the five types, literals, and how types erase at runtime.
4. [Variables](04-variables.md): declarations, the `local` and `const` modifiers, and scope.
5. [Functions](05-functions.md): declarations, parameters, default values, and return types.
6. [Statements](06-statements.md): assignment, `if`, `while`, `for`, `return`, and blocks.
7. [Expressions](07-expressions.md): operators, precedence, and the built-in `print` and `printf`.
8. [Comments](08-comments.md): C-style and Lua comment forms.

## Part III — Tooling

9. [Diagnostics](09-diagnostics.md): errors, warnings, and why a single warning blocks compilation.
10. [Command line](10-cli.md): every CLI mode, output paths, arguments, and exit codes.
11. [Rust API](11-rust-api.md): embed the compiler and the Lua runtime in a Rust program.

## Part IV — Status

12. [Limitations](12-limitations.md): what TypeLua does not do yet.

The [project TODO list](../todo.md) tracks planned language, tooling, and runtime work.
