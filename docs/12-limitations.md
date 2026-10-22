# 12. Limitations

TypeLua 0.1.1 is a small proof of concept. The language does not currently implement:

- imports or modules for splitting programs across `.tlua` files;
- classes, inheritance, or generics;
- static value typing for values read from generic `table` and `dict` collections;
- closures or first-class functions;
- inferred or optional types;
- user-defined operators;
- typings for the Lua standard library;
- definite-return analysis for non-void functions.

The implementation has a small parser without error recovery or source maps, and compile errors report byte positions rather than line and column numbers.

The runtime embeds stock Lua 5.4 and adds no game-engine facilities: a host game engine must provide its own update loop and input API.

Planned work is tracked in the [project TODO list](../todo.md).
