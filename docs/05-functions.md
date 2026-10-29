# 5. Functions

## Declarations

Declare functions with `fn`, typed parameters, and an optional return annotation:

```tdm
fn add(int a, int b) -> int {
    return a + b;
}

fn hello() {
    print("Hello!");
}
```

If `-> type` is omitted, the return type defaults to `void`. A non-void function's `return` statement must include a value of the declared type. A void function may use `return;`. Every control-flow path of a non-void function must return a value. The check understands `return`, nested blocks, `if` statements whose branches both return, and `while (true)` loops. A function that can fall through to its end reports `function 'name' must return a value on every path`.

## Default parameter values

Default parameter values must be trailing; a required parameter after a defaulted one is rejected. Calls may omit trailing defaulted parameters. Generated Lua fills them when the parameter is `nil`:

```tdm
fn greet(str name = "world") -> void {
    print("Hello, " + name);
}

greet();         // uses "world"
greet("Ada");    // uses "Ada"
```

Defaults are type-checked and evaluated left to right, so a default may refer to preceding parameters. At this stage, a default expression should be a literal or another expression valid in the parameter scope.

## Calling and forward declarations

Argument counts must fall between the number of required parameters and the total number of parameters, and each argument must have the declared parameter type.

Function declarations are collected before function bodies are checked, so functions may call declarations that appear later in the source. Function names may not duplicate each other or the built-in names `print` and `printf`.

Functions are not values in 0.1: they cannot be stored in variables, passed as arguments, or returned. See [Limitations](13-limitations.md). Class methods use the same `fn` syntax but run with an implicit `self`; see [Classes](12-classes.md).
