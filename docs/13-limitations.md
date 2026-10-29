# 13. Limitations

Tandem 0.1.1 is a small proof of concept. The language does not currently implement:

- getters and setters, `instanceof`, class expressions, computed member names, abstract/final/protected access, or class exports from compiled modules;
- fully static typing for `dict` values, and for generic `table` values that were not initialized with a literal;
- closures or first-class functions;
- optional types;
- user-defined operators;
- static member types for every Lua standard-library function: common members are typed, but the rest remain dynamic.

Generic `table` literal shapes are tracked per variable. They are not propagated through aliases, and a function that mutates a global table can invalidate the shape the checker knows at the call site.

For multiple inheritance, the first concrete class base supplies the constructor arguments and `super` target. Later concrete bases must have no required constructor arguments. Generic class inheritance does not yet support explicitly applying or transforming a base class's type arguments.

The parser recovers at statement boundaries, which can discard part of a malformed construct before reporting later errors. Source maps are statement-level comments rather than a byte-accurate mapping, and Lua tracebacks still use the generated file and line numbers.

The runtime embeds stock Lua 5.4 and adds no game-engine facilities: a host game engine must provide its own update loop and input API.

Planned work is tracked in the [project TODO list](../todo.md).
