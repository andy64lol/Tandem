# 3. Types

TypeLua 0.1 has five types. Types are checked at compile time and erased in generated Lua.

| Type | Values | Lua representation |
| --- | --- | --- |
| `int` | Integer literals such as `42` | `number` |
| `float` | Decimal literals such as `3.5` | `number` |
| `bool` | `true` or `false` | `boolean` |
| `str` | Double-quoted strings | `string` |
| `void` | No return value; function return type only | `nil` |

There are no implicit conversions. `int` and `float` are distinct static types, even though both use Lua numbers at runtime.

## Literals

- Integer literals: `42`, `0`, `100`.
- Float literals: `3.5`, `9.8`. Decimal literals need digits on both sides of the decimal point, so write `3.5`, not `3.` or `.5`.
- String literals: double-quoted, with the escapes `\n`, `\r`, `\t`, `\"`, and `\\`.
- Boolean literals: `true` and `false`.

## void

`void` is the return type of functions that return nothing. A variable cannot have type `void`; the compiler rejects `void x = ...;`. If a function declaration omits `-> type`, the return type defaults to `void`.

## Type checking

Every expression and statement is checked: initializers must match declared types, operands must have compatible types, arguments must match parameter types, and return values must match the declared return type. Type annotations and `const`/`local` markers exist only in the source; the generated Lua contains plain values and names.
