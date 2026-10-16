# 8. Comments

TypeLua supports C-style line and block comments:

```tlua
// A line comment
/* A block comment
   can span multiple lines. */
```

Lua comment forms are accepted too: `--` for a line comment and Lua long comments such as `--[[ ... ]]` or `--[=[ ... ]=]`. The number of `=` characters in a long comment's opening and closing delimiters must match. Block comments do not nest.

These comment forms are accepted in `.tlua` files. Other Lua syntax belongs in a `.lua` file, which TypeLua can execute directly; `.tlua` files otherwise follow the TypeLua grammar described in this book.
