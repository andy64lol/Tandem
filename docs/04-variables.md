# 4. Variables

## Declarations

Declarations use `(local) (const) type name = value;`, where the parentheses mark optional modifiers. For example:

```tlua
int score = 0;
float gravity = 9.8;
bool alive = true;
str playerName = "Ada";
const int maxLives = 3;
local int scoreLimit = 100;
(local) (const) str edition = "standard";
```

Every declaration needs an initializer, and the initializer must have the declared type. A variable cannot be declared with type `void`.

## The local and const modifiers

The `local` and `const` modifiers are optional, shown here as `(local)` and `(const)`. They may appear in either order, and may be written with or without the parentheses. Thus `local const int limit = 10;`, `(local) (const) int limit = 10;`, and `const (local) int limit = 10;` are all accepted forms. Writing the same modifier twice is an error.

### local

Inside functions and blocks, declarations become Lua locals whether or not `local` is written. At top level, declarations become Lua globals unless marked `local`; top-level locals are visible to functions in the same TypeLua file. In generated Lua, top-level `local` names are hoisted to the top of the chunk.

### const

A `const` binding can be read like any other variable, but TypeLua rejects assignments to it, including compound assignments:

```tlua
const int maxLives = 3;
maxLives = 4;    // error: cannot assign to constant 'maxLives'
maxLives += 1;   // error
```

Its initializer must match its declared type. Constness is checked by the TypeLua compiler and is erased in generated Lua.

## Scope

A declaration is visible from its statement to the end of the enclosing block. Duplicate names in the same scope are rejected, but an inner block may shadow a name from an outer scope. Braces create scopes: function bodies, `if` and `while` bodies, `for` bodies, and standalone blocks (see [Statements](06-statements.md)).

```tlua
{
    int temporary = 1;
    print(temporary);
}
// 'temporary' is unknown here
```

Function parameters occupy the function's scope and may be assigned like any other mutable variable.
