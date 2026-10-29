# 1. Introduction

Tandem is an experimental, statically typed, C-style language that transpiles to Lua 5.4. It is inspired by the idea of TypeScript for Lua: keep Lua as the runtime target, but give authors familiar syntax and compile-time type checks.

**Status:** proof-of-concept compiler and runtime, version 0.1.1. This is not production-ready.

## How compilation works

Tandem source files use the `.tdm` extension. The compiler:

1. lexes and parses the file,
2. type-checks the program,
3. erases all type annotations and modifiers,
4. emits a `.lua` file next to the source, or to a path you choose,
5. runs the generated Lua in an embedded Lua 5.4 interpreter.

The interpreter is Lua 5.4 itself, embedded through [`mlua`](https://docs.rs/mlua/) with its vendored Lua feature, so a separate `lua` executable is not needed. Tandem does not bundle a game engine; a host application provides any frame loop and input API.

## A first taste

```tdm
int playerX = 100;

fn movePlayer(int speed = 10) -> void {
    playerX += speed;
}

movePlayer(20);

if (playerX > 100) {
    print("Player moved!");
}
```

Types such as `int` are checked at compile time and erased in the generated Lua, so the output is ordinary readable Lua.

## How to read this book

If Tandem is new to you, read [Getting started](02-getting-started.md) next, then work through Part II in order. The tooling chapters can be read on their own.
