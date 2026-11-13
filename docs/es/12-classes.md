# 12. Clases

Las clases agrupan campos tipados con métodos. Define una con `class Name { ... }`:

```tdm
class player {
    str name = "Ada";
    int health = 100;

    constructor(str name) {
        self.name = name;
    }

    fn isAlive() -> bool {
        return health > 0;
    }

    fn damage(int amount) {
        health -= amount;
    }
}
```

Los métodos conservan la palabra clave `fn`. Se ejecutan con una instancia implícita, disponible como `self`, que no puede reasignarse. Tandem no usa una palabra clave `this`. Los campos pueden leerse y asignarse directamente por nombre; escribe `self.health` cuando un parámetro o variable local tenga el mismo nombre. Los nombres de campo y método no pueden colisionar.

Los campos se declaran como variables tipadas y normalmente necesitan un inicializador. El valor por defecto se evalúa para cada nueva instancia antes de que se ejecute el cuerpo del constructor; en una clase derivada, los valores por defecto de los campos del hijo se ejecutan después de los constructores base y antes del resto del constructor del hijo. Un campo genérico como `T value;` puede omitir su inicializador para que el constructor lo establezca. Una clase puede definir como máximo un `constructor`, y los constructores no pueden declarar un tipo de retorno.

## Crear instancias

`new` crea una instancia. Los paréntesis son opcionales cuando no hay argumentos:

```tdm
local class Player = new player;
local class Rival = new player("Grace");
```

`class` es un tipo de declaración especial: el compilador distingue una definición de clase (`class player { ... }`) de una declaración de instancia (`class Player = new player;`) por la `{` o el `=` después del nombre. La variable toma el tipo de clase estático de la expresión `new`. Se pueden usar `local` y `const` como en otras declaraciones. Llamar a un método usa la misma sintaxis de punto que otros miembros; Tandem emite la llamada a método de Lua con `:`:

```tdm
print(Player.name);
print(Player.isAlive());
Player.damage(30);
print(Player.isAlive());
```

Los nombres de clase también pueden escribirse directamente como tipos de parámetro, retorno o variable:

```tdm
fn alive(player p) -> bool {
    return p.isAlive();
}
```

## Miembros estáticos y privados

Usa `static` para campos y métodos de clase. Los métodos estáticos no tienen `self` implícito; accede a los campos estáticos a través del nombre de la clase:

```tdm
class Counter {
    static int value = 0;

    static fn increment() -> int {
        Counter.value += 1;
        return Counter.value;
    }
}

int current = Counter.increment();
```

Los campos estáticos se inicializan cuando se carga la clase. Los campos y métodos estáticos se heredan de las bases de clase.

Usa `private` antes de un campo o método para restringir el acceso a la clase que lo declara:

```tdm
class Account {
    private int balance = 0;

    private fn adjust(int amount) {
        balance += amount;
    }

    fn deposit(int amount) {
        self.adjust(amount);
    }
}
```

La privacidad se comprueba por el compilador. Los nombres existentes, incluidos los nombres estilo Lua con prefijo de guion bajo como `_balance`, conservan su visibilidad y comportamiento existentes.

## Genéricos

Las funciones genéricas infieren los argumentos de tipo a partir de los argumentos de la llamada. Las clases genéricas requieren argumentos de tipo explícitos al construirse y al usarse como tipos:

```tdm
fn identity<T>(T value) -> T {
    return value;
}

class Box<T> {
    T value;

    constructor(T value) {
        self.value = value;
    }

    fn get() -> T {
        return value;
    }
}

Box<int> box = new Box<int>(identity(42));
int value = box.get();
```

Los parámetros genéricos pueden aparecer dentro de tipos de colección existentes, por ejemplo `list T` o `table str T`. Los tipos de clase genérica son invariantes: `Box<int>` y `Box<float>` son tipos distintos.

Las clases no se crean a partir de literales de tabla. `player { name = "Ada", health = 100 }` se rechaza; usa `new player("Ada")` en su lugar.

## Herencia

Una clase puede extender varias clases e interfaces. La primera base de clase concreta es la base primaria para `super(...)`, `super.method(...)` y los argumentos del constructor; las bases concretas posteriores se inicializan en orden y no deben tener argumentos de constructor requeridos. Los ancestros compartidos se inicializan una sola vez. Las interfaces añaden requisitos de métodos pero ninguna tabla base en tiempo de ejecución.

```tdm
class enemy {
    str name = "enemy";
    int health = 10;

    constructor(str name) {
        self.name = name;
    }

    fn damage(int amount) {
        health -= amount;
    }
}

class boss extends enemy {
    int phase = 1;

    constructor(str name, int phase) {
        super(name);
        self.phase = phase;
    }

    fn damage(int amount) {
        super.damage(amount);
        phase += 1;
    }
}
```

Separa las bases concretas con comas. Una interfaz puede listarse en `extends` o en `implements`; ambas formas requieren que la clase proporcione cada método declarado con una firma coincidente. Las interfaces pueden extender otras interfaces:

```tdm
interface Named {
    fn name() -> str;
}

interface Renderable extends Named {
    fn render() -> str;
}

class Sprite extends Asset, Drawable implements Renderable {
    fn name() -> str { return "sprite"; }
    fn render() -> str { return "image"; }
}
```

El hijo hereda campos, métodos y miembros estáticos de cada base de clase concreta. Si las bases proporcionan el mismo método, gana la primera base con ese método; se rechazan los tipos de campo heredados en conflicto. Los campos declarados en el hijo no deben reutilizar un nombre heredado con un tipo diferente, y sobrescribir un método requiere los mismos tipos de parámetro y retorno. Un campo y un método no pueden compartir un nombre en ninguna parte de la cadena de herencia.

Si el hijo declara un constructor, su primera sentencia debe llamar a `super(...)` con argumentos para el constructor de la base primaria. El compilador inicializa entonces las bases concretas posteriores antes de los campos del hijo y el resto del cuerpo del constructor. Si el hijo no declara un constructor, su constructor generado acepta los argumentos del constructor de la base primaria e inicializa cada base antes de los propios campos del hijo. Los ancestros compartidos se ejecutan una vez, incluida la herencia en forma de diamante.

Usa `super.method(...)` para llamar a la implementación de la base primaria de un método sobrescrito. `super` no puede usarse en una clase sin una base concreta y solo está disponible dentro de métodos de instancia y constructores.

Una instancia del hijo puede usarse donde se espera el tipo de la clase padre:

```tdm
fn hurt(enemy target) {
    target.damage(5);
}

hurt(new boss("Dragon", 2));
```

El despacho de métodos es dinámico, por lo que esto llama a `boss.damage` aunque el parámetro esté tipado como `enemy`.

## Lua generado

Las clases compilan a tablas y metatablas Lua ordinarias. La tabla de la clase contiene los métodos y se convierte en la metatabla `__index` de cada instancia. La construcción se divide en `.new`, que asigna la instancia, y `:__init`, que inicializa los campos y ejecuta el constructor:

```lua
local Hero, player
local __tandem_init_state = setmetatable({}, { __mode = "k" })
player = {}
player.__index = player

function player.new(name)
    local self = setmetatable({}, player)
    __tandem_init_state[self] = {}
    self:__init(name)
    return self
end

function player:__init(name)
    local initialized = __tandem_init_state[self]
    if initialized["player"] then return end
    initialized["player"] = true
    self.name = "Ada"
    self.health = 100
    self.name = name
end

function player:isAlive()
    return (self.health > 0)
end

Hero = player.new("Ada")
print(Hero:isAlive())
```

Una clase derivada apunta su metatabla a sus bases, por lo que los métodos heredados se encuentran en orden:

```lua
boss = setmetatable({}, { __index = enemy })
boss.__index = boss
```

Su `__init` llama a `super.__init(self, ...)` explícitamente, lo que mantiene la identidad de la instancia mientras ejecuta el inicializador del padre:

```lua
function boss:__init(name, phase)
    local super = enemy
    super.__init(self, name)
    self.phase = 1
    self.phase = phase
end
```

Los campos son entradas normales de tabla en la instancia. Los métodos son entradas de tabla en la clase, encontradas a través de `__index`. Tandem emite llamadas con dos puntos para los métodos de clase para que Lua pase `self` automáticamente. No se requiere ninguna biblioteca en tiempo de ejecución ni representación de valores específica de Tandem.

## Restricciones actuales

Las clases siguen siendo privadas al módulo. Los getters y setters, `instanceof`, las expresiones de clase, los nombres de miembro calculados, el acceso abstract/final/protected y las clases exportadas no están implementados todavía. Consulta [Limitaciones](13-limitations.md) y la [lista de tareas del proyecto](../todo.md).
