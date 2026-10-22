# TypeLua TODO

## Language

- [ ] Add imports and modules for splitting programs across `.tlua` files.
- [ ] Add a statically typed table type.
- [ ] Improve static typing for values read from generic tables.
- [ ] Check that every non-void function returns a value on every control-flow path.
- [ ] Add type declarations for commonly used Lua standard-library functions.

## Compiler and tooling

- [ ] Report source locations with line and column numbers, and include them in type errors.
- [ ] Add parser error recovery so one compile can report more than the first syntax error.
- [ ] Add source maps from generated Lua back to the `.tlua` source.
- [ ] Add end-to-end coverage for CLI compilation, Lua execution, and runtime errors.

## Runtime

- [ ] Expose runtime configuration for Lua memory limits and instruction limits.
- [ ] Document how host applications can provide Lua APIs to TypeLua programs.
