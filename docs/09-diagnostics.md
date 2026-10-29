# 9. Diagnostics

Tandem reports two kinds of diagnostic: **errors** and **warnings**. Both stop compilation. If the compiler produces even one diagnostic, no Lua is generated, written, or run.

## Errors

An error is a violation of the language: a syntax error, a type mismatch, an unknown name, an invalid assignment, and so on. Every statement must end in `;`, so omitting one is an error:

```tdm
int x = 1
// error: expected ';'
```

Syntax errors are recoverable. After a malformed statement or declaration the parser skips forward to the next statement boundary, so a single compile can report several syntax errors with their locations. If parsing succeeds, each module is type-checked and the first error in each module is reported.

Every parser, lexer, and type error includes a line and column. Locations use one-based lines and columns.

## Warnings

A warning flags code that is technically legal but suspicious. Tandem is deliberately strict: a single warning is fatal, so warnings behave like errors and block code generation. The current warning checks are:

- a variable shadows a declaration in an outer scope;
- a local variable or parameter is never used;
- a statement is unreachable because it follows a `return`.

```tdm
fn f() -> int {
    int unused = 1;   // warning: unused variable 'unused'
    return unused;
}
```

The compiler collects every warning in a program and reports them together. `printf("{name}")` placeholders count as uses, so a variable used only inside a `printf` format is not reported.

## Reporting

Each diagnostic is printed with its severity, its message, and the source path with the location:

```
error: expected ';', found 'int' at line 2, column 1
 --> examples/hello.tdm:2:1

warning: unused variable 'unused'
 --> examples/hello.tdm
```

Messages include the kind of token that was found where the compiler expected something else, for example `expected ';', found '}'` or `expected expression, found ';'`. Status messages and diagnostics are colored when the output stream is a terminal.

Type errors use the same form. For example, a missing return reports the function's declaration location:

```
error: function 'f' must return a value on every path at line 1, column 1
 --> examples/missing-return.tdm:1:1
```

Because warnings block code generation, `tandem` writes no `.lua` file and both `tandem` and `tandem --check` exit with a nonzero status whenever a diagnostic is present. The Rust API returns diagnostics as a `Vec<Diagnostic>`; see [Rust API](11-rust-api.md).

## Source maps

Generated Lua keeps a link back to the Tandem source. Each generated statement and function signature ends with a Lua comment naming the original line and column:

```lua
x = 1 -- tdm:1:1
if (x > 0) then -- tdm:2:1
    print("yes") -- tdm:3:5
end
```

The marker is `-- tdm:<line>:<column>`. Editors and debugging tools can parse it to map a generated Lua line back to the `.tdm` file. Nested statements carry their own markers, so the innermost marker on a line is the most precise location.
