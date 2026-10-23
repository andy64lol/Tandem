# 12. Classes

Classes group typed fields with methods. Define one with `class Name { ... }`:

```tlua
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

Methods keep the `fn` keyword. They run with an implicit instance, available as `self`, which cannot be reassigned. TypeLua does not use a `this` keyword. Fields can be read and assigned directly by name; write `self.health` when a parameter or local variable has the same name. Field and method names cannot collide.

Fields are declared like typed variables and must have an initializer. The default is evaluated for every new instance before the constructor body runs; in a derived class, the child's field defaults run after `super(...)` and before the rest of the child constructor. A class may define at most one `constructor`, and constructors cannot declare a return type.

## Creating instances

`new` creates an instance. Parentheses are optional when there are no arguments:

```tlua
local class Player = new player;
local class Rival = new player("Grace");
```

`class` is a special declaration type: the compiler distinguishes a class definition (`class player { ... }`) from an instance declaration (`class Player = new player;`) by the `{` or `=` after the name. The variable takes the static class type of the `new` expression. `local` and `const` may be used as for other declarations. Calling a method uses the same dot syntax as other members; TypeLua emits the Lua method call with `:`:

```tlua
print(Player.name);
print(Player.isAlive());
Player.damage(30);
print(Player.isAlive());
```

Class names can also be written directly as parameter, return, or variable types:

```tlua
fn alive(player p) -> bool {
    return p.isAlive();
}
```

Classes are not created from table literals. `player { name = "Ada", health = 100 }` is rejected; use `new player("Ada")` instead.

## Inheritance

A class can extend one parent class with `extends`:

```tlua
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

The child inherits the parent's fields and methods. Fields declared in the child must not reuse an inherited name with a different type, and overriding a method requires the same parameter and return types. A field and a method cannot share a name anywhere in the inheritance chain.

If the child declares a constructor, its first statement must call `super(...)` with arguments for the nearest parent constructor. If the child does not declare one, its generated constructor accepts the parent's constructor arguments, calls the parent, and then initializes the child's own fields. Field defaults run from the base class to the derived class.

Use `super.method(...)` to call the parent implementation of an overridden method. `super` cannot be used in a class without a parent, and it is only available inside methods and constructors.

A child instance can be used wherever the parent class type is expected:

```tlua
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
player = {}
player.__index = player

function player.new(name)
    local self = setmetatable({}, player)
    self:__init(name)
    return self
end

function player:__init(name)
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

A derived class points its metatable at the parent class, so inherited methods are found through the chain:

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

Fields are normal table entries on the instance. Methods are table entries on the class, found through `__index`. TypeLua emits colon calls for class methods so Lua passes `self` automatically. No runtime library or TypeLua-specific value representation is required.

## Current restrictions

Classes are currently module-private and support single inheritance only. They do not support interfaces, static members, getters, private fields, or generics. See [Limitations](13-limitations.md) and the [project TODO list](../todo.md).
