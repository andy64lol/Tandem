# TypeLua examples

- [`hello.tlua`](hello.tlua): a small program with a function call and a loop.
- [`language-tour.tlua`](language-tour.tlua): structs, functions, conditionals, loops, and explicit type retyping.
- [`collections.tlua`](collections.tlua): dictionaries, typed lists, list preservation with `...`, fixed-shape tuples, and statically typed tables, and type inference for generic table literals.
- [`retyping.tlua`](retyping.tlua): changing a variable's type explicitly with `as`.
- [`stdlib.tlua`](stdlib.tlua): importing standard library packages with static signatures, `auto` inference, and `..` concatenation.
- [`modules.tlua`](modules.tlua): importing a compiled TypeLua module and a plain Lua module with `require`.
- [`classes.tlua`](classes.tlua): class definitions, `fn` methods with `self`, constructors, `class` instance declarations with `new`, and single inheritance with `extends` and `super`.

Compile and run an example from the project root:

```sh
cargo run -- examples/collections.tlua
```

Check an example without generating or running Lua:

```sh
cargo run -- --check examples/language-tour.tlua
```
