# 6. Sentencias

Toda sentencia termina en `;`. Los bloques usan llaves. Las declaraciones se cubren en [Variables](04-variables.md); las expresiones en [Expresiones](07-expressions.md).

## Asignación

Los operadores de asignación admitidos son `=`, `+=`, `-=`, `*=` y `/=`. Los destinos pueden ser variables, campos de struct o tabla, índices de elementos de tupla y índices de diccionario o tabla genérica. La asignación compuesta requiere un destino numérico cuando el tipo del destino es conocido. Una variable declarada `const` no puede reasignarse. Los campos de diccionario pueden actualizarse, mientras que las listas solo pueden cambiarse reemplazando toda la lista. Los valores y elementos de tupla solo pueden cambiar si la tupla conserva su longitud original y cada posición conserva su tipo original; una `const tuple` no puede cambiar.

```tdm
int score = 0;
score = 1;
score += 1;
score -= 1;
score *= 2;
score /= 2;

list int scores = [10, 20];
scores = [..., 12];
// scores[1] = 12; // error: reemplaza toda la lista en su lugar

tuple position = (0, 10, 11);
position[1] = 5; // válido: la posición 1 sigue siendo un int

dict player = { name = "Andy", age = 15, elo = 500 };
player.age = 16;
```

## if

Una sentencia `if` toma una condición entre paréntesis y un bloque, con un bloque `else` opcional. La condición debe tener tipo `bool`.

```tdm
if (count == 0) {
    count += 1;
} else {
    count = 10;
}
```

## while

Un bucle `while` también requiere una condición `bool`. Se acepta un bucle que se ejecuta para siempre en tiempo de ejecución como `while (true) { ... }`; normalmente el anfitrión del juego proporciona su propio bucle de frames.

```tdm
while (count < 5) {
    count += 1;
}
```

## for

Los bucles `for` numéricos incluyen su valor de parada. Las formas son `for (name, stop)`, `for (name, start, stop)` y `for (name, start, stop, step)`. La primera forma por defecto usa inicio `0` y paso `1`; la segunda por defecto usa paso `1`. Todos los límites y pasos deben ser `int`. Un paso negativo cuenta hacia abajo. La variable del bucle es un `int` con alcance en el cuerpo del bucle.

```tdm
for (i, 10) {
    printf("Hello world! {i}");
}
```

Un paso cero es rechazado por el runtime de Lua.

## return

Una función void puede usar `return;`. Una función no-void debe hacer `return` de un valor de su tipo de retorno declarado. El compilador comprueba cada sentencia `return` que ve pero no realiza análisis de retorno definitivo, por lo que una función que puede terminar sin retornar no se diagnostica.

## Sentencias de expresión

Una expresión por sí sola es una sentencia, que es cómo se escriben las llamadas a funciones:

```tdm
movePlayer(20);
print("Hello!");
```

## Bloques independientes

Las llaves también pueden crear un alcance independiente:

```tdm
{
    int temporary = 1;
    print(temporary);
}
```

Este bloque independiente crea un alcance anidado y se convierte en un bloque `do ... end` de Lua. Sus declaraciones no están disponibles después de la `}` de cierre.
