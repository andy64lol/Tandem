# Tandem examples

- [`hello.tdm`](hello.tdm): a small program with a function call and a loop.
- [`language-tour.tdm`](language-tour.tdm): structs, functions, conditionals, loops, and explicit type retyping.
- [`collections.tdm`](collections.tdm): dictionaries, typed lists, list preservation with `...`, fixed-shape tuples, and statically typed tables, and type inference for generic table literals.
- [`retyping.tdm`](retyping.tdm): changing a variable's type explicitly with `as`.
- [`stdlib.tdm`](stdlib.tdm): importing standard library packages with static signatures, `auto` inference, and `..` concatenation.
- [`modules.tdm`](modules.tdm): importing a compiled Tandem module and a plain Lua module with `require`.
- [`classes.tdm`](classes.tdm): class definitions, `fn` methods with `self`, constructors, `class` instance declarations with `new`, and single inheritance with `extends` and `super`.

Compile and run an example from the project root:

```sh
cargo run -- examples/collections.tdm
```

Check an example without generating or running Lua:

```sh
cargo run -- --check examples/language-tour.tdm
```
