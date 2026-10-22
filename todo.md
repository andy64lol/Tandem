# TypeLua TODO

## Language

- [x] Add imports and modules for splitting programs across `.tlua` files.
- [x] Add a statically typed table type.
- [x] Improve static typing for values read from generic tables.
- [x] Check that every non-void function returns a value on every control-flow path.
- [x] Add static member signatures for commonly used Lua standard-library functions.

## Compiler and tooling

- [x] Report source locations with line and column numbers, and include them in type errors.
- [x] Add parser error recovery so one compile can report more than the first syntax error.
- [x] Add source maps from generated Lua back to the `.tlua` source.
- [x] Add end-to-end coverage for CLI compilation, Lua execution, and runtime errors.

## Runtime

- [x] Expose runtime configuration for Lua memory limits and instruction limits.
- [x] Document how host applications can provide Lua APIs to TypeLua programs.
