# 7. Expressions

## Forms

Supported expressions include literals, variables, function calls, table and list literals, field and index access, type assertions with `as type`, parentheses, unary `!` and `-`, arithmetic (`+ - * / %`), string concatenation (`+` and `,`), comparisons (`== != < > <= >=`), and boolean logic (`&& ||`). A tuple literal is written in parentheses in a `tuple` declaration.

Table literals use `{ ... }`; generic tables may use named or integer keys. A `dict` uses named fields inside `{ ... }`. List literals use `[ ... ]` and must match their declared element type. Lists are replaced as a whole, and `...` in a replacement preserves the old list value at the same position; `...` cannot appear elsewhere. Tuple values use `( ... )` in a `tuple` declaration. Their length and positional types are fixed by the first value, though values can be updated without changing those types. `as type` inside a tuple initializer is allowed only for `const tuple`, which locks the whole tuple. Dictionaries use string keys, lists use integer indexes for reads, and generic tables accept integer or string keys. Values read from generic tables and dictionaries, and values exported by dynamic modules, are dynamic; list elements and tuple positions at constant indexes retain their types. A dynamic value needs an explicit `as type` assertion before it can be assigned to a typed variable, field, or parameter. `as` asserts the type but does not convert or validate the value at runtime.

```tlua
list int values = [4, 8, 15];
int first = values[1];
table config = { retries = 3 };
print(config.retries);

dict player = { name = "Andy", age = 15, elo = 500 };
tuple position = (0, 10, 11);
tuple position = (10, 20, 30);
int x = position[1];
```

## Operator precedence

From loosest to tightest binding:

| Operators | Associativity |
| --- | --- |
| `,` | left |
| `\|\|` | left |
| `&&` | left |
| `==` `!=` | left |
| `<` `>` `<=` `>=` | left |
| `+` `-` | left |
| `*` `/` `%` | left |
| unary `!` `-` | prefix |

Parentheses group subexpressions as usual. The comma operator binds loosest of all, which is why a comma-concatenation passed as a function argument needs parentheses.

## Arithmetic

Arithmetic operands must have the same numeric type; there is no mixing of `int` and `float`, and no implicit conversions. Unary `-` negates an `int` or a `float`. `%` is Lua's remainder operator on the chosen numeric type.

## Strings

`+` joins two strings as well as adding matching numeric values; `,` also joins two strings. For example:

```tlua
str greeting = "Hello", " world";
str name = "Type" + "Lua";
```

Both concatenate. Since commas also separate function arguments, wrap a comma-concatenation expression in parentheses when passing it as an argument: `print(("Hello", " world"));`.

## Comparisons and logic

Equality requires matching non-void types. Ordered comparisons (`< > <= >=`) support matching `int`, `float`, or `str` operands and produce a `bool`. Boolean operators `&&` and `||` require boolean operands, as does unary `!`.

## Built-in functions

### print

The built-in `print(...)` accepts any number of arguments of any type and maps to Lua's `print`.

### printf

`printf` accepts one string literal and prints it with `{variable}` interpolation. Placeholders must name variables in scope. Double braces produce literal braces, as in `printf("{{name}} is {playerName}")`. An unmatched `}`, or a placeholder that is not a variable name, is a compile error.

```tlua
int i = 1;
printf("Hello world! {i}");
// prints: Hello world! 1
```

Generated Lua builds the output by concatenating the text with each variable converted by `tostring`.
