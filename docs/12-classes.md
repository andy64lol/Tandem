# 12. Classes

Classes group typed fields with methods. Define one with `class Name { ... }`:

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

Methods keep the `fn` keyword. They run with an implicit instance, available as `self`, which cannot be reassigned. Tandem does not use a `this` keyword. Fields can be read and assigned directly by name; write `self.health` when a parameter or local variable has the same name. Field and method names cannot collide.

Fields are declared like typed variables and normally need an initializer. The default is evaluated for every new instance before the constructor body runs; in a derived class, the child's field defaults run after base constructors and before the rest of the child constructor. A generic field such as `T value;` may omit its initializer so the constructor can set it. A class may define at most one `constructor`, and constructors cannot declare a return type.

## Creating instances

`new` creates an instance. Parentheses are optional when there are no arguments:

```tdm
local class Player = new player;
local class Rival = new player("Grace");
```

`class` is a special declaration type: the compiler distinguishes a class definition (`class player { ... }`) from an instance declaration (`class Player = new player;`) by the `{` or `=` after the name. The variable takes the static class type of the `new` expression. `local` and `const` may be used as for other declarations. Calling a method uses the same dot syntax as other members; Tandem emits the Lua method call with `:`:

```tdm
print(Player.name);
print(Player.isAlive());
Player.damage(30);
print(Player.isAlive());
```

Class names can also be written directly as parameter, return, or variable types:

```tdm
fn alive(player p) -> bool {
    return p.isAlive();
}
```

## Static and private members

Use `static` for class fields and methods. Static methods have no implicit `self`; access static fields through the class name:

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

Static fields are initialized when the class is loaded. Static fields and methods are inherited from class bases.

Use `private` before a field or method to restrict access to the class that declares it:

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

Privacy is checked by the compiler. Existing names, including underscore-prefixed Lua-style names such as `_balance`, keep their existing visibility and behavior.

## Generics

Generic functions infer type arguments from their call arguments. Generic classes require explicit type arguments when constructed and when used as types:

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

Generic parameters can appear inside existing collection types, for example `list T` or `table str T`. Generic class types are invariant: `Box<int>` and `Box<float>` are distinct types.

Classes are not created from table literals. `player { name = "Ada", health = 100 }` is rejected; use `new player("Ada")` instead.

## Inheritance

A class can extend multiple classes and interfaces. The first concrete class base is the primary base for `super(...)`, `super.method(...)`, and constructor arguments; later concrete bases are initialized in order and must have no required constructor arguments. Shared ancestors are initialized once. Interfaces add method requirements but no runtime base table.

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

Separate concrete bases with commas. An interface can be listed in `extends` or in `implements`; both forms require the class to provide every declared method with a matching signature. Interfaces may extend other interfaces:

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

The child inherits fields, methods, and static members from every concrete class base. If bases provide the same method, the first base with that method wins; conflicting inherited field types are rejected. Fields declared in the child must not reuse an inherited name with a different type, and overriding a method requires the same parameter and return types. A field and a method cannot share a name anywhere in the inheritance chain.

If the child declares a constructor, its first statement must call `super(...)` with arguments for the primary base constructor. The compiler then initializes later concrete bases before the child's fields and remaining constructor body. If the child does not declare a constructor, its generated constructor accepts the primary base's constructor arguments and initializes each base before the child's own fields. Shared ancestors run once, including in diamond-shaped inheritance.

Use `super.method(...)` to call the primary base implementation of an overridden method. `super` cannot be used in a class without a concrete base, and it is only available inside instance methods and constructors.

A child instance can be used wherever the parent class type is expected:

```tdm
fn hurt(enemy target) {
    target.damage(5);
}

hurt(new boss("Dragon", 2));
```

Method dispatch is dynamic, so this calls `boss.damage` even though the parameter is typed `enemy`.

## Generated Lua

Classes compile to ordinary Lua tables and metatables. The class table holds the methods and becomes the `__index` metatable for each instance. Construction is split into `.new`, which allocates the instance, and `:__init`, which initializes fields and runs the constructor:

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

A derived class points its metatable at its bases, so inherited methods are found in order:

```lua
boss = setmetatable({}, { __index = enemy })
boss.__index = boss
```

Its `__init` calls `super.__init(self, ...)` explicitly, which keeps the instance identity while running the parent initializer:

```lua
function boss:__init(name, phase)
    local super = enemy
    super.__init(self, name)
    self.phase = 1
    self.phase = phase
end
```

Fields are normal table entries on the instance. Methods are table entries on the class, found through `__index`. Tandem emits colon calls for class methods so Lua passes `self` automatically. No runtime library or Tandem-specific value representation is required.

## Current restrictions

Classes remain module-private. Getters and setters, `instanceof`, class expressions, computed member names, abstract/final/protected access, and exported classes are not implemented yet. See [Limitations](13-limitations.md) and the [project TODO list](../todo.md).
