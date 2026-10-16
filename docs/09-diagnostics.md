# 9. Diagnostics

TypeLua reports two kinds of diagnostic: **errors** and **warnings**. Both stop compilation. If the compiler produces even one diagnostic, no Lua is generated, written, or run.

## Errors

An error is a violation of the language: a syntax error, a type mismatch, an unknown name, an invalid assignment, and so on. Every statement must end in `;`, so omitting one is an error:

```tlua
int x = 1
// error: expected ';'
```

The compiler stops at the first error.

## Warnings

A warning flags code that is technically legal but suspicious. TypeLua is deliberately strict: a single warning is fatal, so warnings behave like errors and block code generation. The current warning checks are:

- a variable shadows a declaration in an outer scope;
- a local variable or parameter is never used;
- a statement is unreachable because it follows a `return`.

```tlua
fn f() -> int {
    int unused = 1;   // warning: unused variable 'unused'
    return unused;
}
```

The compiler collects every warning in a program and reports them together. `printf("{name}")` placeholders count as uses, so a variable used only inside a `printf` format is not reported.

## Reporting

Each diagnostic is printed with its severity:

```
TypeLua error: expected ';' at byte 10
TypeLua warning: unused variable 'unused'
```

Because warnings block code generation, `typelua` writes no `.lua` file and both `typelua` and `typelua --check` exit with a nonzero status whenever a diagnostic is present. The Rust API returns diagnostics as a `Vec<Diagnostic>`; see [Rust API](11-rust-api.md).
