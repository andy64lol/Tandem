# 5. Funciones

## Declaraciones

Declara funciones con `fn`, parámetros tipados y una anotación de retorno opcional:

```tdm
fn add(int a, int b) -> int {
    return a + b;
}

fn hello() {
    print("Hello!");
}
```

Si se omite `-> type`, el tipo de retorno por defecto es `void`. El `return` de una función no-void debe incluir un valor del tipo declarado. Una función void puede usar `return;`. Cada ruta de control de una función no-void debe devolver un valor. La comprobación entiende `return`, bloques anidados, sentencias `if` cuyas ramas ambas retornan y bucles `while (true)`. Una función que puede caer hasta su final informa `function 'name' must return a value on every path`.

## Valores por defecto de los parámetros

Los valores por defecto de los parámetros deben ir al final; se rechaza un parámetro requerido después de uno con valor por defecto. Las llamadas pueden omitir los parámetros con valor por defecto finales. El Lua generado los rellena cuando el parámetro es `nil`:

```tdm
fn greet(str name = "world") -> void {
    print("Hello, " + name);
}

greet();         // usa "world"
greet("Ada");    // usa "Ada"
```

Los valores por defecto se comprueban de tipo y se evalúan de izquierda a derecha, por lo que un valor por defecto puede referirse a parámetros anteriores. En esta etapa, una expresión por defecto debería ser un literal u otra expresión válida en el alcance del parámetro.

## Llamadas y declaraciones adelantadas

El número de argumentos debe estar entre el número de parámetros requeridos y el número total de parámetros, y cada argumento debe tener el tipo de parámetro declarado.

Las declaraciones de función se recopilan antes de comprobar los cuerpos, por lo que las funciones pueden llamar a declaraciones que aparecen más adelante en el código fuente. Los nombres de función no pueden duplicarse entre sí ni con los nombres reservados `print` y `printf`.

Las funciones no son valores en 0.1: no pueden almacenarse en variables, pasarse como argumentos ni devolverse. Consulta [Limitaciones](13-limitations.md). Los métodos de clase usan la misma sintaxis `fn` pero se ejecutan con un `self` implícito; consulta [Clases](12-classes.md).
