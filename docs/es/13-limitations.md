# 13. Limitaciones

Tandem 0.2.0 es una pequeña prueba de concepto. El lenguaje actualmente no implementa:

- getters y setters, `instanceof`, expresiones de clase, nombres de miembro calculados, acceso abstract/final/protected ni exportación de clases desde módulos compilados;
- tipado completamente estático para valores de `dict` y para valores `table` genéricos que no se inicializaron con un literal;
- closures o funciones de primera clase;
- tipos opcionales;
- operadores definidos por el usuario;
- tipos de miembros estáticos para toda función de la biblioteca estándar de Lua: los miembros comunes están tipados, pero el resto sigue siendo dinámico.

Las formas de literales de `table` genéricas se rastrean por variable. No se propagan a través de alias, y una función que muta una tabla global puede invalidar la forma que el comprobador conoce en el sitio de llamada.

Para la herencia múltiple, la primera base de clase concreta proporciona los argumentos del constructor y el objetivo de `super`. Las bases concretas posteriores no deben tener argumentos de constructor requeridos. La herencia de clases genéricas aún no admite aplicar o transformar explícitamente los argumentos de tipo de una base.

El parser se recupera en los límites de sentencia, lo que puede descartar parte de una construcción malformada antes de reportar errores posteriores. Los mapas de fuente son comentarios a nivel de sentencia en lugar de un mapeo byte a byte, y los tracebacks de Lua siguen usando el archivo y los números de línea generados.

El runtime embebe Lua 5.4 estándar y no añade facilidades de motor de juego: un motor de juego anfitrión debe proporcionar su propio bucle de actualización y API de entrada.

El trabajo planificado se sigue en la [lista de tareas del proyecto](../todo.md).
