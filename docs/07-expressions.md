# 7. Expressions

## Forms

Supported expressions include literals, variables, function calls, table and list literals, field and index access, type assertions with `as type`, parentheses, unary `!` and `-`, arithmetic (`+ - * / %`), string concatenation (`+`, `,`, and `..`), comparisons (`== != < > <= >=`), and boolean logic (`&& ||`). A tuple literal is written in parentheses in a `tuple` declaration.

Table literals use `{ ... }`; generic tables may use named or integer keys. A `dict` uses named fields inside `{ ... }`. A `table Key Value` declaration checks the literal against its key and value types: `str` keys use named fields, while `int` keys use positional values. List literals use `[ ... ]` and must match their declared element type. Lists are replaced as a whole, and `...` in a replacement preserves the old list value at the same position; `...` cannot appear elsewhere. Tuple values use `( ... )` in a `tuple` declaration. Their length and positional types are fixed by the first value, though values can be updated without changing those types. `as type` inside a tuple initializer is allowed only for `const tuple`, which locks the whole tuple. Dictionaries use string keys, lists use integer indexes for reads, and generic tables accept integer or string keys. Values read from a generic table keep the types of its literal initializer when known; values read from dictionaries, from generic tables without a literal shape, and from dynamic modules are dynamic. List elements and tuple positions at constant indexes retain their types. A dynamic value needs an explicit `as type` assertion before it can be assigned to a typed variable, field, or parameter. `as` asserts the type but does not convert or validate the value at runtime.

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
| `+` `-` `..` | left |
| `*` `/` `%` | left |
| unary `!` `-` | prefix |

Parentheses group subexpressions as usual. The comma operator binds loosest of all, which is why a comma-concatenation passed as a function argument needs parentheses.

## Arithmetic

Arithmetic operands must have the same numeric type; there is no mixing of `int` and `float`, and no implicit conversions. Unary `-` negates an `int` or a `float`. `%` is Lua's remainder operator on the chosen numeric type.

## Strings

`+`, `,`, and `..` join two strings as well as adding matching numeric values; `..` is Lua's concatenation operator. For example:

```tlua
str greeting = "Hello", " world";
str name = "Type" + "Lua";
str file = "notes" .. ".txt";
```

All three concatenate. Since commas also separate function arguments, wrap a comma-concatenation expression in parentheses when passing it as an argument: `print(("Hello", " world"));`.

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

## Standard library imports

Lua standard library packages are not in scope by default. Each package must be imported at the top level before it can be used:

```tlua
import string, math;

str upper = string.upper("lua");
float root = math.sqrt(16.0);
int length = string.len(upper);
```

`import` may list several packages separated by commas and is only allowed at the top level. Supported packages are `string`, `table`, `math`, `io`, `os`, `coroutine`, `utf8`, `debug`, and `package`. Importing an unknown package is an error: `Unrecognised package 'nope'`.

Common package members have static signatures. For example, `string.upper` and `string.format` return `str`, `string.len` returns `int`, and `math.sqrt` returns `float`. Arguments and return values are checked like any other function call. Members without a signature remain dynamic and require an explicit `as type` assertion. An unknown member is an error such as `package 'string' has no member 'nope'`. A package name used without its import reports `Unrecognised package 'string': import it with 'import string;'`.

Lua's base functions (`assert`, `pairs`, `ipairs`, `pcall`, `tonumber`, `tostring`, `type`, and others) and base values (`_G`, `_VERSION`) follow the same rule: `import base;` brings them into scope. Frequently used base functions have static signatures too, such as `tostring(any) -> str`, `type(any) -> str`, `rawlen(any) -> int`, and `_VERSION -> str`. The TypeLua built-ins `print` and `printf` described above are always available.
