# 9. Diagnósticos

Tandem reporta dos tipos de diagnóstico: **errores** y **advertencias**. Ambos detienen la compilación. Si el compilador produce aunque sea un diagnóstico, no se genera, escribe ni ejecuta ningún Lua.

## Errores

Un error es una violación del lenguaje: un error de sintaxis, una discordancia de tipos, un nombre desconocido, una asignación inválida, etc. Toda sentencia debe terminar en `;`, por lo que omitirlo es un error:

```tdm
int x = 1
// error: expected ';'
```

Los errores de sintaxis son recuperables. Tras una sentencia o declaración malformada, el parser avanza hasta el siguiente límite de sentencia, por lo que una sola compilación puede reportar varios errores de sintaxis con sus ubicaciones. Si el análisis tiene éxito, se comprueba el tipo de cada módulo y se reporta el primer error de cada módulo.

Todo error de parser, lexer y tipo incluye línea y columna. Las ubicaciones usan líneas y columnas basadas en uno.

## Advertencias

Una advertencia señala código que es legal pero sospechoso. Tandem es deliberadamente estricto: una sola advertencia es fatal, por lo que las advertencias se comportan como errores y bloquean la generación de código. Las comprobaciones de advertencia actuales son:

- una variable oculta una declaración en un alcance exterior;
- una variable local o parámetro nunca se usa;
- una sentencia es inalcanzable porque sigue a un `return`.

```tdm
fn f() -> int {
    int unused = 1;   // warning: unused variable 'unused'
    return unused;
}
```

El compilador recopila todas las advertencias del programa y las reporta juntas. Los marcadores de posición de `printf("{name}")` cuentan como usos, por lo que una variable usada solo dentro de un formato `printf` no se reporta.

## Reporte

Cada diagnóstico se imprime con su severidad, su mensaje y la ruta fuente con la ubicación:

```
error: expected ';', found 'int' at line 2, column 1
 --> examples/hello.tdm:2:1

warning: unused variable 'unused'
 --> examples/hello.tdm
```

Los mensajes incluyen el tipo de token que se encontró donde el compilador esperaba otra cosa, por ejemplo `expected ';', found '}'` o `expected expression, found ';'`. Los mensajes de estado y diagnóstico usan colores ANSI cuando el flujo de salida es una terminal.

Los errores de tipo usan la misma forma. Por ejemplo, un retorno faltante reporta la ubicación de la declaración de la función:

```
error: function 'f' must return a value on every path at line 1, column 1
 --> examples/missing-return.tdm:1:1
```

Como las advertencias bloquean la generación de código, `tandem` no escribe ningún archivo `.lua` y tanto `tandem` como `tandem --check` salen con un estado distinto de cero cuando hay un diagnóstico presente. La API de Rust devuelve los diagnósticos como un `Vec<Diagnostic>`; consulta [API de Rust](11-rust-api.md).

## Mapas de fuente

El Lua generado mantiene un enlace de vuelta al código fuente de Tandem. Cada sentencia y firma de función generada termina con un comentario de Lua que nombra la línea y columna originales:

```lua
x = 1 -- tdm:1:1
if (x > 0) then -- tdm:2:1
    print("yes") -- tdm:3:5
end
```

El marcador es `-- tdm:<line>:<column>`. Los editores y herramientas de depuración pueden analizarlo para mapear una línea de Lua generado de vuelta al archivo `.tdm`. Las sentencias anidadas llevan sus propios marcadores, por lo que el marcador más interno en una línea es la ubicación más precisa.
