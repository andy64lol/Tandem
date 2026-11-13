# 3. Tipos

Los tipos primitivos y los tipos de colección se comprueban en tiempo de compilación y se borran en el Lua generado. Los tipos struct conservan los nombres de sus campos en tiempo de ejecución como tablas Lua ordinarias.

| Tipo | Valores | Representación en Lua |
| --- | --- | --- |
| `int` | Literales enteros como `42` | `number` |
| `float` | Literales decimales como `3.5` | `number` |
| `bool` | `true` o `false` | `boolean` |
| `str` | Cadenas entre comillas dobles | `string` |
| `table` | Una tabla Lua con claves y valores de tipo dinámico | `table` |
| `table Key Value` | Una tabla tipada cuyas claves tienen tipo `Key` (`str` o `int`) y cuyos valores tienen tipo `Value` | `table` |
| `dict` | Valores nombrados de tipo dinámico | `table` |
| `list Type` | Valores ordenados que todos tienen el tipo indicado | `table` |
| `tuple` | Valores ordenados con longitud fija y tipo fijo en cada posición | `table` |
| `void` | Sin valor de retorno; solo tipo de retorno de función | `nil` |
| nombre de clase | Una instancia creada con `new` | tabla con metatabla |

No hay conversiones implícitas. `int` y `float` son tipos estáticos distintos, aunque ambos usen números de Lua en tiempo de ejecución.

`auto` no es un tipo por sí mismo: infiere el tipo de una variable a partir de su inicializador y lo bloquea. Es válido solo en una declaración de variable con inicializador. Consulta [Variables](04-variables.md#inferencia-automática-de-tipos).

Una declaración puede usar el tipo `module` para enlazar un archivo requerido: `module greetings = require("greetings.tdm");`. El compilador infiere los miembros exportados del módulo a partir de ese archivo. Un tipo `module` suelto fuera de tal declaración se rechaza.

Las listas usan un tipo después de `list`, por ejemplo `list int scores = [10, 20, 30];`. Los elementos de la lista deben coincidir con ese tipo. Las listas usan corchetes e índices basados en uno de Lua. No uses `int[]`; las llaves son para tablas y diccionarios, y los paréntesis para tuplas.

Una `table` simple es una tabla Lua genérica cuyas claves y valores son dinámicos. Una tabla genérica inicializada con un literal de tabla conserva los tipos estáticos de sus campos nombrados y valores posicionales para lecturas a través de esa variable: `table t = { count = 1 }; int n = t.count;` se comprueba. Asignar un campo o índice conocido valida el tipo de su valor. Reasignar toda la tabla hace que las lecturas posteriores sean dinámicas. `table Key Value` es una tabla estáticamente tipada: `Key` debe ser `str` o `int`, y cada lectura o escritura se comprueba contra `Value`. Con claves `str`, los literales usan campos nombrados y los valores se pueden leer tanto con `t.name` como con `t["name"]`. Con claves `int`, los literales son posicionales y reciben las claves 1, 2, 3, etc. Las tablas tipadas siguen compilando a tablas Lua ordinarias.

```tdm
table str int ages = { alice = 21, bob = 32 };
int bob = ages.bob;
ages["carol"] = 17;
ages.dave = 4;

table int str names = { "zero", "one", "two" };
str one = names[1];
```

Usa `dict` para claves nombradas, `list Type` para valores ordenados y redimensionables de un solo tipo, y `tuple` para un grupo ordenado cuya longitud y tipos posicionales se infieren de su primer valor. Estas colecciones compilan a tablas Lua. Los valores leídos de una `table` genérica conservan los tipos de su inicializador literal cuando se conocen; los valores leídos de un `dict`, de una `table` genérica sin forma literal y los exportados por módulos dinámicos no tienen tipo estático conocido. Los valores dinámicos requieren una aserción explícita `as type` para asignarlos a una variable, campo o parámetro tipado estáticamente. Las lecturas de listas conservan el tipo de elemento de la lista y las lecturas de tuplas en un índice constante conservan el tipo de esa posición. Una aserción no convierte ni valida el valor en tiempo de ejecución.

Los structs definen campos nombrados y sus tipos:

```tdm
struct Point {
    float x;
    float y;
}

Point origin = { x = 0.0, y = 0.0 };
origin.x = 2.0;
```

Los inicializadores de struct deben proporcionar cada campo exactamente una vez con un tipo de valor coincidente. Las declaraciones de struct son de nivel superior. Sus valores compilan a tablas Lua.

Un nombre de clase también es un tipo. Las instancias de clase se crean con `new` y llevan tipos de campo y método; los valores de clase no pueden inicializarse con literales de tabla. Consulta [Clases](12-classes.md).

```tdm
table options = { debug = true, retries = 3 };
print(options["debug"]);

dict player = { name = "Maria", age = 35, points = 500 };
player.age = 16;
list int scores = [10, 20, 30];
scores = [..., 12, ...];
tuple position = (0, 10, 11);
tuple position = (4, 10, 11); // misma longitud y tipos posicionales
position[1] = 5;
int x = position[1];
```

Las listas y tuplas usan la indexación basada en uno de Lua. Una lista solo puede cambiarse asignando una lista de reemplazo; la asignación a un índice de lista se rechaza. En un reemplazo de lista, cada `...` conserva el valor antiguo en esa posición, y `...` solo es válido en listas. Las tuplas pueden reasignarse o actualizarse por índice solo manteniendo la longitud original y el tipo en cada posición. `as type` dentro de un inicializador de tupla solo se permite para `const tuple`, que bloquea toda la tupla. Una variable no puede hacerse `const` después de su primera declaración. Las listas y diccionarios solo pueden redefinirse mientras su variable sea mutable; una redefinición de lista conserva su tipo de elemento.

## Literales

- Literales enteros: `42`, `0`, `100`.
- Literales float: `3.5`, `9.8`. Los literales decimales necesitan dígitos a ambos lados del punto decimal, así que escribe `3.5`, no `3.` ni `.5`.
- Literales de cadena: entre comillas dobles, con los escapes `\n`, `\r`, `\t`, `\"` y `\\`.
- Literales booleanos: `true` y `false`.

## void

`void` es el tipo de retorno de las funciones que no devuelven nada. Una variable no puede tener tipo `void`; el compilador rechaza `void x = ...;`. Si una declaración de función omite `-> type`, el tipo de retorno por defecto es `void`.

## Comprobación de tipos

Cada expresión y sentencia se comprueba: los inicializadores deben coincidir con los tipos declarados, los operandos deben tener tipos compatibles, los argumentos deben coincidir con los tipos de los parámetros y los valores de retorno deben coincidir con el tipo de retorno declarado. Las anotaciones de tipo y los marcadores `const`/`local` existen solo en el código fuente; el Lua generado contiene valores y nombres simples.
