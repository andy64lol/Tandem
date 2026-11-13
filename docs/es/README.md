# El libro de Tandem

Tandem es un lenguaje experimental, estáticamente tipado y de estilo C que transpila a Lua 5.4. Compila archivos fuente `.tdm` a Lua puro y puede ejecutar tanto el Lua generado como archivos `.lua` ordinarios a través de su runtime embebido. No se necesita un ejecutable de Lua separado.

Este libro es la referencia completa de Tandem 0.2. Los capítulos están ordenados para que un lector nuevo aprenda el lenguaje desde el principio y pueda saltar a los capítulos de referencia cuando lo necesite.

## Parte I — Primeros pasos

1. [Introducción](01-introduction.md): qué es Tandem, cómo funciona la compilación y un primer vistazo a la sintaxis.
2. [Primeros pasos](02-getting-started.md): compila el compilador, ejecuta un primer programa y comprueba un archivo sin ejecutarlo.

## Parte II — El lenguaje

3. [Tipos](03-types.md): tipos primitivos, tablas, diccionarios, listas, tuplas, structs y módulos.
4. [Variables](04-variables.md): declaraciones, modificadores `local` y `const`, inferencia `auto` y alcance.
5. [Funciones](05-functions.md): declaraciones, parámetros, valores por defecto y tipos de retorno.
6. [Sentencias](06-statements.md): asignación, `if`, `while`, `for`, `return` y bloques.
7. [Expresiones](07-expressions.md): operadores, precedencia, importación de la biblioteca estándar y los built-ins `print` y `printf`.
8. [Comentarios](08-comments.md): formas de comentarios estilo C y Lua.
9. [Clases](12-classes.md): campos tipados, métodos `fn`, `self`, constructores, instancias con `new` y herencia simple.

## Parte III — Herramientas

10. [Diagnósticos](09-diagnostics.md): errores, advertencias y por qué una sola advertencia bloquea la compilación.
11. [Línea de comandos](10-cli.md): todos los modos de la CLI, rutas de salida, argumentos y códigos de salida.
12. [API de Rust](11-rust-api.md): embebe el compilador y el runtime de Lua en un programa Rust.

## Parte IV — Estado

13. [Limitaciones](13-limitations.md): lo que Tandem todavía no hace.

La [lista de tareas del proyecto](../todo.md) sigue el trabajo planificado de lenguaje, herramientas y runtime.
