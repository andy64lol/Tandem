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
list int scores = [10, 20, 30];
tuple position = (0, 10, 11);
```

Every declaration needs an initializer, and the initializer must have the declared type. Reassignments are checked against the variable's current type. To deliberately change that type, annotate the new value with `as type`:

```tlua
int name = 0;
name = "Hello!" as str;
name = "TypeLua"; // now a str
// name = 1;      // error: name is now a str
```

`as` is a type assertion, not a conversion: a statically known value must already have the asserted type. For a dynamic value read from a generic `table` or `dict`, from a tuple with an unknown index, or from a dynamic module, `as type` explicitly asserts its type. The generated Lua does not validate or convert the value at runtime. If different control-flow paths leave a variable with different types, the compiler treats its type as dynamic until an explicit `as type` assertion is used. A global variable can be retyped at top level; inside a function, use `as` to read a global whose type changes in the program, and retyping the global itself is rejected.

A variable cannot be declared with type `void`.

### Collection redefinition

`list`, `dict`, and `tuple` declarations may repeat an existing collection name in the same scope to replace its value. Lists keep their declared element type. Tuples keep the length and type at each position inferred by the first declaration. Dictionaries keep the `dict` type and may contain named fields with different value types. These redefinitions are allowed only for mutable variables. A mutable variable cannot be made `const` by a later declaration.

```tlua
tuple position = (0, 10, 11);
tuple position = (10, 20, 30); // valid
// tuple position = ("Hello", 10); // error: wrong length and first element type

list int scores = [10, 20, 30];
scores = [..., 12, ...];
```

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

Its initializer must match its declared type. Constness is checked by the TypeLua compiler and is erased in generated Lua. A `const tuple` also locks its elements. Declaring an already mutable collection with `const` is an error.

## Scope

A declaration is visible from its statement to the end of the enclosing block. Duplicate names in the same scope are rejected except for the collection redefinitions described above. An inner block may shadow a name from an outer scope. Braces create scopes: function bodies, `if` and `while` bodies, `for` bodies, and standalone blocks (see [Statements](06-statements.md)).

```tlua
{
    int temporary = 1;
    print(temporary);
}
// 'temporary' is unknown here
```

Function parameters occupy the function's scope and may be assigned like any other mutable variable.
