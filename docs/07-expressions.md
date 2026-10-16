# 7. Expressions

## Forms

Supported expressions include literals, variables, function calls, parentheses, unary `!` and `-`, arithmetic (`+ - * / %`), string concatenation (`+` and `,`), comparisons (`== != < > <= >=`), and boolean logic (`&& ||`).

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
