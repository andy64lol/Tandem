# 7. Expresiones

## Formas

Las expresiones admitidas incluyen literales, variables, llamadas a funciones, literales de tabla y lista, acceso a campos e índices, aserciones de tipo con `as type`, paréntesis, unarios `!` y `-`, aritmética (`+ - * / %`), concatenación de cadenas (`+`, `,` y `..`), comparaciones (`== != < > <= >=`) y lógica booleana (`&& ||`). Un literal de tupla se escribe entre paréntesis en una declaración `tuple`.

Los literales de tabla usan `{ ... }`; las tablas genéricas pueden usar claves nombradas o enteras. Un `dict` usa campos nombrados dentro de `{ ... }`. Una declaración `table Key Value` comprueba el literal contra sus tipos de clave y valor: las claves `str` usan campos nombrados, mientras que las claves `int` usan valores posicionales. Los literales de lista usan `[ ... ]` y deben coincidir con su tipo de elemento declarado. Las listas se reemplazan como un todo, y `...` en un reemplazo conserva el valor antiguo de la lista en la misma posición; `...` no puede aparecer en otro lugar. Los valores de tupla usan `( ... )` en una declaración `tuple`. Su longitud y tipos posicionales quedan fijados por el primer valor, aunque los valores pueden actualizarse sin cambiar esos tipos. `as type` dentro de un inicializador de tupla solo se permite para `const tuple`, que bloquea toda la tupla. Los diccionarios usan claves de cadena, las listas usan índices enteros para lecturas y las tablas genéricas aceptan claves enteras o de cadena. Los valores leídos de una tabla genérica conservan los tipos de su inicializador literal cuando se conocen; los valores leídos de diccionarios, de tablas genéricas sin forma literal y de módulos dinámicos son dinámicos. Los elementos de lista y las posiciones de tupla en índices constantes conservan sus tipos. Un valor dinámico necesita una aserción explícita `as type` antes de poder asignarse a una variable, campo o parámetro tipado. `as` aserta el tipo pero no convierte ni valida el valor en tiempo de ejecución.

```tdm
list int values = [4, 8, 15];
int first = values[1];
table config = { retries = 3 };
print(config.retries);

dict player = { name = "Andy", age = 15, elo = 500 };
tuple position = (0, 10, 11);
tuple position = (10, 20, 30);
int x = position[1];
```

## Precedencia de operadores

De enlace más débil a más fuerte:

| Operadores | Asociatividad |
| --- | --- |
| `,` | izquierda |
| `\|\|` | izquierda |
| `&&` | izquierda |
| `==` `!=` | izquierda |
| `<` `>` `<=` `>=` | izquierda |
| `+` `-` `..` | izquierda |
| `*` `/` `%` | izquierda |
| unario `!` `-` | prefijo |

Los paréntesis agrupan subexpresiones como siempre. El operador coma es el de enlace más débil de todos, por lo que una concatenación con coma pasada como argumento de función necesita paréntesis.

## Aritmética

Los operandos aritméticos deben tener el mismo tipo numérico; no se mezclan `int` y `float`, y no hay conversiones implícitas. El unario `-` niega un `int` o un `float`. `%` es el operador de resto de Lua sobre el tipo numérico elegido.

## Cadenas

`+`, `,` y `..` unen dos cadenas, además de sumar valores numéricos coincidentes; `..` es el operador de concatenación de Lua. Por ejemplo:

```tdm
str greeting = "Hello", " world";
str name = "Type" + "Lua";
str file = "notes" .. ".txt";
```

Los tres concatenan. Como las comas también separan argumentos de función, envuelve una expresión de concatenación con coma en paréntesis al pasarla como argumento: `print(("Hello", " world"));`.

## Comparaciones y lógica

La igualdad requiere tipos coincidentes no-void. Las comparaciones ordenadas (`< > <= >=`) admiten operandos `int`, `float` o `str` coincidentes y producen un `bool`. Los operadores booleanos `&&` y `||` requieren operandos booleanos, igual que el unario `!`.

## Funciones built-in

### print

El built-in `print(...)` acepta cualquier número de argumentos de cualquier tipo y se mapea al `print` de Lua.

### printf

`printf` acepta un literal de cadena y lo imprime con interpolación `{variable}`. Los marcadores de posición deben nombrar variables en alcance. Las llaves dobles producen llaves literales, como en `printf("{{name}} is {playerName}")`. Un `}` sin pareja o un marcador que no sea un nombre de variable es un error de compilación.

```tdm
int i = 1;
printf("Hello world! {i}");
// imprime: Hello world! 1
```

El Lua generado construye la salida concatenando el texto con cada variable convertida por `tostring`.

## Importación de la biblioteca estándar

Los paquetes de la biblioteca estándar de Lua no están en alcance por defecto. Cada paquete debe importarse en el nivel superior antes de usarse:

```tdm
import string, math;

str upper = string.upper("lua");
float root = math.sqrt(16.0);
int length = string.len(upper);
```

`import` puede listar varios paquetes separados por comas y solo se permite en el nivel superior. Los paquetes admitidos son `string`, `table`, `math`, `io`, `os`, `coroutine`, `utf8`, `debug` y `package`. Importar un paquete desconocido es un error: `Unrecognised package 'nope'`.

Los miembros comunes de los paquetes tienen firmas estáticas. Por ejemplo, `string.upper` y `string.format` devuelven `str`, `string.len` devuelve `int` y `math.sqrt` devuelve `float`. Los argumentos y valores de retorno se comprueban como cualquier otra llamada a función. Los miembros sin firma siguen siendo dinámicos y requieren una aserción explícita `as type`. Un miembro desconocido es un error como `package 'string' has no member 'nope'`. Un nombre de paquete usado sin su importación informa `Unrecognised package 'string': import it with 'import string;'`.

Las funciones base de Lua (`assert`, `pairs`, `ipairs`, `pcall`, `tonumber`, `tostring`, `type` y otras) y los valores base (`_G`, `_VERSION`) siguen la misma regla: `import base;` los trae al alcance. Las funciones base de uso frecuente también tienen firmas estáticas, como `tostring(any) -> str`, `type(any) -> str`, `rawlen(any) -> int` y `_VERSION -> str`. Los built-ins de Tandem `print` y `printf` descritos arriba siempre están disponibles.
