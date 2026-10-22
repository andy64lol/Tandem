# 3. Types

Primitive types and collection types are checked at compile time and erased in generated Lua. Struct types retain their field names at runtime as ordinary Lua tables.

| Type | Values | Lua representation |
| --- | --- | --- |
| `int` | Integer literals such as `42` | `number` |
| `float` | Decimal literals such as `3.5` | `number` |
| `bool` | `true` or `false` | `boolean` |
| `str` | Double-quoted strings | `string` |
| `table` | A Lua table with dynamically typed keys and values | `table` |
| `table Key Value` | A typed table whose keys have type `Key` (`str` or `int`) and values have type `Value` | `table` |
| `dict` | Named, dynamically typed values | `table` |
| `list Type` | Ordered values that all have the given type | `table` |
| `tuple` | Ordered values with a fixed length and a fixed type at each position | `table` |
| `void` | No return value; function return type only | `nil` |

There are no implicit conversions. `int` and `float` are distinct static types, even though both use Lua numbers at runtime.

`auto` is not a type of its own: it infers a variable's type from its initializer and locks it. It is valid only in a variable declaration with an initializer. See [Variables](04-variables.md#automatic-type-inference).

A declaration can use the `module` type to bind a required file: `module greetings = require("greetings.tlua");`. The compiler infers the module's exported members from that file. A bare `module` type outside such a declaration is rejected.

Lists use a type after `list`, for example `list int scores = [10, 20, 30];`. List elements must match that type. Lists use square brackets and Lua's one-based indexes. Do not use `int[]`; curly braces are for tables and dictionaries, and parentheses are for tuples.

A plain `table` is a generic Lua table whose keys and values are dynamic. A generic table initialized with a table literal keeps the static types of its named fields and positional values for reads through that variable: `table t = { count = 1 }; int n = t.count;` is checked. Assigning a known field or index validates its value type. Reassigning the whole table makes subsequent reads dynamic again. `table Key Value` is a statically typed table: `Key` must be `str` or `int`, and every read or write is checked against `Value`. With `str` keys, literals use named fields and values can be read with both `t.name` and `t["name"]`. With `int` keys, literals are positional and receive keys 1, 2, 3, and so on. Typed tables still compile to ordinary Lua tables.

```tlua
table str int ages = { alice = 21, bob = 32 };
int bob = ages.bob;
ages["carol"] = 17;
ages.dave = 4;

table int str names = { "zero", "one", "two" };
str one = names[1];
```

Use `dict` for named keys, `list Type` for resizable ordered values of one type, and `tuple` for an ordered group whose length and positional types are inferred from its first value. These collections compile to Lua tables. Values read from a generic `table` keep the types of its literal initializer when known; values read from a `dict`, from a generic `table` without a literal shape, and values exported by dynamic modules have no statically known type. Dynamic values require an explicit `as type` assertion to assign them to a statically typed variable, field, or parameter. List reads retain the list's element type, and tuple reads at a constant index retain that position's type. An assertion does not convert or validate the value at runtime.

Structs define named fields and their types:

```tlua
struct Point {
    float x;
    float y;
}

Point origin = { x = 0.0, y = 0.0 };
origin.x = 2.0;
```

Struct initializers must provide each field exactly once with a matching value type. Struct declarations are top-level. Their values compile to Lua tables.

```tlua
table options = { debug = true, retries = 3 };
print(options["debug"]);

dict player = { name = "Maria", age = 35, points = 500 };
player.age = 16;
list int scores = [10, 20, 30];
scores = [..., 12, ...];
tuple position = (0, 10, 11);
tuple position = (4, 10, 11); // same length and positional types
position[1] = 5;
int x = position[1];
```

Lists and tuples use Lua's one-based indexing. A list can only be changed by assigning a replacement list; list index assignment is rejected. In a list replacement, each `...` keeps the old value at that position, and `...` is only valid in lists. Tuples can be reassigned or updated by index only while keeping the original length and type at every position. `as type` inside a tuple initializer is allowed only for `const tuple`, which locks the whole tuple. A variable cannot be made const after its first declaration. Lists and dictionaries can be redefined only while their variable is mutable; a list redefinition keeps its element type.

## Literals

- Integer literals: `42`, `0`, `100`.
- Float literals: `3.5`, `9.8`. Decimal literals need digits on both sides of the decimal point, so write `3.5`, not `3.` or `.5`.
- String literals: double-quoted, with the escapes `\n`, `\r`, `\t`, `\"`, and `\\`.
- Boolean literals: `true` and `false`.

## void

`void` is the return type of functions that return nothing. A variable cannot have type `void`; the compiler rejects `void x = ...;`. If a function declaration omits `-> type`, the return type defaults to `void`.

## Type checking

Every expression and statement is checked: initializers must match declared types, operands must have compatible types, arguments must match parameter types, and return values must match the declared return type. Type annotations and `const`/`local` markers exist only in the source; the generated Lua contains plain values and names.
