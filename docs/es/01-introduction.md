# 1. Introducción

Tandem es un lenguaje pequeño, estáticamente tipado y de estilo C que transpila a Lua 5.4. Los programas se escriben en archivos `.tdm`, se compilan a Lua ordinario y se ejecutan con el runtime embebido. No se necesita un ejecutable de Lua separado.

La compilación es de dos fases: el frontend comprueba la sintaxis y los tipos, y el backend genera Lua legible. Los tipos se verifican en tiempo de compilación y se borran en el Lua generado, por lo que la salida es Lua normal.

El intérprete es Lua 5.4 embebido a través de [`mlua`](https://docs.rs/mlua/) con la feature de Lua vendido, por lo que no se necesita un ejecutable `lua` separado. Tandem no incluye un motor de juego; la aplicación anfitriona proporciona cualquier bucle de frames y API de entrada.

## Un primer vistazo

```tdm
int playerX = 100;

fn movePlayer(int speed = 10) -> void {
    playerX += speed;
}

movePlayer(20);

if (playerX > 100) {
    print("Player moved!");
}
```

Los tipos como `int` se comprueban en tiempo de compilación y se borran en el Lua generado, así que la salida es Lua ordinario y legible.

## Cómo leer este libro

Si Tandem es nuevo para ti, lee [Primeros pasos](02-getting-started.md) a continuación y después recorre la Parte II en orden. Los capítulos de herramientas pueden leerse de forma independiente.
