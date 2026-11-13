# 8. Comentarios

Tandem admite comentarios de línea y bloque estilo C:

```tdm
// Un comentario de línea
/* Un comentario de bloque
   puede abarcar varias líneas. */
```

También se aceptan las formas de comentario de Lua: `--` para un comentario de línea y comentarios largos de Lua como `--[[ ... ]]` o `--[=[ ... ]=]`. El número de caracteres `=` en los delimitadores de apertura y cierre de un comentario largo debe coincidir. Los comentarios de bloque no se anidan.

Estas formas de comentario se aceptan en archivos `.tdm`. El resto de la sintaxis de Lua pertenece a un archivo `.lua`, que Tandem puede ejecutar directamente; los archivos `.tdm` siguen en lo demás la gramática de Tandem descrita en este libro.
