# 6. Statements

Every statement ends in `;`. Blocks use braces. Declarations are covered in [Variables](04-variables.md); expressions in [Expressions](07-expressions.md).

## Assignment

Supported assignment operators are `=`, `+=`, `-=`, `*=`, and `/=`. The target must be a known, mutable variable. Compound assignment is limited to numeric variables, and no assignment operator may target a `const` binding. The right-hand side must have the target's type.

```tlua
int score = 0;
score = 1;
score += 1;
score -= 1;
score *= 2;
score /= 2;
```

## if

An `if` statement takes a parenthesized condition and a block, with an optional `else` block. The condition must have type `bool`.

```tlua
if (count == 0) {
    count += 1;
} else {
    count = 10;
}
```

## while

A `while` loop also requires a `bool` condition. A runtime-forever loop such as `while (true) { ... }` is accepted; a game host normally supplies its own frame loop instead.

```tlua
while (count < 5) {
    count += 1;
}
```

## for

Numeric `for` loops include their stop value. The forms are `for (name, stop)`, `for (name, start, stop)`, and `for (name, start, stop, step)`. The first form defaults to start `0` and step `1`; the second defaults to step `1`. All bounds and steps must be `int`. A negative step counts downward. The loop variable is an `int` scoped to the loop body.

```tlua
for (i, 10) {
    printf("Hello world! {i}");
}
```

A zero step is rejected by the Lua runtime.

## return

A void function may use `return;`. A non-void function must `return` a value of its declared return type. The compiler checks each `return` statement it sees but performs no definite-return analysis, so a function that can finish without returning is not diagnosed.

## Expression statements

An expression on its own is a statement, which is how function calls are written:

```tlua
movePlayer(20);
print("Hello!");
```

## Standalone blocks

Braces can also create a standalone scope:

```tlua
{
    int temporary = 1;
    print(temporary);
}
```

This standalone block creates a nested scope and becomes a Lua `do ... end` block. Its declarations are unavailable after the closing `}`.
