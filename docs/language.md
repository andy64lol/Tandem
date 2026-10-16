# TypeLua language guide

TypeLua 0.1 source files conventionally use `.tlua`. They are statically checked and transpiled to Lua 5.4. Type annotations are erased in generated Lua.

## Types

| Type | Values | Lua representation |
| --- | --- | --- |
| `int` | Integer literals such as `42` | `number` |
| `float` | Decimal literals such as `3.5` | `number` |
| `bool` | `true` or `false` | `boolean` |
| `str` | Double-quoted strings | `string` |
| `void` | No return value; function return type | `nil` |

There are no implicit conversions. `int` and `float` are distinct static types, even though both use Lua numbers at runtime. Decimal literals need digits on both sides of the decimal point.

## Variables

Mutable declarations have a type, name, and initializer. Prefix a declaration with `const` to make its binding immutable:

```tlua
int score = 0;
float gravity = 9.8;
bool alive = true;
str playerName = "Ada";
const int maxLives = 3;
```

Declarations inside functions and blocks become Lua locals. Top-level declarations become Lua globals. A `const` binding can be read like any other variable, but TypeLua rejects assignments to it, including compound assignments. Its initializer must match its declared type. Constness is checked by the TypeLua compiler and is erased in generated Lua.

## Functions

Declare functions with `fn`, typed parameters, and an optional return annotation:

```tlua
fn add(int a, int b) -> int {
    return a + b;
}

fn hello() {
    print("Hello!");
}
```

If `-> type` is omitted, the return type defaults to `void`. A non-void function's `return` statement must include a value of the declared type. A void function may use `return;`. The compiler checks return statements but does not ensure every control-flow path returns a value.

Default parameter values must be trailing. Calls may omit trailing defaulted parameters; generated Lua fills them when the parameter is `nil`. Defaults are checked left to right and may refer to preceding parameters.

Function declarations are collected before function bodies are checked, so functions may call declarations that appear later in the source.

## Statements and scope

Every statement ends in `;`. Blocks use braces. `if` and `while` conditions must have type `bool`.

```tlua
int count = 0;

if (count == 0) {
    count += 1;
} else {
    count = 10;
}

while (count < 5) {
    count += 1;
}

for (i, 10) {
    printf("Hello world! {i}");
}
```

Numeric `for` loops include their stop value. The forms are `for (name, stop)`, `for (name, start, stop)`, and `for (name, start, stop, step)`. The first form defaults to start `0` and step `1`; the second defaults to step `1`. All bounds and steps must be `int`. A negative step counts downward. The loop variable is an `int` scoped to the loop body. A zero step is rejected by the Lua runtime.

Supported assignment operators are `=`, `+=`, `-=`, `*=`, and `/=`. Compound assignment is limited to numeric variables, and no assignment operator may target a `const` binding. Variables declared in a block are scoped to that block during checking.

## Expressions

Supported expressions include literals, variables, function calls, parentheses, unary `!` and `-`, arithmetic (`+ - * / %`), string concatenation (`+` and `,`), comparisons (`== != < > <= >=`), and boolean logic (`&& ||`).

Arithmetic operands must have the same numeric type. `+` joins two strings as well as adding matching numeric values; `,` joins two strings. For example, `str greeting = "Hello", " world";` and `str name = "Type" + "Lua";` both concatenate strings. Since commas also separate function arguments, wrap a comma-concatenation expression in parentheses when passing it as an argument: `print(("Hello", " world"));`. Equality requires matching non-void types. Ordered comparisons support matching `int`, `float`, or `str` operands. Boolean operators require boolean operands. The built-in `print(...)` accepts any number of arguments and maps to Lua's `print`.

`printf` accepts one string literal and prints it with `{variable}` interpolation. Placeholders must name variables in scope. Double braces produce literal braces, as in `printf("{{name}} is {playerName}")`.

## Comments

TypeLua supports C-style line and block comments:

```tlua
// A line comment
/* A block comment
   can span multiple lines. */
```

Lua comment forms are accepted too: `--` for a line comment and Lua long comments such as `--[[ ... ]]` or `--[=[ ... ]=]`. The number of `=` characters in a long comment's opening and closing delimiters must match. Block comments do not nest.

These comment forms are accepted in `.tlua` files. Other Lua syntax belongs in a `.lua` file, which TypeLua can execute directly; `.tlua` files otherwise follow the TypeLua grammar described here.

## Current limitations

The language does not currently implement arrays, objects, imports, structs, classes, generics, closures, inferred or optional types, user-defined operators, or typings for the Lua standard library. It has a small parser without error recovery or source maps. A host game engine must provide its own update loop and input API.
