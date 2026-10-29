# 6. Statements

Every statement ends in `;`. Blocks use braces. Declarations are covered in [Variables](04-variables.md); expressions in [Expressions](07-expressions.md).

## Assignment

Supported assignment operators are `=`, `+=`, `-=`, `*=`, and `/=`. Targets can be variables, struct or table fields, tuple element indexes, and dictionary or generic table indexes. Compound assignment requires a numeric target when the target type is known. A variable declared `const` cannot be reassigned. Dictionary fields can be updated, while lists can only be changed by replacing the whole list. Tuple values and elements can change only if the tuple keeps its original length and each position keeps its original type; a `const tuple` cannot be changed.

```tdm
int score = 0;
score = 1;
score += 1;
score -= 1;
score *= 2;
score /= 2;

list int scores = [10, 20];
scores = [..., 12];
// scores[1] = 12; // error: replace the whole list instead

tuple position = (0, 10, 11);
position[1] = 5; // valid: position 1 is still an int

dict player = { name = "Andy", age = 15, elo = 500 };
player.age = 16;
```

## if

An `if` statement takes a parenthesized condition and a block, with an optional `else` block. The condition must have type `bool`.

```tdm
if (count == 0) {
    count += 1;
} else {
    count = 10;
}
```

## while

A `while` loop also requires a `bool` condition. A runtime-forever loop such as `while (true) { ... }` is accepted; a game host normally supplies its own frame loop instead.

```tdm
while (count < 5) {
    count += 1;
}
```

## for

Numeric `for` loops include their stop value. The forms are `for (name, stop)`, `for (name, start, stop)`, and `for (name, start, stop, step)`. The first form defaults to start `0` and step `1`; the second defaults to step `1`. All bounds and steps must be `int`. A negative step counts downward. The loop variable is an `int` scoped to the loop body.

```tdm
for (i, 10) {
    printf("Hello world! {i}");
}
```

A zero step is rejected by the Lua runtime.

## return

A void function may use `return;`. A non-void function must `return` a value of its declared return type. The compiler checks each `return` statement it sees but performs no definite-return analysis, so a function that can finish without returning is not diagnosed.

## Expression statements

An expression on its own is a statement, which is how function calls are written:

```tdm
movePlayer(20);
print("Hello!");
```

## Standalone blocks

Braces can also create a standalone scope:

```tdm
{
    int temporary = 1;
    print(temporary);
}
```

This standalone block creates a nested scope and becomes a Lua `do ... end` block. Its declarations are unavailable after the closing `}`.
