# Tandem TODO

## Language

- [x] Add imports and modules for splitting programs across `.tdm` files.
- [x] Add a statically typed table type.
- [x] Improve static typing for values read from generic tables.
- [x] Check that every non-void function returns a value on every control-flow path.
- [x] Add static member signatures for commonly used Lua standard-library functions.
- [x] Add classes with typed fields, constructors, `fn` methods, implicit `self`, and `new` instances.
- [x] Add `class name = new ClassName;` declarations, with `local` and `const` modifiers.
- [x] Reject struct literals for class instances and check field, method, and constructor types.

### Classes and objects

- [x] Support multiple class and interface bases with `super(...)`, `super.method(...)`, overrides, and inherited constructors.
- [x] Allow subclass instances where a parent class type is expected, with dynamic method dispatch.
- [x] Add static fields and static methods.
- [x] Add property getters and setters.
- [x] Add private fields and private methods with `private` visibility.
- [x] Add `instanceof` and runtime class checks.
- [x] Add class expressions and anonymous classes.
- [x] Add computed method names and `super` property access.
- [x] Add mixins and traits for sharing behavior without multiple inheritance.
- [x] Add abstract, final, and protected classes/members.
- [x] Infer class fields from constructor assignments when no initializer is written.
- [x] Export class definitions from compiled `.tdm` modules.
- [x] Add interfaces and protocol conformance checks.
- [x] Support method references and first-class methods.

### Types and language

- [x] Add generics for classes and functions; collection type constructors can contain generic parameters.
- [x] Add closures and first-class functions.
- [x] Add optional types and null safety.
- [x] Add enums and discriminated unions.
- [x] Add user-defined operator overloads.
- [x] Add pattern matching and destructuring.

## Compiler and tooling

- [x] Add a source formatter for `.tdm` files.
- [x] Add a language server with completion and diagnostics.
- [x] Add an interactive REPL.
- [x] Emit byte-accurate debug information for Lua tracebacks.
- [x] Expand static signatures to the remaining Lua standard-library members.
- [x] Report source locations with line and column numbers, and include them in type errors.
- [x] Add parser error recovery so one compile can report more than the first syntax error.
- [x] Add source maps from generated Lua back to the `.tdm` source.
- [x] Add end-to-end coverage for CLI compilation, Lua execution, and runtime errors.

## Runtime

- [x] Expose runtime configuration for Lua memory limits and instruction limits.
- [x] Document how host applications can provide Lua APIs to Tandem programs.