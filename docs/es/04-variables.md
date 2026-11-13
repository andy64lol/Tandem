# 4. Variables

## Declaraciones

Las declaraciones usan `(local) (const) type name = value;`, donde los paréntesis marcan modificadores opcionales. Por ejemplo:

```tdm
int score = 0;
float gravity = 9.8;
bool alive = true;
str playerName = "Ada";
const int maxLives = 3;
local int scoreLimit = 100;
(local) (const) str edition = "standard";
list int scores = [10, 20, 30];
tuple position = (0, 10, 11);
```

Toda declaración necesita un inicializador, y el inicializador debe tener el tipo declarado. Las reasignaciones se comprueban contra el tipo actual de la variable. Para cambiar deliberadamente ese tipo, anota el nuevo valor con `as type`:

```tdm
int name = 0;
name = "Hello!" as str;
name = "Tandem"; // ahora es un str
// name = 1;      // error: name ahora es un str
```

Las instancias de clase usan la forma especial de declaración `class`: `class Player = new player;`, opcionalmente con `local` o `const`. La clase de la expresión `new` proporciona el tipo estático. Consulta [Clases](12-classes.md).

La única excepción a la regla de tipos es un enlace de módulo: `module greetings = require("greetings.tdm");` y `local greetings = require("greetings.tdm");` infieren el tipo de módulo a partir de `require`. Consulta [Primeros pasos](02-getting-started.md#dividir-un-programa-en-varios-archivos) para las importaciones de módulos.

### Inferencia automática de tipos

El tipo `auto` infiere el tipo de una variable a partir de su inicializador y luego lo bloquea, de modo que las asignaciones posteriores se comportan como si el tipo inferido se hubiera escrito explícitamente:

```tdm
auto lives = 3;        // inferido como int
lives += 1;
// lives = "three" as str; // error: no se puede cambiar el tipo de la variable 'auto' 'lives'

auto greeting = "hi " .. "there"; // inferido como str
```

Una variable `auto` no puede redefinirse y el inicializador debe tener un tipo estático. El tipo no puede inferirse de un valor dinámico como un miembro de un paquete de la biblioteca estándar, y usar `auto` en un parámetro, campo o tipo de retorno es un error; solo es válido en una declaración de variable con inicializador.

`as` es una aserción de tipo, no una conversión: un valor estáticamente conocido ya debe tener el tipo asertado. Para un valor dinámico leído de una `table` o `dict` genérica, de una tupla con índice desconocido o de un módulo dinámico, `as type` aserta explícitamente su tipo. El Lua generado no valida ni convierte el valor en tiempo de ejecución. Si distintas rutas de control dejan una variable con tipos diferentes, el compilador trata su tipo como dinámico hasta que se use una aserción explícita `as type`. Una variable global puede re-tiparse en el nivel superior; dentro de una función, usa `as` para leer una global cuyo tipo cambia en el programa, y re-tipar la global misma se rechaza.

Una variable no puede declararse con tipo `void`.

### Redefinición de colecciones

Las declaraciones de `list`, `dict` y `tuple` pueden repetir un nombre de colección existente en el mismo alcance para reemplazar su valor. Las listas conservan su tipo de elemento declarado. Las tuplas conservan la longitud y el tipo en cada posición inferidos por la primera declaración. Los diccionarios conservan el tipo `dict` y pueden contener campos nombrados con distintos tipos de valor. Estas redefiniciones solo se permiten para variables mutables. No se puede hacer `const` una variable mutable mediante una declaración posterior.

```tdm
tuple position = (0, 10, 11);
tuple position = (10, 20, 30); // válido
// tuple position = ("Hello", 10); // error: longitud y tipo del primer elemento incorrectos

list int scores = [10, 20, 30];
scores = [..., 12, ...];
```

## Los modificadores local y const

Los modificadores `local` y `const` son opcionales, mostrados aquí como `(local)` y `(const)`. Pueden aparecer en cualquier orden y escribirse con o sin paréntesis. Así, `local const int limit = 10;`, `(local) (const) int limit = 10;` y `const (local) int limit = 10;` son formas aceptadas. Escribir el mismo modificador dos veces es un error.

### local

Dentro de funciones y bloques, las declaraciones se convierten en locales de Lua tanto si se escribe `local` como si no. En el nivel superior, las declaraciones se convierten en globales de Lua a menos que se marquen como `local`; los locales de nivel superior son visibles para las funciones del mismo archivo Tandem. En el Lua generado, los nombres `local` de nivel superior se elevan a la parte superior del chunk.

### const

Un enlace `const` puede leerse como cualquier otra variable, pero Tandem rechaza las asignaciones a él, incluidas las compuestas:

```tdm
const int maxLives = 3;
maxLives = 4;    // error: no se puede asignar a la constante 'maxLives'
maxLives += 1;   // error
```

Su inicializador debe coincidir con su tipo declarado. La constancia se comprueba por el compilador de Tandem y se borra en el Lua generado. Un `const tuple` también bloquea sus elementos. Declarar una colección ya mutable con `const` es un error.

## Alcance

Una declaración es visible desde su sentencia hasta el final del bloque que la contiene. Se rechazan nombres duplicados en el mismo alcance, excepto las redefiniciones de colección descritas arriba. Un bloque interno puede ocultar un nombre de un alcance exterior. Las llaves crean alcances: cuerpos de funciones, cuerpos de `if` y `while`, cuerpos de `for` y bloques independientes (consulta [Sentencias](06-statements.md)).

```tdm
{
    int temporary = 1;
    print(temporary);
}
// 'temporary' es desconocido aquí
```

Los parámetros de función ocupan el alcance de la función y pueden asignarse como cualquier otra variable mutable.
