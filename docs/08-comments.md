# 8. Comments

Tandem supports C-style line and block comments:

```tdm
// A line comment
/* A block comment
   can span multiple lines. */
```

Lua comment forms are accepted too: `--` for a line comment and Lua long comments such as `--[[ ... ]]` or `--[=[ ... ]=]`. The number of `=` characters in a long comment's opening and closing delimiters must match. Block comments do not nest.

These comment forms are accepted in `.tdm` files. Other Lua syntax belongs in a `.lua` file, which Tandem can execute directly; `.tdm` files otherwise follow the Tandem grammar described in this book.
