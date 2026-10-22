# TypeLua examples

- [`hello.tlua`](hello.tlua): a small program with a function call and a loop.
- [`language-tour.tlua`](language-tour.tlua): structs, functions, conditionals, loops, and explicit type retyping.
- [`collections.tlua`](collections.tlua): dictionaries, typed lists, list preservation with `...`, and fixed-shape tuples.
- [`retyping.tlua`](retyping.tlua): changing a variable's type explicitly with `as`.

Compile and run an example from the project root:

```sh
cargo run -- examples/collections.tlua
```

Check an example without generating or running Lua:

```sh
cargo run -- --check examples/language-tour.tlua
```
