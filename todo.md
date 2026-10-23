# TypeLua TODO

## Language

- [x] Add imports and modules for splitting programs across `.tlua` files.
- [x] Add a statically typed table type.
- [x] Improve static typing for values read from generic tables.
- [x] Check that every non-void function returns a value on every control-flow path.
- [x] Add static member signatures for commonly used Lua standard-library functions.
- [x] Add classes with typed fields, constructors, `fn` methods, implicit `self`, and `new` instances.
- [x] Add `class name = new ClassName;` declarations, with `local` and `const` modifiers.
- [x] Reject struct literals for class instances and check field, method, and constructor types.

### Classes and objects

- [x] Support single inheritance with `class Child extends Parent`, `super(...)`, `super.method(...)`, overrides, and inherited constructors.
- [x] Allow subclass instances where a parent class type is expected, with dynamic method dispatch.
- [ ] Add static fields, static methods, and static initialization blocks.
- [ ] Add property getters and setters.
- [ ] Add private fields, private methods, and `#`-style encapsulation.
- [ ] Add `instanceof` and runtime class checks.
- [ ] Add class expressions and anonymous classes.
- [ ] Add computed method names and `super` property access.
- [ ] Add mixins and traits for sharing behavior without multiple inheritance.
- [ ] Add abstract, final, and protected classes/members.
- [ ] Infer class fields from constructor assignments when no initializer is written.
- [ ] Export class definitions from compiled `.tlua` modules.
- [ ] Add interfaces and protocol conformance checks.
- [ ] Support method references and first-class methods.

### Types and language

- [ ] Add generics for classes, functions, and collections.
- [ ] Add closures and first-class functions.
- [ ] Add optional types and null safety.
- [ ] Add enums and discriminated unions.
- [ ] Add user-defined operator overloads.
- [ ] Add pattern matching and destructuring.

## Compiler and tooling

- [ ] Add a source formatter for `.tlua` files.
- [ ] Add a language server with completion and diagnostics.
- [ ] Add an interactive REPL.
- [ ] Emit byte-accurate debug information for Lua tracebacks.
- [ ] Expand static signatures to the remaining Lua standard-library members.
- [x] Report source locations with line and column numbers, and include them in type errors.
- [x] Add parser error recovery so one compile can report more than the first syntax error.
- [x] Add source maps from generated Lua back to the `.tlua` source.
- [x] Add end-to-end coverage for CLI compilation, Lua execution, and runtime errors.

## Runtime

- [x] Expose runtime configuration for Lua memory limits and instruction limits.
- [x] Document how host applications can provide Lua APIs to TypeLua programs.
