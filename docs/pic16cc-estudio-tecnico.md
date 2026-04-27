# `pic16cc` / `picc`

## Estudio Técnico Integral y Didáctico

### Cómo entender, explicar y usar este compilador como base para construir el tuyo

---

# Prólogo

Este documento está escrito como si fuera el comienzo de un libro técnico serio sobre construcción de compiladores para microcontroladores PIC16 clásicos, usando como caso de estudio concreto el compilador de este repositorio: `pic16cc`, cuyo binario de usuario es `picc`.

La intención no es resumir el proyecto. La intención es enseñarlo.

Más aún: la intención es que una persona que ya sabe programar, que sabe leer código, que sabe lo que es un proyecto software serio, pero que todavía no sabe “pensar como constructor de compiladores”, pueda terminar este texto con tres capacidades reales:

- entender cómo funciona `pic16cc` de extremo a extremo
- explicar el proyecto con criterio técnico, no sólo con palabras memorizadas
- usar el proyecto como base conceptual para comenzar a diseñar su propio compilador

Este texto está escrito con una regla pedagógica estricta:

Cada concepto importante se presenta respondiendo siempre cuatro preguntas:

1. qué es
2. por qué existe
3. cómo se implementa aquí
4. ejemplo pequeño

No se asume conocimiento previo de compiladores. Sí se asume que el lector sabe programar y puede seguir razonamientos técnicos.

Esta edición ampliada añade una segunda capa pedagógica sobre la primera:

- más lectura guiada de código real del repositorio
- más casos prácticos completos, siguiendo ejemplos existentes en `examples/`
- más conexión explícita entre fuente C, semántica, IR, backend, artefactos y tests

La idea es que el lector no sólo entienda conceptos. La idea es que pueda abrir archivos reales del proyecto y reconocer por qué cada bloque de código está ahí, qué problema resuelve y cómo encaja con el resto.

Estado del proyecto estudiado:

- crate principal: `pic16cc`
- binario CLI de usuario final: `picc`
- targets soportados: `PIC16F628A` y `PIC16F877A`
- backend compartido: `src/backend/pic16/midrange14`
- fase funcional actual: Phase 7

---

# 1. Cómo leer este documento

## Qué es este documento

Es un texto de estudio, no una referencia mínima ni una lista de archivos.

Por qué existe:

- porque un compilador no se entiende bien leyendo código suelto
- primero hace falta un modelo mental
- después ese modelo se conecta con el repositorio

Cómo está construido:

- primero se explica qué hace un compilador, desde cero
- luego se explica el hardware PIC16, porque sin entender el hardware no se entiende el backend
- después se recorre el pipeline completo
- luego se estudia la evolución fase por fase
- finalmente se profundiza en memoria, ABI, helpers, ISR, optimización, pruebas y trade-offs

Ejemplo:

Si una persona ve una expresión como:

```c
PORTB = add1(3);
```

al principio sólo ve “una línea C”.

Tras leer este documento debería poder verla también como:

```text
1. tokens
2. árbol sintáctico
3. expresión tipada
4. llamada ABI
5. retorno en W
6. store a SFR bancarizado
7. instrucciones PIC16
8. palabras de 14 bits
9. registro Intel HEX
```

## Qué no es este documento

No es:

- un tutorial de C
- un manual completo de todos los PIC16 existentes
- un tratado teórico general de compiladores desligado del código real
- una promesa de soporte de C completo

Su foco es éste:

```text
explicar un compilador real, con límites reales, sobre una arquitectura difícil
```

## Qué gana el lector si lo estudia bien

Si se estudia bien este texto, el lector gana algo muy valioso: vocabulario técnico con significado real.

No sólo sabrá decir palabras como:

- AST
- IR
- lowering
- ABI
- frame
- stack

Sino que podrá responder:

- qué problema resuelve cada una
- por qué aparece en este proyecto
- y qué consecuencias tiene si se diseña mal

---

# 2. Si nunca has construido un compilador antes

## Qué es un compilador

Definición:

Un compilador es un programa que toma código fuente escrito por humanos y lo transforma en instrucciones ejecutables para una máquina concreta.

Por qué existe:

- porque las CPUs no entienden lenguajes de alto nivel como C
- entienden instrucciones muy concretas, con formatos muy concretos

Cómo se ve eso en `pic16cc`:

- entrada: un subconjunto de C
- salida: Intel HEX programable para PIC16 clásicos

Ejemplo:

Código fuente:

```c
PORTB = 1;
```

Resultado conceptual:

```text
1. cargar el literal 1 en el acumulador W
2. escribir W en el registro PORTB
```

Resultado asm aproximado:

```text
movlw 0x01
movwf PORTB
```

## Por qué un compilador no traduce “de golpe”

Definición:

Traducir “de golpe” sería intentar pasar directamente de texto fuente a código máquina sin etapas intermedias claras.

Por qué no suele funcionar bien:

- el texto tiene demasiada ambigüedad sintáctica
- el lenguaje fuente tiene reglas de tipos, ámbitos y conversiones
- el hardware tiene limitaciones muy distintas a las del lenguaje
- conviene separar problemas

Cómo se refleja en `pic16cc`:

- hay frontend
- hay análisis semántico
- hay IR
- hay backend
- hay encoder
- hay writer de Intel HEX

Ejemplo:

La línea:

```c
x = a + b;
```

puede verse de muchos modos distintos según la etapa:

```text
texto:     "x = a + b;"
tokens:    IDENT(x), '=', IDENT(a), '+', IDENT(b), ';'
AST:       asignación(x, suma(a, b))
semántica: x es lvalue, a y b son enteros, la suma devuelve un entero
IR:        t0 = a + b; x = t0
backend:   cargar a, sumar b, guardar en x
```

## Qué es un lenguaje fuente y qué es un lenguaje destino

Definición:

- lenguaje fuente: el lenguaje en que escribe el programador
- lenguaje destino: el que entiende el hardware o una herramienta posterior

Por qué existe esta distinción:

- compilar siempre es transformar entre dos niveles de representación

Cómo se ve aquí:

- fuente: C acotado
- destino final: Intel HEX para PIC16

Ejemplo:

```text
fuente:    unsigned int x = 3;
destino:   secuencia de instrucciones PIC16 codificadas y empaquetadas en HEX
```

## Qué es una representación intermedia mental

Antes de hablar de la IR del repositorio, conviene entender la idea general.

Definición:

Una representación intermedia es una forma de representar un programa que es:

- más estructurada que el texto
- más simple que el lenguaje original
- todavía no tan concreta como el código máquina final

Por qué existe:

- porque ayuda a separar el problema de entender el programa del problema de generar instrucciones concretas

Ejemplo:

No es cómodo generar directamente código PIC16 desde algo tan abstracto como:

```c
arr[i] = arr[i] + 1;
```

Es mucho más cómodo pasarlo antes a una forma que diga explícitamente:

```text
1. calcular dirección de arr[i]
2. leer indirectamente
3. sumar 1
4. escribir indirectamente
```

Eso, precisamente, es el tipo de trabajo que hace una IR y su lowering.

## Analogía útil: un compilador como una cadena de traducción profesional

Imagina que tienes una novela en español y quieres imprimirla en japonés.

No haces:

```text
novela en español -> imprenta japonesa
```

Haces algo como:

```text
texto
-> dividir en palabras
-> entender la gramática
-> entender el significado
-> reexpresar la idea
-> producir el formato final de impresión
```

Un compilador hace algo muy parecido:

```text
C
-> tokens
-> árbol
-> programa tipado
-> representación intermedia
-> instrucciones de máquina
-> archivo grabable
```

La analogía no es perfecta, pero ayuda mucho al principio.

---

# 3. Qué es `pic16cc` y por qué este proyecto es interesante

## Qué es `pic16cc`

Definición:

`pic16cc` es un compilador experimental escrito en Rust para microcontroladores PIC16 clásicos de 14 bits.

Por qué existe:

- porque PIC16 es una arquitectura muy instructiva para estudiar diseño de compiladores
- porque obliga a resolver problemas de ABI, memoria, aritmética e ISR de forma explícita
- porque el proyecto busca una cadena completa y nativa de C a `.hex`

Cómo se implementa aquí:

- crate `pic16cc`
- binario `picc`
- backend compartido `midrange14`
- targets `PIC16F628A` y `PIC16F877A`

Ejemplo:

Uso real del compilador:

```bash
picc --target pic16f628a -Wall -Wextra -Werror -O2 -I include -o build/blink.hex examples/pic16f628a/blink.c
```

## Por qué PIC16 es una diana tan interesante para un compilador

Porque es una arquitectura incómoda.

Eso, lejos de ser un defecto para el estudio, es una ventaja enorme.

Problemas interesantes que fuerza:

- muy pocos registros
- acumulador principal `W`
- memoria de datos bancarizada
- llamadas y saltos paginados
- sin pila de datos de propósito general
- sin multiplicación o división hardware
- 16 bits implementados sobre una ALU esencialmente de 8 bits

Por qué eso es bueno para aprender:

- porque obliga a que el compilador sea honesto
- no puede “apoyarse” en un hardware generoso
- cada decisión importante se vuelve visible

Ejemplo:

En una arquitectura moderna, llamar a una función con 4 argumentos es rutinario.

En PIC16 clásico, eso obliga a diseñar una convención de llamada seria, porque el hardware no te da una pila de datos estándar.

## Qué problema resuelve el proyecto

Definición del problema:

```text
compilar un subconjunto útil y coherente de C a PIC16 real,
sin dejar soporte fingido en el frontend que no exista en el backend
```

Por qué esa formulación es importante:

- muchos proyectos parsean cosas que luego no saben bajar
- aquí el objetivo es que el soporte sea extremo a extremo

Cómo se implementa aquí:

- parser reconoce construcciones
- semántica las valida
- IR las representa
- backend las baja a PIC16
- encoder las convierte a palabras reales
- Intel HEX writer produce salida programable

Ejemplo:

La multiplicación `a * b` no se considera “soportada” sólo porque el parser entienda el `*`.

Se considera soportada porque:

- la semántica la clasifica
- la IR la transporta
- el backend la baja a inline o helper
- el `.asm` la muestra
- el `.hex` final sigue siendo válido

## Qué produce el compilador

La salida principal es un `.hex`.

Además puede producir:

- `.ast`
- `.ir`
- `.asm`
- `.map`
- `.lst`

Por qué existen esos artefactos:

- el `.hex` sirve para programar
- el `.asm` sirve para estudiar qué emitió el backend
- el `.map` sirve para ver símbolos y layout
- el `.lst` sirve para inspección humana
- el `.ast` y `.ir` sirven para estudiar el frontend y el lowering

Ejemplo:

Si estás depurando un bug de stack ABI, el `.hex` no te ayuda mucho visualmente.

El `.asm`, `.map` y `.lst` sí.

---

# 4. Entender el hardware PIC16 antes de entender el compilador

Este capítulo es decisivo. Mucha gente intenta entender un backend sin entender el hardware destino. Eso suele llevar a una comprensión superficial.

Aquí haremos lo contrario: primero el hardware, luego el compilador.

## Qué es una arquitectura Harvard

Definición:

Una arquitectura Harvard separa la memoria de programa y la memoria de datos.

Por qué existe:

- es una forma clásica de diseñar microcontroladores pequeños
- simplifica ciertas rutas internas
- históricamente ha sido común en familias como PIC

Cómo se implementa aquí:

- el compilador trata los punteros soportados como punteros a memoria de datos
- no soporta punteros a código
- los vectores de reset e interrupción viven en memoria de programa
- variables y SFR viven en memoria de datos

Ejemplo:

```c
unsigned char *p;
```

En `pic16cc`, `p` es un puntero a datos, no un puntero a instrucciones de programa.

### Intuición práctica

Piensa en dos edificios distintos:

- edificio A: instrucciones
- edificio B: datos

No puedes tratar una habitación del edificio A como si fuera una habitación del edificio B.

## Qué es la RAM bancarizada

Definición:

La RAM bancarizada es una memoria de datos dividida en bancos. La instrucción no codifica toda la dirección final de forma directa; parte de la dirección depende de bits de selección de banco.

Por qué existe:

- las instrucciones PIC16 son compactas
- no hay espacio suficiente para meter direcciones grandes directas en todos los formatos

Cómo se implementa aquí:

- el backend usa `STATUS.RP0` y `STATUS.RP1`
- mantiene un “banco actual”
- antes de acceder a una dirección directa, decide si necesita cambiar de banco

Ejemplo:

Supón dos variables:

```text
var_a en banco 0
var_b en banco 1
```

Acceder a ambas seguidas puede requerir:

```text
seleccionar banco 0
leer/escribir var_a
seleccionar banco 1
leer/escribir var_b
```

### Analogía útil

Imagina una cómoda con varios cajones.

- la dirección codificada por la instrucción te dice “qué posición mirar dentro del cajón”
- `RP0` y `RP1` te dicen “qué cajón está abierto”

## Qué es el registro `W`

Definición:

`W` es el acumulador principal del PIC16.

Por qué existe:

- muchas instrucciones PIC16 están diseñadas alrededor de un único registro de trabajo
- el hardware es muy pequeño y no ofrece una colección rica de registros generales

Cómo se implementa aquí:

- gran parte del backend piensa en términos de “cargar algo en `W`”, operar y guardar desde `W`
- muchas rutas de codegen se organizan alrededor de ese patrón

Ejemplo:

```c
x = 5;
```

Puede bajar a:

```text
movlw 0x05
movwf x
```

### Consecuencia importante

Cuando una CPU tiene muy pocos registros, la calidad del compilador depende mucho de:

- cuándo recarga a `W`
- cuándo puede reutilizar `W`
- cuándo tiene que derramar a memoria

Eso es justo uno de los focos de Phase 7.

## Qué es la pila hardware del PIC16 y por qué no basta

Definición:

PIC16 sí tiene pila hardware, pero esa pila sirve para guardar direcciones de retorno de llamadas, no datos arbitrarios del programa.

Por qué existe:

- la CPU necesita volver al sitio correcto después de un `CALL`

Por qué no basta para C:

El lenguaje C necesita espacio para:

- argumentos
- variables locales automáticas
- temporales
- frames de llamadas anidadas

La pila hardware del PIC16 no resuelve eso.

Cómo se implementa aquí:

- `pic16cc` construye un stack de software en RAM a partir de Phase 4

Ejemplo:

```c
return a(b(c(1)));
```

Sin un stack de datos real, los intermedios de `c`, `b` y `a` se pisan con facilidad.

## Qué es el direccionamiento indirecto

Definición:

Direccionamiento indirecto significa acceder a una posición de memoria mediante una dirección almacenada en otra ubicación, en vez de codificar directamente la dirección fija en la instrucción.

Por qué existe:

- porque los punteros requieren calcular una dirección en tiempo de ejecución
- porque el stack y los frames se navegan por dirección relativa

Cómo se implementa en PIC16:

- `FSR` contiene la dirección
- `INDF` es la ventana de acceso al contenido apuntado por `FSR`
- `STATUS.IRP` ayuda con el banco indirecto

Cómo lo usa `pic16cc`:

- Phase 3: punteros y arrays
- Phase 4: frame y stack
- Phase 6: ISR con locales y temporales

Ejemplo:

```c
*ptr = value;
```

Pasos conceptuales:

1. leer el valor de `ptr`
2. poner parte baja en `FSR`
3. ajustar `IRP`
4. escribir `value` a través de `INDF`

### Analogía muy útil

Piensa así:

```text
FSR  = “el dedo señala una caja”
INDF = “abrir la caja señalada”
```

## Qué es `PCLATH`

Definición:

`PCLATH` es un registro que ayuda a completar la dirección de ciertas transferencias de control, como `CALL` y `GOTO`.

Por qué existe:

- porque las instrucciones no llevan toda la dirección completa “larga”
- la CPU toma parte de la dirección de un registro auxiliar

Cómo se implementa aquí:

- el backend emite la preparación de página adecuada antes de llamadas y saltos
- Phase 7 evita `setpage` redundantes

Ejemplo:

```text
llamar a una función en otra página no es sólo “call fn”
también exige preparar `PCLATH`
```

## Qué es un SFR

Definición:

SFR significa Special Function Register: registro especial del hardware.

Por qué existe:

- porque el hardware del microcontrolador se controla mediante registros mapeados en memoria

Cómo lo implementa aquí `pic16cc`:

- los headers de dispositivo los exponen por nombre
- la semántica los registra como símbolos válidos
- el backend usa sus direcciones reales desde el descriptor del dispositivo

Ejemplo:

```c
TRISB = 0x00;
PORTB = 0x01;
```

No estás escribiendo variables normales. Estás configurando y usando hardware real.

## Resumen: por qué este hardware hace difícil escribir un compilador

Ahora ya se puede formular el problema completo:

- poca capacidad de direccionamiento directo
- bancos de RAM
- páginas de programa
- muy pocos registros
- acumulador `W`
- sin stack de datos de propósito general
- sin multiplicación/división hardware
- 16 bits sobre una máquina muy limitada

Eso convierte a `pic16cc` en un caso de estudio excelente. Obliga a que decisiones como:

- IR
- ABI
- frames
- helpers
- ISR

sean visibles y necesarias, no simples “ornamentos teóricos”.

---

# 5. Un programa guía que seguiremos durante el libro

Tener un ejemplo conductor ayuda mucho. Vamos a usar varios ejemplos, pero uno de los más sencillos será éste:

```c
#include <pic16/pic16f628a.h>

unsigned char add1(unsigned char x) {
    return x + 1;
}

void main(void) {
    TRISB = 0x00;
    PORTB = add1(3);
}
```

Por qué este programa es útil:

- tiene una función con parámetro
- tiene una llamada
- tiene una suma
- toca SFR reales
- acaba siendo fácil de seguir en cada etapa

Más adelante añadiremos ejemplos más ricos:

- arrays y punteros
- llamadas anidadas
- multiplicación y división
- ISR

Pero este programa base es ideal para comprender el pipeline.

---

# 6. Vista global del pipeline completo

## El pipeline en una sola imagen

```text
archivo .c
   |
   v
preprocesador
   |
   v
lexer
   |
   v
tokens
   |
   v
parser
   |
   v
AST
   |
   v
análisis semántico
   |
   v
programa tipado
   |
   v
IR
   |
   v
optimizaciones IR
   |
   v
backend PIC16
   |
   v
ensamblador interno
   |
   v
codificación a palabras de 14 bits
   |
   v
Intel HEX
```

## Qué problema resuelve cada etapa

### Preprocesador

Resuelve:

- includes
- macros simples
- condicionales del preprocesador

### Lexer

Resuelve:

- pasar de caracteres sueltos a tokens

### Parser

Resuelve:

- reconstruir la estructura gramatical del programa

### AST

Sirve para:

- representar la forma del programa sin ruido textual

### Semántica

Resuelve:

- tipos
- nombres
- lvalue/rvalue
- restricciones del lenguaje soportado

### IR

Sirve para:

- separar frontend y backend
- hacer explícitas operaciones que el backend necesita

### Optimizaciones IR

Sirven para:

- simplificar el programa antes de generar asm

### Backend

Resuelve:

- cómo bajar todo eso a PIC16 real

### Encoder

Resuelve:

- convertir instrucciones simbólicas en palabras binarias válidas

### HEX

Resuelve:

- producir un artefacto programable por herramientas externas

## Cómo se ve esto en el repositorio

Capas principales:

- frontend: `src/frontend`
- IR: `src/ir`
- backend PIC16: `src/backend/pic16/midrange14`
- artefactos de salida: `src/assembler`, `src/linker`, `src/hex`
- CLI y orquestación: `src/cli`, `src/lib.rs`, `src/main.rs`

Ejemplo:

La función `execute` en `src/lib.rs` es el hilo que encadena todo el pipeline.

---

# 7. Preprocesado: la etapa anterior al lenguaje “real”

## Qué es el preprocesado

Definición:

El preprocesado es una transformación textual previa al análisis gramatical del lenguaje.

Por qué existe:

- C históricamente lo incorpora
- los headers de microcontroladores dependen muchísimo de `#include` y `#define`

Cómo se implementa aquí:

- en `src/frontend/preprocessor.rs`
- soporta `#include`
- soporta macros objeto
- soporta condicionales del preprocesador básicos

Ejemplo:

```c
#define LED 0x01
PORTB = LED;
```

Tras el preprocesado, el compilador ya trabaja con el valor expandido.

## Qué significa que el preprocesador sea “textual”

Definición:

Significa que opera sobre texto, no sobre AST ni sobre tipos.

Por qué importa:

- porque `#define` no “entiende C” como lo entiende la semántica
- hace sustituciones previas

Cómo se ve aquí:

La expansión ocurre antes de tokenizar completamente el programa final.

Ejemplo:

```c
#define X 3
int a = X + 1;
```

No es el parser quien decide primero qué es `X`. Antes de eso, el preprocesador ya ha expandido el texto.

## Qué resuelve `#include`

Definición:

`#include` inserta el contenido de un archivo en otro durante el preprocesado.

Por qué existe:

- para reutilizar declaraciones, macros y definiciones de dispositivo

Cómo se implementa aquí:

- el compilador localiza headers según `-I`
- además conoce include dirs del usuario y del proyecto

Ejemplo:

```c
#include <pic16/pic16f877a.h>
```

Ese header es fundamental porque expone nombres como:

- `PORTB`
- `TRISB`
- `ADCON1`

## Qué problema extra resuelve `pic16cc`: el origen de los diagnósticos

Cuando expandes includes y macros, el texto final ya no coincide línea por línea con el fichero que escribió el usuario.

Por qué importa:

- si aparece un error, el usuario quiere saber en qué archivo y línea original ocurrió

Cómo se implementa aquí:

- el preprocesador conserva información de origen
- el emisor de diagnósticos puede mapear errores de vuelta al fichero correcto

Ejemplo:

Si un error viene de un header incluido, el mensaje debe apuntar al header; si viene del archivo principal, debe apuntar allí.

---

# 8. Lexing: de caracteres a tokens

## Qué es el lexing

Definición:

Lexing es el proceso de agrupar caracteres en unidades léxicas significativas llamadas tokens.

Por qué existe:

- porque el parser no quiere trabajar con letras una a una
- quiere trabajar con piezas ya clasificadas

Cómo se implementa aquí:

- en `src/frontend/lexer.rs`

Ejemplo:

Código:

```c
int x = 5;
```

Tokens aproximados:

```text
KW_INT
IDENT(x)
ASSIGN
INT_LITERAL(5)
SEMICOLON
```

## Qué es un token

Definición:

Un token es una unidad mínima con significado sintáctico para el parser.

Por qué existe:

- simplifica mucho la siguiente etapa

Cómo se implementa aquí:

- hay tokens para keywords, identificadores, números, signos y operadores dobles

Ejemplo:

En:

```c
a << 1
```

el lexer reconoce `<<` como un único token, no como dos caracteres `<` independientes.

## Qué diferencia hay entre keyword e identificador

### Keyword

Definición:

Palabra reservada del lenguaje.

Ejemplo:

- `int`
- `void`
- `return`

### Identificador

Definición:

Nombre definido por el programador o por headers/macros válidos.

Ejemplo:

- `counter`
- `add1`
- `PORTB`

Por qué importa la distinción:

- el parser no puede tratar igual `int` y `counter`

## Qué hace el lexer con números

Definición:

Convierte secuencias de dígitos en literales numéricos.

Cómo se implementa aquí:

- reconoce decimales y hexadecimales

Ejemplo:

```c
0x20
42
```

## Qué aporta el lexer al diseñador de compiladores

Le aporta la primera lección importante:

```text
antes de entender “significado” hay que estabilizar la forma de las piezas
```

Si estás diseñando tu propio compilador, esta etapa debe ser:

- simple
- determinista
- muy fácil de depurar

Porque todo lo demás se apoya en ella.

---

# 9. Parsing: de tokens a estructura

## Qué es el parsing

Definición:

Parsing es el proceso de reconstruir la estructura gramatical del programa a partir de tokens.

Por qué existe:

- los tokens sólo dicen qué piezas hay
- no dicen cómo se agrupan

Cómo se implementa aquí:

- en `src/frontend/parser.rs`
- con niveles de precedencia y funciones por categoría sintáctica

Ejemplo:

Tokens:

```text
IDENT(a) PLUS IDENT(b) STAR IDENT(c)
```

El parser decide que eso es:

```text
a + (b * c)
```

y no:

```text
(a + b) * c
```

## Qué es un AST

Definición:

AST significa Abstract Syntax Tree, árbol de sintaxis abstracta.

Por qué existe:

- porque el parser necesita producir una representación estructurada del programa
- el AST elimina detalles textuales secundarios y conserva relaciones importantes

Cómo se implementa aquí:

- tipos en `src/frontend/ast.rs`
- nodos para unidad de traducción, declaraciones, sentencias y expresiones

Ejemplo:

Código:

```c
x = a + b;
```

AST simplificado:

```text
   =
  / \
 x   +
    / \
   a   b
```

## Qué significa “abstracto” en AST

Definición:

“Abstracto” significa que no conserva todos los detalles superficiales del texto original.

Por qué existe:

- los espacios, saltos, ciertos paréntesis redundantes y formatos visuales no importan para el significado

Cómo se ve aquí:

Estas dos expresiones acaban con la misma estructura:

```c
x=a+b;
```

```c
x = a + b;
```

## Qué hace el parser con una llamada a función

Ejemplo:

```c
add1(3)
```

El parser produce una estructura que ya distingue:

- nombre de función
- lista de argumentos

Por qué importa:

- más adelante la semántica resolverá si `add1` existe y qué firma tiene

## Qué aporta esta etapa a quien quiera construir su propio compilador

Una lección muy importante:

```text
el parser no debe intentar resolver todos los problemas del compilador
```

Su trabajo es reconstruir estructura.

No debería decidir:

- tipos finales
- ABI
- cómo se genera el código

Esa separación de responsabilidades es una de las claves del diseño limpio.

---

# 10. Análisis semántico: pasar de “estructura válida” a “programa válido”

## Qué es el análisis semántico

Definición:

Es la etapa que comprueba si el programa tiene sentido desde el punto de vista del lenguaje.

Por qué existe:

- porque un programa puede estar bien parseado y seguir siendo incorrecto

Cómo se implementa aquí:

- en `src/frontend/semantic.rs`

Ejemplo:

```c
5 = x;
```

El parser puede reconocer algo con forma de asignación.

La semántica lo rechaza porque `5` no es una ubicación de memoria escribible.

## Qué es un símbolo

Definición:

Un símbolo es la representación interna de una entidad nombrada del programa.

Ejemplos:

- variables
- funciones
- SFR

Por qué existe:

- el compilador necesita asociar un nombre con información concreta

Cómo se implementa aquí:

- la semántica crea símbolos y les asigna ids internos

Ejemplo:

Cuando el usuario escribe:

```c
unsigned char counter;
```

el compilador crea un símbolo para `counter`, con tipo, ámbito y almacenamiento.

## Qué es un tipo

Definición:

Un tipo describe qué clase de valor es algo y qué operaciones tienen sentido sobre él.

Por qué existe:

- para evitar operaciones absurdas
- para decidir conversiones
- para saber cuántos bytes ocupa un valor

Cómo se implementa aquí:

Tipos soportados:

- `void`
- `char`
- `unsigned char`
- `int`
- `unsigned int`
- punteros simples a tipos soportados
- arrays unidimensionales de tipos soportados

Ejemplo:

```c
unsigned int x;
```

Semánticamente, el compilador ya sabe:

- que `x` es entero sin signo
- que ocupa 2 bytes
- que ciertas operaciones serán válidas y otras no

## Qué son lvalue y rvalue

### Lvalue

Definición:

Expresión que representa una ubicación de memoria.

Ejemplos:

- `x`
- `*ptr`
- `arr[i]`

### Rvalue

Definición:

Valor calculado, no directamente escribible.

Ejemplos:

- `x + 1`
- `a < b`

Por qué existe esta distinción:

- porque en C no todo lo que puedes leer lo puedes escribir

Cómo lo implementa aquí:

- la semántica conserva esa información
- es esencial para Phase 3, donde aparecen punteros y arrays

## Qué significa “programa tipado”

Definición:

Es una representación del programa en la que cada expresión ya tiene tipo y categoría semántica conocidas.

Por qué existe:

- el backend no debería tener que “re-descubrir” tipos mirando AST bruto

Cómo se implementa aquí:

- resultado de `SemanticAnalyzer`
- entrada para el `IrLowerer`

Ejemplo:

La expresión:

```c
x + 1
```

ya no es sólo “una suma”.

Pasa a ser algo como:

```text
suma de tipo unsigned char
o
suma de tipo unsigned int
```

y eso cambia el lowering.

## Qué papel juegan los casts

Definición:

Un cast es una conversión explícita o implícita entre tipos.

Por qué existe:

- porque a menudo hay que extender, truncar o reinterpretar valores

Cómo se implementa aquí:

- la semántica inserta casts explícitos en la representación tipada
- la IR los convierte en instrucciones `Cast`

Ejemplo:

Si un byte se convierte en entero de 16 bits, la semántica deja clara la intención:

- zero extend
- sign extend

Eso evita ambigüedad más adelante.

## Qué diagnósticos importantes aparecen aquí

Ejemplos reales del proyecto:

- firma ISR inválida
- recursión no soportada
- puntero a local devuelto
- división por cero constante
- pointer-to-pointer no soportado
- conversiones estrechas problemáticas

Por qué esta etapa es ideal para esos diagnósticos:

- porque aquí ya conoces tipos, símbolos y estructura
- pero aún no has generado código

## Lección para quien quiera construir su propio compilador

No subestimes la semántica.

Mucha gente piensa en el compilador como:

```text
parser + generador de código
```

En realidad, sin una semántica buena:

- el backend se vuelve frágil
- los errores se detectan demasiado tarde
- la arquitectura se ensucia

---

# 11. La representación intermedia: IR

## Qué es una IR

Definición:

IR significa Intermediate Representation: representación intermedia.

Por qué existe:

- porque el AST está demasiado cerca del lenguaje fuente
- el backend PIC16 necesita operaciones más explícitas
- además permite introducir optimizaciones antes del código final

Cómo se implementa aquí:

- modelo en `src/ir/model.rs`
- lowering en `src/ir/lowering.rs`
- passes en `src/ir/passes.rs`

Ejemplo:

Una llamada a función en AST es todavía una expresión del lenguaje.

En IR se convierte en una operación explícita:

```text
Call { dst, function, args }
```

## Qué significa que la IR esté basada en CFG

Definición:

CFG significa Control Flow Graph, grafo de flujo de control.

Qué significa en lenguaje simple:

- una función se representa como bloques
- cada bloque tiene posibles transiciones a otros

Por qué existe:

- porque `if`, `while`, `break`, `continue`, `return` y comparaciones encajan muy bien en ese modelo

Cómo se implementa aquí:

- `IrFunction`
- `IrBlock`
- terminadores como `Jump`, `Branch`, `Return`

Ejemplo:

Código:

```c
if (x) {
    y = 1;
} else {
    y = 0;
}
```

CFG simplificado:

```text
entry -> branch on x
  -> then
  -> else
then -> exit
else -> exit
```

## Qué es un temporal IR

Definición:

Un temporal IR es un valor intermedio nombrado por el compilador, no por el usuario.

Por qué existe:

- porque descomponer expresiones complejas en pasos simples facilita el backend

Cómo se implementa aquí:

- como `TempId`
- cada temporal tiene tipo asociado

Ejemplo:

```c
return (a + b) + 1;
```

IR conceptual:

```text
t0 = a + b
t1 = t0 + 1
return t1
```

## Qué operaciones hace explícitas la IR de este proyecto

Ejemplos importantes:

- `Copy`
- `Cast`
- `AddrOf`
- `LoadIndirect`
- `StoreIndirect`
- `Call`
- comparaciones tipadas

Por qué existe esta explicitud:

- porque el backend no debería deducir punteros, decay de arrays o casts mirando AST ambiguo

Ejemplo:

```c
*ptr = value;
```

IR conceptual:

```text
StoreIndirect(ptr, value)
```

Eso hace visible el problema real al backend: acceso indirecto.

## Qué significa “lowering a IR”

Definición:

Es traducir el programa tipado a esta representación intermedia.

Por qué existe:

- porque la semántica ya ha resuelto el significado
- ahora toca expresarlo de forma operable para el backend

Cómo se implementa aquí:

- `IrLowerer::lower`

Ejemplo:

El `sizeof` no llega vivo al backend como una operación “misteriosa”. La semántica lo resuelve y el lowering ya lo convierte en un literal concreto.

## Una idea crucial: la IR no es “una moda”

A veces quien empieza en compiladores oye hablar de AST e IR y piensa que son capas “académicas”.

En un proyecto como éste no lo son.

Aquí la IR es práctica porque resuelve problemas concretos:

- ayuda a bajar comparaciones de 16 bits
- ayuda a hacer explícitos arrays y punteros
- ayuda a modelar llamadas
- permite optimizar antes del backend

---

# 12. Optimizaciones a nivel IR

## Qué es optimizar en un compilador

Definición:

Optimizar es transformar el programa interno para que mantenga el mismo comportamiento observable pero con mejor coste.

Qué significa “mejor coste” aquí:

- menos instrucciones
- menos temporales
- menos helpers
- menos cambios innecesarios de banco o página

## Qué optimizaciones aplica `pic16cc` en la IR

Phase 7 añade varias optimizaciones antes del backend:

- propagación de constantes
- constant folding
- simplificación de ramas constantes
- eliminación de código muerto
- compactación de temporales

## Propagación de constantes

Definición:

Si el compilador sabe que un temporal vale una constante, intenta sustituir usos posteriores por esa constante.

Por qué existe:

- simplifica expresiones
- abre la puerta a otras optimizaciones

Cómo se implementa aquí:

- pass `constant_fold` en `src/ir/passes.rs`

Ejemplo:

```text
t0 = 3
t1 = t0 + 1
```

puede convertirse en:

```text
t1 = 3 + 1
```

y luego:

```text
t1 = 4
```

## Constant folding

Definición:

Es evaluar en tiempo de compilación una expresión cuyos operandos ya son constantes.

Por qué existe:

- evita generar trabajo inútil en tiempo de ejecución

Cómo se implementa aquí:

- también en `constant_fold`

Ejemplo:

```text
(2 + 3) * 4
```

puede reducirse a:

```text
20
```

## Eliminación de código muerto

Definición:

Es eliminar instrucciones o bloques cuyo resultado no afecta al comportamiento final.

Por qué existe:

- el lowering y otras optimizaciones pueden dejar restos inútiles

Cómo se implementa aquí:

- `dead_code_elimination`

Ejemplo:

```text
t0 = a + b
// nadie usa t0
```

Si `t0` no tiene efectos laterales, puede eliminarse.

## Compactación de temporales

Definición:

Es remapear temporales vivos para usar menos slots efectivos.

Por qué existe:

- en este compilador los temporales viven en el frame
- menos temporales útiles significa menos presión sobre el frame

Cómo se implementa aquí:

- `compact_temps`

Ejemplo:

Si sobreviven sólo `t0` y `t4`, puede renumerarlos como `t0` y `t1`.

## Lección de diseño

En un compilador pequeño y serio, una buena IR con unas cuantas optimizaciones honestas vale mucho más que intentar hacer magia tarde en el backend.

---

# 13. Backend PIC16: donde la semántica se encuentra con la máquina

## Qué es el backend

Definición:

El backend es la parte del compilador que traduce la IR a operaciones concretas de la arquitectura destino.

Por qué existe:

- porque el frontend no debería conocer detalles de ISA
- porque el mismo frontend podría, en teoría, reutilizarse con otros backends

Cómo se implementa aquí:

- backend compartido `src/backend/pic16/midrange14`

Piezas relevantes:

- `codegen.rs`
- `asm.rs`
- `encoder.rs`
- `runtime.rs`

## Qué hace exactamente `codegen.rs`

Definición:

Es el corazón del lowering final a instrucciones simbólicas PIC16.

Por qué existe:

- porque alguien tiene que decidir cómo se implementa cada instrucción IR sobre una máquina concreta

Cómo se implementa aquí:

- emite `AsmInstr`
- decide banking
- decide paging
- decide prologues/epilogues
- integra helpers
- integra ISR

Ejemplo:

Una suma de 16 bits en IR no es una sola instrucción PIC16. `codegen.rs` la descompone en secuencias byte a byte con carry.

## Qué es el ensamblador interno

Definición:

Es una representación estructurada y legible de instrucciones PIC16 antes de codificarlas como palabras.

Por qué existe:

- permite depuración humana
- permite generar `.asm`
- permite optimización peephole

Cómo se implementa aquí:

- `AsmProgram`, `AsmLine`, `AsmInstr` en `asm.rs`

Ejemplo:

```text
movlw 0x01
movwf 0x06
```

Esto ya es “muy cercano” a la máquina, pero todavía no es binario final.

## Qué es el encoder

Definición:

Es la etapa que toma instrucciones simbólicas y las convierte en palabras de 14 bits reales.

Por qué existe:

- porque el microcontrolador ejecuta bits, no strings como `movlw`

Cómo se implementa aquí:

- `encoder.rs`

Ejemplo:

`retfie` tiene una codificación concreta. El encoder debe producir exactamente esa palabra.

## Qué es el Intel HEX writer

Definición:

Es la etapa que empaqueta las palabras de programa en el formato Intel HEX.

Por qué existe:

- porque ése es el artefacto que usan herramientas de grabación y flasheo

Cómo se implementa aquí:

- `src/hex/intel_hex.rs`

Ejemplo:

Además de código, también debe manejar:

- config word
- EOF

## Qué problema adicional resuelve el backend aquí

En un compilador para arquitectura cómoda, el backend ya es complejo.

Aquí además debe resolver:

- stack de software
- acceso indirecto a frames
- runtime helpers
- ISR con guardado de contexto
- bank/page management
- optimizaciones Phase 7

Por eso es el componente más intensamente “arquitectura-dependiente” del proyecto.

---

# 14. Un recorrido pedagógico completo por el pipeline

Vamos a seguir el programa guía desde la fuente hasta la salida final.

Programa:

```c
#include <pic16/pic16f628a.h>

unsigned char add1(unsigned char x) {
    return x + 1;
}

void main(void) {
    TRISB = 0x00;
    PORTB = add1(3);
}
```

## Etapa 1: preprocesado

Lo primero que ocurre es:

- se resuelve el `#include`
- el compilador incorpora el header del dispositivo
- nombres como `TRISB` y `PORTB` ya quedan disponibles

## Etapa 2: lexing

Parte del código se convierte en tokens.

Ejemplo aproximado para:

```c
return x + 1;
```

Tokens:

```text
KW_RETURN
IDENT(x)
PLUS
INT_LITERAL(1)
SEMICOLON
```

## Etapa 3: parsing

El parser reconstruye:

```text
return
  +
 / \
x   1
```

y además entiende:

- que `add1` es una función
- que `main` es otra
- que `PORTB = add1(3)` es una asignación cuyo lado derecho es una llamada

## Etapa 4: análisis semántico

Ahora el compilador decide:

- `x` es `unsigned char`
- `1` debe adaptarse al tipo correcto de la suma
- `add1` devuelve `unsigned char`
- `TRISB` y `PORTB` son SFR válidos del dispositivo

Además clasifica:

- `PORTB` es lvalue
- `add1(3)` es rvalue

## Etapa 5: lowering a IR

Versión conceptual simplificada:

```text
fn add1:
  t0 = x + 1
  return t0

fn main:
  TRISB = 0
  t0 = call add1(3)
  PORTB = t0
  return void
```

Esta IR no es todavía ensamblador, pero ya es muchísimo más explícita que el C original.

## Etapa 6: optimización IR

Si hay optimización activada:

- constantes pueden propagarse
- temporales muertos pueden desaparecer
- ramas constantes pueden simplificarse

En este ejemplo concreto las ganancias pueden ser modestas, pero en programas más ricos son importantes.

## Etapa 7: codegen PIC16

Ahora el backend decide:

- cómo pasar el argumento `3` a `add1`
- cómo leer `x` dentro de `add1`
- cómo devolver el resultado en `W`
- cómo escribir a `PORTB`

## Etapa 8: encoder

Las instrucciones simbólicas resultantes se convierten a palabras de 14 bits.

## Etapa 9: Intel HEX

Esas palabras se escriben en registros Intel HEX listos para programar el micro.

## Por qué este recorrido importa

Porque permite responder una pregunta fundamental:

```text
¿qué parte del compilador es responsable de cada cosa?
```

Si entiendes esto bien, ya no verás el compilador como una caja negra.

---

# 15. Evolución histórica del compilador: de v0.1 a Phase 7

Este capítulo es central porque enseña algo muy valioso: un compilador no suele nacer “completo”. Crece resolviendo problemas concretos en orden.

## v0.1: establecer la cadena mínima completa

### Problema

Antes de hablar de optimización, arrays o ISR, el proyecto necesitaba demostrar una cosa elemental:

```text
puedo compilar de C a un artefacto PIC16 real
```

### Concepto introducido: pipeline completo

Qué es:

la cadena entera desde archivo fuente hasta `.hex`

Por qué existe:

- sin pipeline completo no hay base para validar ninguna fase posterior

Cómo se implementó aquí:

- CLI
- frontend básico
- backend mínimo
- encoding
- HEX

### Qué se ganó

- estructura del proyecto
- delimitación entre frontend, IR y backend
- primeros ejemplos reales

### Ejemplo

Un blink simple ya permite validar:

- lectura de headers
- uso de SFR
- generación de salida programable

### Trade-off

Se sacrifica riqueza de lenguaje para ganar infraestructura.

## Phase 2: 16 bits y comparaciones reales

### Problema

El subconjunto inicial no bastaba para trabajar cómodamente con enteros de 16 bits y comparaciones más interesantes.

### Concepto introducido: representación little-endian de 16 bits

Qué es:

representar un valor de 16 bits como dos bytes, bajo primero y alto después

Por qué existe:

- la CPU es de 8 bits, pero el lenguaje necesita valores más anchos

Cómo se implementa aquí:

- layout little-endian uniforme en globals, locals, params y temporales

Ejemplo:

```text
0x1234 -> low=0x34, high=0x12
```

### Concepto introducido: ABI por slots fijos

Qué es:

un ABI muy limitado basado en unos pocos slots reservados (`arg0`, `arg1`, etc.)

Por qué existía:

- era una forma simple de arrancar antes de tener stack de software

Cómo se implementó:

- dos argumentos máximos
- `W + return_high` para retorno 16-bit

### Qué se implementó

- enteros de 16 bits
- comparaciones signed/unsigned
- casts explícitos en IR
- booleanos normalizados a `0` o `1`

### Trade-off

El ABI por slots sirve para comenzar, pero no escala.

## Phase 3: arrays, punteros y modelo de memoria

### Problema

Sin punteros ni arrays reales, el compilador seguía siendo demasiado estrecho para firmware mínimamente interesante.

### Concepto introducido: address-of y dereference

Qué es:

- `&x`: obtener dirección
- `*ptr`: acceder a través de dirección

Por qué existe:

- es el corazón del modelo de memoria de C

Cómo se implementa aquí:

- `AddrOf`
- `LoadIndirect`
- `StoreIndirect`
- lowering con `FSR/INDF`

### Qué se implementó

- arrays unidimensionales
- punteros a datos
- decay de arrays
- indexación
- loads/stores indirectos

### Trade-off

Se gana mucho poder expresivo, pero aún sin software stack. Los locales y llamadas complejas siguen pendientes.

## Phase 4: Stack-first ABI y frames

### Problema

El ABI anterior y el modelo de memoria por llamada eran insuficientes para:

- 3+ argumentos
- llamadas anidadas profundas
- temporales por invocación
- locales y arrays locales robustos

### Concepto introducido: ABI

Qué es:

el contrato binario entre caller y callee

Por qué existe:

- para que distintas funciones compiladas por separado cooperen sin corrupción de estado

Cómo se implementa aquí:

- caller-pushed
- caller-cleanup
- stack-first
- upward-growing software stack

### Concepto introducido: stack de software

Qué es:

una pila implementada en RAM por el compilador, no por el hardware

Por qué existe:

- el PIC16 no da una pila de datos general

Cómo se implementa aquí:

- `stack_ptr`
- `frame_ptr`
- acceso por `FSR/INDF`

### Qué se implementó

- software stack real
- frames por llamada
- 3+ argumentos
- locals, arrays y temporales por frame
- nested calls correctas

### Ejemplo crítico

```c
return a(b(c(1)));
```

Sin frame model, eso es propenso a corrupción.

Con Phase 4, cada llamada tiene su propio espacio.

### Trade-off

Es una mejora decisiva, pero introduce complejidad backend muy seria y obliga a rechazar recursión.

## Phase 5: helpers aritméticos

### Problema

El lenguaje ya parseaba `*`, `/`, `%` y shifts, pero el backend debía soportarlos de verdad.

### Concepto introducido: helper runtime

Qué es:

una rutina interna del compilador para implementar operaciones complejas

Por qué existe:

- porque la ISA PIC16 no ofrece ciertas operaciones por hardware

Cómo se implementa aquí:

- `__rt_mul_*`
- `__rt_div_*`
- `__rt_mod_*`
- `__rt_sh*`

### Qué se implementó

- multiply/divide/modulo signed y unsigned
- shifts
- fast paths simples inline

### Trade-off

Se añade runtime interno, pero se gana soporte aritmético real.

## Phase 6: ISR

### Problema

Un compilador para firmware real necesita una historia clara para interrupciones.

### Concepto introducido: ISR

Qué es:

ruta de ejecución asíncrona disparada por hardware

Por qué existe:

- para responder a eventos sin polling constante

Cómo se implementa aquí:

- `void __interrupt isr(void)`
- vector `0x0004`
- `retfie`
- guardado/restaurado conservador de contexto

### Qué se implementó

- vector de interrupción
- prologue/epilogue ISR
- restricciones de seguridad

### Trade-off

Se eligió un modelo restringido pero seguro:

- una ISR
- sin llamadas normales
- sin helpers dentro de ISR

## Phase 7: calidad de código

### Problema

El compilador ya era funcional, pero todavía podía generar código innecesariamente torpe.

### Concepto introducido: optimización conservadora

Qué es:

mejorar tamaño/claridad/eficiencia sin cambiar semántica

Cómo se implementa aquí:

- constantes
- DCE
- compactación de temporales
- peephole
- fast paths para potencias de dos
- `--opt-report`

### Trade-off

No hay optimización global agresiva. Se prioriza corrección.

---

# 16. Modelo de memoria en profundidad

Este capítulo amplía Phase 3 y Phase 4 desde una perspectiva más pedagógica.

## Qué es un modelo de memoria

Definición:

Es la explicación de cómo el compilador representa y organiza los datos en la máquina destino.

Por qué existe:

- porque el lenguaje habla de variables y direcciones
- pero el hardware sólo ve bytes en ubicaciones concretas

Cómo se implementa aquí:

- globales en RAM
- SFR por descriptor
- helper slots ABI
- contexto ISR
- stack de software
- acceso directo e indirecto

Ejemplo:

Una variable global, un local, un temporal y un SFR no son “lo mismo”, aunque desde C todos parezcan nombres.

## Globales

Qué son:

variables con vida durante todo el programa

Por qué existen:

- para estado persistente

Cómo se implementan aquí:

- ocupan direcciones RAM globales
- aparecen en `.map`

Ejemplo:

```c
unsigned char counter;
```

## Locales

Qué son:

variables con vida limitada a una invocación concreta

Por qué existen:

- encapsulan trabajo interno de una función

Cómo se implementan aquí:

- desde Phase 4 viven en el frame del stack de software

Ejemplo:

```c
void f(void) {
    unsigned char local;
}
```

## Arrays

Qué son:

zonas contiguas de elementos del mismo tipo

Por qué existen:

- permiten almacenamiento secuencial indexado

Cómo se implementan aquí:

- unidimensionales
- tamaño fijo
- elementos soportados

Ejemplo:

```c
unsigned int words[2];
```

Layout conceptual:

```text
words[0].low
words[0].high
words[1].low
words[1].high
```

## Punteros

Qué son:

valores que representan direcciones

Por qué existen:

- para acceso indirecto
- para pasar referencias
- para decay de arrays

Cómo se implementan aquí:

- 16 bits
- little-endian
- sólo data-space

Ejemplo:

```c
unsigned int *p;
```

## Directo vs indirecto

### Acceso directo

Qué es:

acceder a una dirección fija conocida por el compilador

Cómo se implementa aquí:

- selección de banco
- instrucción directa de fichero

Ejemplo:

```c
PORTB = 1;
```

### Acceso indirecto

Qué es:

acceder mediante una dirección calculada

Cómo se implementa aquí:

- `FSR`
- `INDF`
- `IRP`

Ejemplo:

```c
*p = 1;
```

## Escalado de índices

Qué es:

ajustar un índice por el tamaño del elemento.

Por qué existe:

- `arr[i]` no significa “sumar i bytes” siempre
- depende del tipo del elemento

Cómo se implementa aquí:

- `char` y `unsigned char`: escala 1
- `int` y `unsigned int`: escala 2

Ejemplo:

```c
unsigned int arr[4];
arr[3]
```

La dirección real no es `base + 3`, sino `base + 3 * 2`.

## Lección general

Quien diseña un compilador para una máquina pequeña debe dejar de pensar en “variables abstractas” cuanto antes. Debe empezar a pensar en:

- bytes
- direcciones
- offsets
- accesos directos
- accesos indirectos

Ésa es una de las enseñanzas más valiosas de Phase 3 y Phase 4.

---

# 17. ABI, stack y frames en profundidad

Este capítulo es el corazón conceptual del proyecto.

## Qué es un ABI

Definición:

ABI significa Application Binary Interface.

Por qué existe:

- para que módulos binarios puedan cooperar
- para que caller y callee hablen el mismo protocolo

Cómo se implementa aquí:

- stack-first ABI
- caller pushes args
- caller cleanup
- retorno 8-bit en `W`
- retorno 16-bit en `W + return_high`

Ejemplo:

Si una función espera argumentos en stack pero otra los deja en slots fijos, el programa se rompe. Ésa es precisamente la razón de ser del ABI.

## Qué es un stack

Definición:

Estructura LIFO, último en entrar, primero en salir.

Por qué existe en llamadas:

- las llamadas se anidan
- los retornos se deshacen en orden inverso

Cómo se implementa aquí:

- stack de software en RAM

Ejemplo:

```text
main
  -> f
     -> g
```

Los datos de `g` se retiran antes que los de `f`, y los de `f` antes que los de `main`.

## Qué es un frame

Definición:

Es la porción del stack que pertenece a una invocación concreta.

Por qué existe:

- separa memoria entre llamadas

Cómo se implementa aquí:

- base estable en `frame_ptr`
- offsets relativos para args, saved FP, locals y temps

Diagrama:

```text
FP -> [ args ]
      [ saved FP ]
      [ locals ]
      [ temps ]
```

## Qué es `SP`

Definición:

Stack Pointer, el tope del stack.

Por qué existe:

- para saber dónde queda espacio libre o dónde termina el frame actual

Cómo se implementa aquí:

- helper slot `stack_ptr`

Ejemplo:

Empujar un argumento significa avanzar `SP`.

## Qué es `FP`

Definición:

Frame Pointer, la base del frame actual.

Por qué existe:

- porque `SP` cambia
- y los offsets de parámetros/locales deben medirse respecto a un punto estable

Cómo se implementa aquí:

- helper slot `frame_ptr`

Ejemplo:

Leer el primer parámetro es más fácil como `FP + 0` que como “posición relativa al SP actual después de varias reservas”.

## Layout del frame explicado paso a paso

### Paso 1: caller empuja argumentos

Supongamos:

```c
sum4(1, 2, 3, 4)
```

El caller hace conceptualmente:

```text
push 1
push 2
push 3
push 4
call sum4
```

### Paso 2: callee guarda el `FP` anterior

Por qué:

- al terminar hay que volver al frame anterior

### Paso 3: callee fija su nuevo `FP`

Por qué:

- necesita una referencia estable

### Paso 4: callee reserva locals y temps

Por qué:

- necesita espacio propio de trabajo

### Resultado

```text
FP + 0 ... arg_bytes-1     -> argumentos
FP + arg_bytes             -> saved FP low
FP + arg_bytes + 1         -> saved FP high
FP + arg_bytes + 2 ...     -> locals y temps
```

## Caller vs callee: responsabilidades

### Responsabilidad del caller

- evaluar argumentos
- empujarlos
- invocar
- limpiar bytes de argumentos
- leer retorno

### Responsabilidad del callee

- guardar FP anterior
- fijar nuevo FP
- reservar frame interno
- ejecutar cuerpo
- restaurar SP y FP
- retornar

Por qué esta separación importa:

- porque si ambos limpian la misma zona, el stack se corrompe
- si ninguno la limpia, el stack crece mal

## Ejemplo detallado: llamada simple

Código:

```c
unsigned char add1(unsigned char x) {
    return x + 1;
}
```

Flujo conceptual:

```text
caller:
  push x
  call add1
  cleanup 1 byte

add1:
  save old FP
  set new FP
  reserve temp
  load x
  sum 1
  return in W
```

## Ejemplo detallado: llamadas anidadas

Código:

```c
int a(int x);
int b(int y);
int c(int z);

int main(void) {
    return a(b(c(1)));
}
```

Mentalmente:

```text
frame main
  frame c
  vuelve c
  frame b
  vuelve b
  frame a
  vuelve a
```

Cada invocación tiene espacio lógico propio.

## Por qué se rechaza recursión

Definición:

Recursión es cuando una función se llama a sí misma directa o indirectamente.

Por qué se rechaza:

- el compilador calcula la profundidad máxima de stack estáticamente
- con recursión, ese cálculo deja de ser un DAG limpio
- además no hay detección dinámica de overflow

Ejemplo:

```c
int f(int x) { return f(x); }
```

Se rechaza semánticamente.

## Qué te enseña esto para construir tu propio compilador

Una lección fundamental:

```text
si tu arquitectura no te da una pila cómoda,
tu compilador debe construir un modelo de llamadas extremadamente explícito
```

No puedes posponer esta decisión demasiado. Cuando el lenguaje empieza a tener:

- arrays locales
- llamadas anidadas
- helpers internos

el ABI deja de ser un detalle. Se convierte en la espina dorsal del compilador.

---

# 18. Runtime helpers y aritmética compleja

## Qué es un runtime helper

Definición:

Una rutina auxiliar que el compilador genera para implementar una operación demasiado compleja o demasiado larga para expandirse siempre inline.

Por qué existe:

- porque el hardware PIC16 clásico no ofrece instrucciones cómodas para todo

Cómo se implementa aquí:

- en `src/backend/pic16/midrange14/runtime.rs`
- emitidos sólo si se usan

Ejemplo:

- `__rt_mul_u16`
- `__rt_div_i16`
- `__rt_mod_u8`

## Por qué hacen falta helpers para multiplicar

Definición del problema:

PIC16 no tiene una instrucción nativa de multiplicación clásica del tipo:

```text
mul a, b
```

Por qué importa:

- C sí tiene operador `*`
- por tanto el compilador debe implementarlo de algún modo

Cómo se implementa aquí:

- algoritmo shift-and-add

## Multiplicación shift-and-add, desde cero

### Qué es

Es un algoritmo que combina:

- mirar bits del multiplicador
- desplazar el multiplicando
- sumar sólo cuando el bit correspondiente vale 1

### Por qué existe

- es simple
- funciona bien sin hardware dedicado

### Cómo se implementa aquí

- helpers distintos por signedness y anchura

### Ejemplo paso a paso: `6 * 5`

Valores:

```text
6 = 0110
5 = 0101
```

Proceso:

```text
bit 0 de 5 = 1 -> sumar 6
bit 1 de 5 = 0 -> no sumar
bit 2 de 5 = 1 -> sumar 24
resultado = 30
```

La belleza del método es que transforma multiplicar en combinar:

- inspección de bits
- shifts
- sumas

operaciones todas ellas razonables para un PIC16.

## Por qué hacen falta helpers para dividir y hacer módulo

Dividir es todavía más incómodo que multiplicar.

Por qué:

- tampoco hay instrucción hardware
- además el resultado puede requerir:
  - cociente
  - resto

Cómo se implementa aquí:

- restoring division
- helpers `__rt_div_*` y `__rt_mod_*`

## Qué es restoring division, explicado sin sobrecarga matemática

Definición:

Es un algoritmo que va construyendo el cociente bit a bit mientras mantiene un acumulador parcial y compara/resta contra el divisor.

Por qué existe:

- es una receta sistemática y razonable para hardware sin división nativa

Cómo lo implementa aquí:

- núcleo unsigned
- normalización de signos para casos signed

Ejemplo intuitivo:

```text
13 / 3 = 4 resto 1
```

La idea operativa es:

```text
intentar si el divisor “cabe”
si cabe, restar y marcar bit del cociente
si no cabe, dejar ese bit a cero
repetir
```

## Signed vs unsigned

Definición:

- unsigned: todos los bits se interpretan como magnitud
- signed: el bit alto representa signo

Por qué importa:

- el mismo patrón binario no significa lo mismo
- `>>` no se comporta igual
- división y módulo necesitan cuidado

Cómo se implementa aquí:

- helpers signed separados
- normalización de signos
- restauración final del signo

Ejemplo:

```c
int div16(int a, int b) {
    return a / b;
}
```

Si `a` es negativo, el helper debe tener en cuenta signo y dos complementos, no sólo magnitudes puras.

## Shifts

## Qué es un shift

Definición:

Desplazar bits a izquierda o derecha.

Por qué existe:

- es una operación del lenguaje
- y además es útil como bloque básico para otras operaciones

Cómo se implementa aquí:

- counts constantes inline
- counts dinámicos por helper o bucle
- `>>` unsigned lógico
- `>>` signed aritmético

Ejemplo:

```c
x << 1
```

equivale conceptualmente a multiplicar por 2, salvo detalles de overflow/truncación de anchura fija.

## Qué casos se evitan inline

Phase 7 añade fast paths importantes:

- `x / 2^n` unsigned -> shift right
- `x % 2^n` unsigned -> máscara

Ejemplo:

```c
value / 4
```

puede bajar a:

```text
shift right 2
```

en vez de llamar a `__rt_div_u16`.

## Qué te enseña esto para diseñar tu propio compilador

Una lección crucial:

```text
no toda operación del lenguaje debe bajar inline
ni toda operación compleja debe ir siempre a helper
```

La buena ingeniería está en elegir:

- qué casos son triviales
- qué casos merecen helper
- qué casos son ilegales o ambiguos

---

# 19. ISR e interrupciones en profundidad

## Qué es una interrupción

Definición:

Un mecanismo por el cual el hardware interrumpe temporalmente el flujo normal y fuerza a la CPU a ejecutar una rutina especial.

Por qué existe:

- para reaccionar a eventos en tiempo oportuno

Ejemplos:

- timer
- GPIO
- periféricos

## Qué es una ISR

Definición:

Interrupt Service Routine: rutina que atiende la interrupción.

Cómo se implementa aquí:

- sintaxis: `void __interrupt isr(void)`

Por qué esa sintaxis:

- es simple
- encaja bien con el parser actual
- evita introducir varias formas equivalentes que compliquen el frontend

Ejemplo:

```c
void __interrupt isr(void) {
    if (T0IF) {
        T0IF = 0;
        PORTB = PORTB ^ 0x01;
    }
}
```

## Qué es un vector

Definición:

Una dirección fija de programa a la que el hardware salta automáticamente ante un evento especial.

Por qué existe:

- porque el hardware necesita un punto de entrada conocido

Cómo se implementa aquí:

- reset vector en `0x0000`
- interrupt vector en `0x0004`

Ejemplo:

```text
0x0000 -> camino de arranque
0x0004 -> camino de interrupción
```

## Qué significa guardar contexto

Definición:

Copiar el estado relevante antes de entrar a la ISR para poder restaurarlo al salir.

Por qué existe:

- porque la ISR interrumpe cualquier punto arbitrario del programa
- ese punto podría estar usando registros y slots temporales delicados

Cómo se implementa aquí:

Se guarda:

- `W`
- `STATUS`
- `PCLATH`
- `FSR`
- `return_high`
- `scratch0`
- `scratch1`
- `stack_ptr`
- `frame_ptr`

Ejemplo:

Si el programa normal estaba usando `FSR` para navegar un frame y la ISR lo pisa sin restaurar, el retorno al flujo interrumpido sería corrupto.

## Por qué la ISR está restringida

### Regla

`pic16cc` elige una política conservadora:

- una sola ISR
- no llamadas normales dentro de ISR
- no helpers aritméticos dentro de ISR

### Por qué existe

- porque las ISR son peligrosas
- porque una llamada normal o un helper interno introducirían más cambios de contexto y más presión sobre el estado interrumpido

### Cómo se implementa aquí

- el parser reconoce la ISR
- la semántica valida la firma
- la semántica recorre el cuerpo y rechaza operaciones prohibidas
- el backend emite prologue/epilogue ISR distintos de los normales

### Ejemplo

Válido:

```c
void __interrupt isr(void) {
    if (T0IF) {
        T0IF = 0;
    }
}
```

No válido:

```c
void helper(void) {}

void __interrupt isr(void) {
    helper();
}
```

## Qué es `retfie`

Definición:

La instrucción de retorno de interrupción de PIC16.

Por qué existe:

- volver de una interrupción no es exactamente igual que volver de una llamada normal

Cómo se implementa aquí:

- el backend ISR termina con `retfie`
- el encoder sabe codificarla

Ejemplo:

En `.asm` y `.lst`, una ISR correcta debe terminar con `retfie`, no con `return`.

## Lección para tu propio compilador

Si alguna vez diseñas soporte de interrupciones:

- no empieces por “qué sintaxis bonita quiero”
- empieza por “qué estado del hardware y del runtime debo preservar”

Ésa es la pregunta correcta.

---

# 20. Optimización y calidad del código generado

## Qué significa “optimizar” en este proyecto

No significa “perseguir el código perfecto”.

Significa:

- reducir redundancias
- evitar helpers innecesarios
- simplificar la IR
- no tocar la semántica
- no romper ISR ni ABI

## Peephole optimization

Definición:

Optimización local sobre pequeñas ventanas de instrucciones ya emitidas.

Por qué existe:

- muchas redundancias aparecen sólo al final del codegen

Cómo se implementa aquí:

- en `asm.rs`

Ejemplos:

```text
movf X,w
movwf X
```

puede simplificarse a:

```text
movf X,w
```

Otro ejemplo:

```text
movwf X
movwf X
```

La segunda escritura es redundante.

## Propagación y folding de constantes

Definición:

- propagación: reemplazar usos por constantes ya conocidas
- folding: evaluar operaciones constantes en compilación

Por qué existe:

- evita trabajo en runtime

Cómo se implementa aquí:

- `constant_fold` en la IR

Ejemplo:

```c
if (1) {
    x = 3;
} else {
    x = 4;
}
```

Puede simplificarse a:

- salto directo al bloque verdadero
- bloque falso muerto

## Eliminación de código muerto

Definición:

Eliminar instrucciones o bloques que ya no afectan al resultado.

Cómo se implementa aquí:

- `dead_code_elimination`

Ejemplo:

Un temporal calculado pero nunca usado puede desaparecer.

## Compactación de temporales

Definición:

Reducir el número efectivo de temporales usados tras eliminar muertos.

Por qué importa especialmente aquí:

- porque los temporales viven en el frame
- menos temporales útiles significa menos presión de stack

## Banking y paging más limpios

Phase 7 mejora:

- cambios de `RP0/RP1` sólo cuando hace falta
- eliminación de `setpage` duplicados

Por qué importa:

- en PIC16 cada instrucción cuenta
- muchas pérdidas de calidad vienen de bookkeeping redundante

## `--opt-report`

Definición:

Informe textual de optimizaciones aplicadas.

Por qué existe:

- porque cuando ya optimizas de verdad, conviene hacerlo visible

Cómo se implementa aquí:

- opción CLI
- impresión de estadísticas

Ejemplo:

Puede informar de:

- constantes propagadas
- instrucciones eliminadas
- helpers evitados

## Lección importante

Una optimización pequeña pero correcta vale más que una optimización “brillante” que introduce un bug de ABI o de banking.

Eso es exactamente el tono de Phase 7.

---

# 21. CLI, flujo Linux e integración práctica

## Qué es `picc`

Definición:

Es el binario que usa el usuario final del compilador.

Por qué existe:

- separa el nombre del crate del nombre de la herramienta de línea de comandos

Cómo se usa:

```bash
picc --target pic16f877a -Wall -Wextra -Werror -O2 -I include -o build/main.hex src/main.c
```

## Qué hace la CLI

Coordina:

- parsing de flags
- selección de target
- directorios de include
- nivel de optimización
- warnings
- artefactos opcionales
- ruta de salida `.hex`

## Opciones importantes

### `--target`

Selecciona dispositivo.

Ejemplo:

```bash
--target pic16f628a
```

### `-I`

Añade directorio de include.

### `-o`

Ruta final del `.hex`.

### `--emit-ast`, `--emit-ir`, `--emit-asm`

Piden artefactos de estudio.

### `--map`, `--list-file`

Piden `.map` y `.lst`.

### `--opt-report`

Pide resumen de optimizaciones.

## Qué artefacto es el realmente “programable”

El `.hex`.

Por qué:

- porque el programador externo no consume AST ni IR

Cómo se implementa aquí:

- el path de `-o` es la ruta final del `.hex`

Ejemplo:

```bash
picc --target pic16f628a -I include -o build/blink.hex examples/pic16f628a/blink.c
```

## Makefiles

Por qué importan:

- muestran que el compilador encaja en un flujo Linux normal

Ejemplo de forma:

```make
PIC := picc
TARGET := pic16f877a
CFLAGS := -Wall -Wextra -Werror -O2 -I include
SRC := src/main.c
OUT := build/main.hex

$(OUT): $(SRC)
	mkdir -p build
	$(PIC) --target $(TARGET) $(CFLAGS) -o $(OUT) $(SRC)
```

## Lección de ingeniería

Un compilador no está terminado cuando “genera código”. Está más cerca de estarlo cuando:

- tiene CLI clara
- tiene integración reproducible
- genera artefactos legibles
- se puede automatizar

---

# 22. Diagnósticos y manejo de errores

## Qué es un diagnóstico

Definición:

Mensaje del compilador sobre algo incorrecto o sospechoso.

Por qué existe:

- porque compilar no es sólo aceptar o rechazar
- también es enseñar al usuario qué pasa

Cómo se implementa aquí:

- `DiagnosticBag`
- `DiagnosticEmitter`
- perfiles de warnings

## Error vs warning

### Error

Impide compilación correcta.

### Warning

Señala algo dudoso o potencialmente peligroso.

### `-Werror`

Convierte warnings en errores.

## Narrowing conversions

Definición:

Pasar de un tipo más ancho a otro más estrecho y potencialmente perder información.

Por qué importa:

- en firmware es muy común mover datos entre `int` y registros byte

Cómo se implementa aquí:

- la semántica diagnostica conversiones problemáticas
- constantes representables pueden pasar sin warning innecesario

Ejemplo:

```c
unsigned char x = 300;
```

Eso es sospechoso porque 300 no cabe en un byte.

## División por cero constante

Por qué se diagnostica:

- el compilador lo sabe en compilación

Ejemplo:

```c
return value / 0;
```

## Restricciones del subconjunto

Ejemplos diagnosticados:

- pointer-to-pointer
- function pointers
- arrays multidimensionales
- array initializers no soportados
- recursión

## Restricciones ISR

Ejemplos:

- ISR con retorno no `void`
- ISR con parámetros
- dos ISR
- llamadas normales dentro de ISR
- helpers dentro de ISR

## Lección para tu propio compilador

Los buenos diagnósticos no son un detalle “de UX”.

Son parte del diseño técnico:

- fuerzan a aclarar reglas
- obligan a decidir responsabilidades por etapa
- hacen al compilador más mantenible

---

# 23. Estrategia de pruebas

## Qué se prueba en un compilador

Un compilador puede fallar de muchas formas:

- parsea mal
- tipa mal
- baja mal a IR
- emite asm incorrecto
- rompe el ABI
- genera HEX inválido

Por eso necesita varios niveles de pruebas.

## Unit tests

Qué son:

tests pequeños de piezas aisladas.

Cómo se ven aquí:

- encoder de `retfie`
- peephole patterns
- clasificación de helpers
- constant folding

Por qué existen:

- localizan fallos con precisión

## Integration tests

Qué son:

tests que atraviesan varias etapas o todo el pipeline.

Cómo se ven aquí:

- compilar ejemplos reales
- comprobar `.map`, `.asm`, `.lst`
- comprobar helpers y vectores ISR

Por qué existen:

- muchos bugs sólo aparecen cuando interactúan varias capas

## Regression tests

Qué son:

tests que congelan bugs ya arreglados.

Ejemplos del proyecto:

- sequential calls
- nested calls
- temps across calls
- escapes de stack locals
- helpers prohibidos en ISR

## Qué no se prueba automáticamente todavía

No hay validación automática en hardware real o emulador en CI.

Eso significa:

- sí hay validación de compilación y forma
- no hay ejecución automatizada del binario en silicio

Por qué es importante decirlo:

- porque honestidad técnica también es explicar límites de la validación

## Lección para tu propio compilador

Haz tests por capas.

No dependas sólo de “compilar un ejemplo y mirar si no explota”.

Necesitas:

- microtests
- tests de integración
- tests de regresión

---

# 24. Limitaciones actuales del compilador

## Lenguaje deliberadamente limitado

No soporta:

- `struct`
- `union`
- `enum`
- `float`
- `switch`

Por qué:

- cada uno multiplicaría mucho la complejidad del diseño actual

## Punteros limitados

No soporta:

- puntero a puntero
- punteros a código
- function pointers

Por qué:

- Phase 3 eligió un subconjunto realista y coherente para PIC16

## Recursión no soportada

Por qué:

- stack de software con dimensionamiento estático

## Sin detección dinámica de overflow del stack

Por qué:

- el proyecto usa análisis estático de profundidad
- no implementa comprobación runtime

## ISR conservadora

Por qué:

- seguridad y corrección antes que flexibilidad

## Optimización todavía moderada

Qué significa:

- no hay asignación global de registros
- no hay análisis interprocedimental fuerte
- no hay milagros de tamaño

Por qué:

- se prioriza mantener un compilador entendible y correcto

---

# 25. Trade-offs de diseño y por qué son razonables

## Subconjunto de C en vez de C completo

Trade-off:

- menos cobertura del lenguaje
- más coherencia del sistema

Razón:

- PIC16 ya ofrece suficiente complejidad técnica por sí mismo

## Backend compartido + descriptores

Trade-off:

- exige diseñar una abstracción familiar
- evita duplicación por dispositivo

Razón:

- los dos chips actuales comparten mucha semántica de backend

## Stack-first ABI

Trade-off:

- más complejidad backend
- mucha más robustez semántica

Razón:

- sin eso, las llamadas profundas y temporales por invocación son frágiles

## Helpers

Trade-off:

- más runtime interno
- soporte aritmético real y reutilizable

## ISR restringida

Trade-off:

- menos expresividad
- mayor seguridad del estado interrumpido

## Optimizaciones conservadoras

Trade-off:

- menos agresividad
- menos riesgo de miscompilación

## Gran lección de ingeniería

El diseño de un compilador bueno no consiste en añadir la mayor cantidad de features posible.

Consiste en:

- elegir bien el orden
- cerrar contratos claros
- no mentir sobre lo que realmente soporta el backend

Ése es uno de los mayores méritos de este repositorio.

---

# 26. Cómo usar este proyecto como base para construir tu propio compilador

Ésta es quizá la parte más útil si tu objetivo no es sólo entender `pic16cc`, sino empezar a construir algo inspirado en él.

## Lección 1: separa etapas pronto

No mezcles todo en un único archivo “parser que ya genera asm”.

Aunque el lenguaje sea pequeño, separa:

- lexer
- parser
- semántica
- IR
- backend

Razón:

- cada fase resuelve un problema distinto

## Lección 2: define tu subconjunto con honestidad

No digas “soporto C” si en realidad soportas unas pocas construcciones.

Haz lo que hace bien este proyecto:

- define límites
- diagnostica lo no soportado
- documenta trade-offs

## Lección 3: diseña tu ABI antes de que sea demasiado tarde

En cuanto tu lenguaje tenga:

- funciones
- argumentos múltiples
- locales
- llamadas anidadas

necesitas un ABI coherente.

Si lo pospones demasiado, el backend se llenará de parches.

## Lección 4: trata la memoria como problema de primer orden

Especialmente en micros pequeños:

- dónde viven las cosas
- cómo se direccionan
- cuánto ocupan

no es un detalle; es parte central del compilador.

## Lección 5: usa una IR aunque tu compilador sea pequeño

Razón:

- te ayudará a no acoplar frontend y backend
- te dará un punto natural para optimizar

## Lección 6: empieza con optimizaciones pequeñas y seguras

Ejemplos buenos:

- constant folding
- DCE simple
- peephole local

Ejemplos que conviene dejar para después:

- asignación global de registros
- análisis interprocedimental complejo

## Lección 7: diseña pruebas desde el principio

Un compilador sin tests se degrada muy rápido.

Mínimos recomendables:

- parser tests
- semantic tests
- IR tests
- backend shape tests
- regression tests

## Lección 8: documenta tus límites

Esto no es “marketing negativo”.

Es diseño profesional.

Un compilador serio debe decir claramente:

- qué soporta
- qué no
- por qué

## Arquitectura recomendada si empezaras hoy

Una arquitectura sencilla, inspirada en `pic16cc`, sería:

```text
frontend/
  lexer
  parser
  semantic

ir/
  model
  lowering
  passes

backend/
  device descriptors
  codegen
  asm
  encoder
  runtime helpers

driver/
  cli
  orchestration
```

Ése es un punto de partida excelente para un compilador pequeño pero serio.

---

# 27. Cómo explicar este proyecto en una entrevista o defensa

## Versión de 2 minutos

“`pic16cc` es un compilador en Rust para PIC16 clásicos de 14 bits. Toma un subconjunto acotado de C y genera Intel HEX real. Lo más interesante del proyecto es que no se limita a parsear: tiene frontend, análisis semántico, una IR propia, un backend compartido para la familia `midrange14`, un ABI stack-first con stack de software, runtime helpers para operaciones que el hardware no implementa y soporte de ISR con guardado de contexto. Es un caso de estudio muy bueno porque PIC16 obliga a hacer explícitos problemas que en arquitecturas modernas suelen estar ocultos: banking, paging, falta de pila de datos, aritmética de 16 bits e interrupciones seguras.” 

## Versión de 5 minutos

“El compilador está organizado por capas. El frontend hace preprocesado, lexing, parsing y análisis semántico. Después el programa se baja a una IR basada en bloques y temporales tipados. Esa IR hace explícitas cosas como casts, llamadas y acceso indirecto, y además es el sitio natural para optimizaciones simples. Luego entra el backend compartido PIC16, que traduce esa IR a ensamblador interno, decide banking, paging, prologues, epilogues y helpers runtime, y finalmente codifica a palabras de 14 bits e Intel HEX. La evolución por fases es muy instructiva: primero un pipeline mínimo, luego 16 bits, luego arrays y punteros, luego un ABI serio con stack de software, luego helpers aritméticos, luego ISR y finalmente optimización conservadora. La gran decisión arquitectónica es Phase 4, porque convierte las llamadas en un modelo realmente robusto sobre un hardware que no tiene pila de datos general.” 

## Preguntas habituales y respuestas

### “¿Por qué una IR propia?”

Porque separa frontend y backend y hace explícitas operaciones que el backend PIC16 necesita, como acceso indirecto, casts y llamadas.

### “¿Por qué una pila de software?”

Porque la pila hardware del PIC16 sólo sirve para retorno de llamadas, no para argumentos, locales o temporales.

### “¿Por qué no compilar C completo?”

Porque el objetivo del proyecto es coherencia real extremo a extremo, no cobertura superficial del lenguaje.

### “¿Por qué los helpers usan el mismo ABI?”

Porque mantener un solo contrato de llamada reduce complejidad y evita mezclar dos modelos internos.

### “¿Por qué la ISR es tan conservadora?”

Porque interrumpe cualquier estado parcial del programa. Es una zona donde la corrección vale más que la expresividad.

---

# 28. Glosario

## ABI

Contrato binario entre caller y callee.

En este proyecto define:

- cómo se pasan argumentos
- cómo se devuelven valores
- quién limpia el stack

Ejemplo:

8 bits retornan en `W`; 16 bits retornan en `W + return_high`.

## AST

Árbol de sintaxis abstracta.

Representa la estructura del programa sin el ruido del texto original.

Ejemplo:

```text
x = a + b
```

se representa como un árbol con `=` y `+`.

## Backend

Parte del compilador que conoce la arquitectura destino y baja la IR a instrucciones reales.

## Banking

Mecanismo de selección de banco de RAM mediante `STATUS.RP0/RP1`.

## CFG

Control Flow Graph.

Representación de bloques y transiciones de control.

## Encoder

Etapa que convierte instrucciones simbólicas en palabras máquina.

## FSR

Registro usado para direccionamiento indirecto.

## Frame

Bloque del stack que pertenece a una invocación concreta de una función.

## IR

Intermediate Representation.

Representación interna entre frontend y backend.

## INDF

Ventana de acceso al contenido apuntado por `FSR`.

## ISR

Interrupt Service Routine.

Rutina ejecutada al atender una interrupción.

## Lexer

Etapa que transforma caracteres en tokens.

## Lowering

Traducción de una representación más abstracta a otra más explícita y cercana a la máquina.

## Lvalue

Expresión que representa una ubicación escribible de memoria.

## PCLATH

Registro usado para completar direcciones de llamadas y saltos entre páginas.

## Peephole optimization

Optimización local sobre ventanas pequeñas de instrucciones ya emitidas.

## Preprocesador

Etapa textual previa al parser que resuelve includes y macros.

## Rvalue

Valor calculado, no escribible directamente.

## Semantic analysis

Etapa que valida nombres, tipos y reglas del lenguaje.

## SFR

Special Function Register.

Registro especial del hardware, como `PORTB`.

## Stack

Estructura LIFO usada aquí como pila de software.

## SP

Stack Pointer.

## FP

Frame Pointer.

## Token

Unidad léxica mínima con sentido para el parser.

---

# 29. Conclusión final

La mejor manera de resumir `pic16cc` es ésta:

Es un compilador pequeño, pero no trivial.

Su valor no está en “cuántas features de C soporta”, sino en otra cosa mucho más útil para aprender diseño de compiladores:

- cada capa existe por una razón
- cada fase resuelve un problema real
- cada limitación está relacionada con una decisión técnica concreta
- el hardware PIC16 obliga a hacer visibles los contratos internos

Si entiendes bien este proyecto, entiendes varias ideas profundas de construcción de compiladores:

- por qué un parser no basta
- por qué la semántica es crítica
- por qué la IR no es un lujo académico
- por qué el ABI es un contrato central y no un detalle
- por qué el hardware condiciona el diseño
- por qué optimizar sin romper semántica es un trabajo delicado

Y si además quisieras empezar tu propio compilador, este repositorio te deja una enseñanza especialmente valiosa:

```text
empieza por definir bien tus contratos internos,
sé honesto con el subconjunto que soportas,
y construye cada capa para resolver un problema concreto.
```

Ésa es una de las mejores formas de pasar de “leer sobre compiladores” a “construir uno de verdad”.

---

# 30. Apéndice A: anatomía del repositorio y cómo leer el código sin perderse

Una de las grandes dificultades de quien empieza a estudiar compiladores no es sólo entender los conceptos. Es saber por dónde entrar en un repositorio real.

Este apéndice existe para resolver justamente eso.

## Qué es “leer un compilador” de forma productiva

Definición:

Leer un compilador de forma productiva no es abrir archivos al azar. Es seguir el flujo real del programa y entender qué responsabilidad tiene cada módulo.

Por qué existe esta necesidad:

- porque un compilador tiene muchas capas
- porque si empiezas por el archivo equivocado puedes ver detalle sin contexto

Cómo conviene hacerlo aquí:

1. empezar por la entrada CLI
2. seguir la función que orquesta el pipeline
3. bajar capa por capa
4. volver a subir con ejemplos concretos

Ejemplo:

Si empiezas leyendo `codegen.rs` sin haber entendido semántica ni IR, verás muchas decisiones PIC16 pero te faltará el “por qué” de cada una.

## Punto de entrada: `src/main.rs`

Qué es:

el ejecutable real del compilador.

Por qué existe:

- porque alguien tiene que recibir la línea de comandos
- alguien tiene que decidir código de salida del proceso

Cómo se implementa aquí:

- parsea opciones con `CliOptions::parse`
- llama a `execute`
- si hay error de parseo CLI, sale con código `2`
- si hay diagnóstico de compilación, sale con código `1`

Ejemplo conceptual:

```text
argv -> parse CLI -> execute(options)
```

Fragmento literal representativo de `src/main.rs`:

```rust
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let options = match pic16cc::cli::CliOptions::parse(args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };

    if let Err(error) = pic16cc::execute(options) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
```

Qué enseña este bloque:

- `main` es deliberadamente pequeño
- la lógica real vive fuera
- los códigos de salida CLI y compilación están separados

## Orquestación principal: `src/lib.rs`

Qué es:

la pieza que conecta todas las fases.

Por qué existe:

- para centralizar el pipeline
- para que el `main` sea mínimo

Cómo se implementa aquí:

`execute` decide entre:

- compilar
- listar targets
- mostrar ayuda
- mostrar versión

Y `compile_command` hace el recorrido principal:

1. resolver target
2. cargar fuente
3. preprocesar
4. lexear
5. parsear
6. analizar semánticamente
7. bajar a IR
8. optimizar IR
9. compilar backend
10. emitir artefactos
11. escribir `.hex`

Ejemplo:

Este archivo es excelente para una primera lectura seria del compilador, porque te da el mapa general sin hundirte todavía en detalles de una sola fase.

Fragmento literal representativo de `src/lib.rs`:

```rust
let mut lexer = Lexer::new(&preprocessed, &mut diagnostics);
let tokens = lexer.tokenize();
if diagnostics.has_errors() {
    return Err(diagnostics);
}

let mut parser = Parser::new(tokens, &preprocessed, &mut diagnostics);
let ast: TranslationUnit = parser.parse_translation_unit();
if diagnostics.has_errors() {
    return Err(diagnostics);
}

let semantic = SemanticAnalyzer::new(target);
let typed_program = semantic.analyze(ast, &mut diagnostics);
if diagnostics.has_errors() {
    return Err(diagnostics);
}
let typed_program = typed_program.expect("semantic result checked");
```

Este bloque es importante porque deja ver transición real:

```text
texto preprocesado -> tokens -> AST -> programa tipado
```

## CLI: `src/cli/mod.rs`

Qué es:

el contrato externo del compilador.

Por qué existe:

- porque alguien tiene que traducir opciones de usuario a estructura interna
- porque así `main` no mezcla parsing de argumentos con compilación

Cómo se implementa aquí:

- reconoce `--target`, `-I`, `-D`, `-O*`
- activa artefactos como `--emit-ir` o `--emit-asm`
- valida errores de forma antes de entrar al pipeline real

Fragmento literal representativo de `src/cli/mod.rs`:

```rust
match argument.as_str() {
    "--help" | "-h" => return Ok(Self { command: CliCommand::Help }),
    "--version" => return Ok(Self { command: CliCommand::Version }),
    "--list-targets" => return Ok(Self { command: CliCommand::ListTargets }),
    "--emit-tokens" => artifacts.emit_tokens = true,
    "--emit-ast" => artifacts.emit_ast = true,
    "--emit-ir" => artifacts.emit_ir = true,
    "--emit-asm" => artifacts.emit_asm = true,
    "--map" => artifacts.map = true,
    "--list-file" => artifacts.list_file = true,
    "--verbose" => verbose = true,
    "--opt-report" => opt_report = true,
    "-Wall" => warning_profile.wall = true,
    "-Wextra" => warning_profile.wextra = true,
    "-Werror" => warning_profile.werror = true,
```

Qué enseña:

- la CLI del proyecto es explícita
- los artefactos de estudio forman parte del producto
- el libro puede referirse a flags reales, no inventados

## Frontend: `src/frontend`

### `preprocessor.rs`

Responsabilidad:

- includes
- macros objeto
- condicionales del preprocesador

Fragmento literal de `src/frontend/preprocessor.rs`:

```rust
pub struct Preprocessor<'a> {
    target: &'a TargetDevice,
    include_dirs: Vec<PathBuf>,
    macros: BTreeMap<String, MacroDef>,
    source_manager: &'a mut SourceManager,
    include_guard: HashSet<PathBuf>,
}
```

Este bloque ya comunica diseño:

- depende del target
- mantiene macros
- preserva acceso a fuentes reales
- evita inclusión recursiva accidental

### `lexer.rs`

Responsabilidad:

- convertir texto en tokens

Fragmento literal de `src/frontend/lexer.rs`:

```rust
pub enum TokenKind {
    Identifier(String),
    Number(i64),
    Keyword(Keyword),
    Symbol(Symbol),
    Eof,
}
```

Qué enseña:

- el token ya clasifica sin semántica
- los enteros ya se materializan como valores
- EOF explícito simplifica parser

### `parser.rs`

Responsabilidad:

- construir AST
- manejar precedencia
- reconocer declaraciones, sentencias y expresiones

Fragmento literal de `src/frontend/parser.rs`:

```rust
if self.match_keyword(Keyword::If) {
    self.expect_symbol(Symbol::LParen);
    let condition = self.parse_expression();
    self.expect_symbol(Symbol::RParen);
    let then_branch = Box::new(self.parse_statement());
    let else_branch = if self.match_keyword(Keyword::Else) {
        Some(Box::new(self.parse_statement()))
    } else {
        None
    };
    return Stmt::If {
        condition,
        then_branch,
        else_branch,
        span: Span::new(start, self.previous_span().end),
    };
}
```

Esto es visualmente útil porque el lector ve:

- dónde nace un `if` en el AST
- cómo el parser consume tokens en orden
- cómo se conservan `span` para diagnósticos

### `semantic.rs`

Responsabilidad:

- símbolos
- tipos
- lvalue/rvalue
- casts
- restricciones del subconjunto
- validaciones Phase 3/4/5/6

Fragmento literal de `src/frontend/semantic.rs`:

```rust
fn seed_device_registers(&mut self) {
    for register in self.target.sfrs {
        let symbol = self.insert_symbol(Symbol {
            id: self.symbols.len(),
            name: register.name.to_string(),
            ty: Type::new(ScalarType::U8).with_qualifiers(Qualifiers {
                is_const: false,
                is_volatile: true,
            }),
            storage_class: StorageClass::Extern,
            is_interrupt: false,
            kind: SymbolKind::DeviceRegister,
            span: Span::new(0, 0),
            fixed_address: Some(register.address),
            is_defined: true,
            is_referenced: false,
            parameter_types: Vec::new(),
        });
```

Este bloque es de los más pedagógicos del proyecto porque muestra literalmente cómo `PORTA`, `PORTB`, `TRISB` o `INTCON` entran al compilador como símbolos reales.

## IR: `src/ir`

### `model.rs`

Responsabilidad:

- definir la forma de la IR

Fragmento literal de `src/ir/model.rs`:

```rust
pub enum IrTerminator {
    Return(Option<Operand>),
    Jump(BlockId),
    Branch {
        condition: IrCondition,
        then_block: BlockId,
        else_block: BlockId,
    },
    Unreachable,
}
```

Qué enseña:

- el control flow ya no es sintaxis
- ahora es grafo explícito
- `Return`, `Jump` y `Branch` son casos distintos y visibles

### `lowering.rs`

Responsabilidad:

- transformar el programa tipado en IR

Fragmento literal de `src/ir/lowering.rs`:

```rust
TypedStmt::While {
    condition, body, ..
} => {
    let header = self.new_block("while.head");
    let body_block = self.new_block("while.body");
    let end = self.new_block("while.end");
    self.ensure_jump(header);

    self.current = header;
    self.loop_stack.push((header, end));
    self.lower_condition(condition, body_block, end);
```

Este bloque vuelve muy visual el paso de:

```text
while fuente -> header + body + end
```

### `passes.rs`

Responsabilidad:

- optimizaciones IR

Fragmento literal de `src/ir/passes.rs`:

```rust
if let (Operand::Constant(lhs), Operand::Constant(rhs)) = (lhs, rhs) {
    let ty = function.temp_types[dst];
    let result = eval_binary(op, lhs, rhs, ty, ty);
    *instr = IrInstr::Copy {
        dst,
        src: Operand::Constant(result),
    };
    constants.insert(dst, result);
    stats.expressions_folded += 1;
}
```

Qué enseña:

- el pass no “piensa” en alto nivel
- reescribe instrucciones concretas
- una expresión constante deja de ser `Binary` y pasa a ser `Copy`

## Backend: `src/backend/pic16`

### `devices.rs`

Responsabilidad:

- describir chips concretos

Fragmento literal representativo:

```text
target descriptor -> vectores, RAM, bancos, SFR, config word
```

Aquí conviene recordar algo importante:

- `devices.rs` no es decoración
- es lo que permite compartir backend entre `PIC16F628A` y `PIC16F877A`

### `midrange14/codegen.rs`

Responsabilidad:

- bajar IR a asm interno
- stack ABI
- helpers
- ISR
- banking/paging

Fragmento literal de `src/backend/pic16/midrange14/codegen.rs`:

```rust
self.program.push(AsmLine::Comment(format!(
    "frame args={} saved_fp={} locals={} temps={} frame_bytes={}",
    arg_bytes, saved_fp_offset, local_bytes, temp_bytes, frame_bytes
)));
self.current_bank = UNKNOWN_BANK;
if function.is_interrupt {
    self.program.push(AsmLine::Comment(
        "interrupt context save + isolated stack frame".to_string(),
    ));
    self.emit_interrupt_prologue(function.symbol);
} else {
    self.emit_prologue(function.symbol);
}
```

Este bloque es muy ilustrativo porque muestra:

- el backend deja huellas didácticas en asm
- el ABI real aparece impreso
- ISR y función normal divergen desde prólogo

### `midrange14/asm.rs`

Responsabilidad:

- representar instrucciones simbólicas
- peephole optimization

Fragmento conceptual útil:

```text
asm interno = representación antes de codificar a palabra PIC16
```

Este punto importa porque el compilador no genera HEX directamente desde IR.

### `midrange14/encoder.rs`

Responsabilidad:

- codificar instrucciones a palabras

Fragmento conceptual útil:

```text
asm simbólico -> palabras de 14 bits -> registros Intel HEX
```

### `midrange14/runtime.rs`

Responsabilidad:

- clasificación y soporte de helpers aritméticos

Fragmento literal de `src/backend/pic16/midrange14/runtime.rs`:

```rust
Self::MulU16 => RuntimeHelperInfo {
    label: "__rt_mul_u16",
    operand_ty: Type::new(ScalarType::U16),
    arg_bytes: 4,
    local_bytes: 4,
    frame_bytes: 6,
},
Self::DivU16 => RuntimeHelperInfo {
    label: "__rt_div_u16",
    operand_ty: Type::new(ScalarType::U16),
    arg_bytes: 4,
    local_bytes: 4,
    frame_bytes: 6,
},
```

Qué enseña:

- cada helper tiene contrato
- el coste de frame está explicitado
- el runtime no es “caja negra”

## Salida: otras carpetas clave

### `src/assembler`

Responsabilidad:

- render de listados

### `src/linker`

Responsabilidad:

- `map` de símbolos

### `src/hex`

Responsabilidad:

- Intel HEX

## Tests: `tests/compiler_pipeline.rs`

Qué es:

el sitio donde mejor se ve el comportamiento esperado “de extremo a extremo”.

Por qué es tan valioso:

- porque enseña tanto como la implementación
- congela regresiones importantes

Cómo conviene leerlo:

- por grupos de fase
- buscando nombres de fixtures y comentarios

Ejemplo:

Los tests de nested calls, pointer escapes e ISR prohibiendo helpers cuentan la historia de los problemas reales que el proyecto tuvo que cerrar.

Fragmento literal de `tests/compiler_pipeline.rs`:

```rust
fn assert_phase4_stack_metadata(output: &Path) {
    let asm = read_artifact(output, "asm");
    let map = read_artifact(output, "map");
    let listing = read_artifact(output, "lst");

    assert!(asm.contains("frame args="));
    assert!(asm.contains("stack base="));
    assert!(map.contains("__abi.stack_ptr.lo"));
    assert!(map.contains("__abi.frame_ptr.lo"));
    assert!(map.contains("__stack.base"));
    assert!(map.contains("__stack.end"));
    assert!(listing.contains("frame args="));
}
```

Este bloque es importante porque enseña algo muy valioso:

- los tests no sólo miran “compila o no”
- también verifican rastros concretos del ABI y del backend

## Método recomendado para leer el repositorio

Orden recomendado:

1. `src/main.rs`
2. `src/lib.rs`
3. `src/cli/mod.rs`
4. `src/frontend/*`
5. `src/ir/model.rs`
6. `src/ir/lowering.rs`
7. `src/ir/passes.rs`
8. `src/backend/pic16/devices.rs`
9. `src/backend/pic16/midrange14/codegen.rs`
10. `tests/compiler_pipeline.rs`

Por qué ese orden:

- va de lo general a lo específico
- primero entiendes flujo
- luego representación
- luego detalles de backend

---

# 31. Apéndice B: caso práctico completo 1, de una función sencilla a PIC16

Vamos a seguir este programa con máximo detalle conceptual:

```c
#include <pic16/pic16f628a.h>

unsigned char add1(unsigned char x) {
    return x + 1;
}

void main(void) {
    TRISB = 0x00;
    PORTB = add1(3);
}
```

## Paso 1: qué ve el usuario

El usuario ve dos funciones:

- `add1`
- `main`

Y una línea importante:

```c
PORTB = add1(3);
```

Intuitivamente eso parece trivial.

Pero para el compilador real contiene varios problemas:

- llamada a función
- parámetro de 8 bits
- retorno de 8 bits
- escritura a SFR
- manejo de bancos

## Paso 2: qué ve el lexer

Trozo:

```c
return x + 1;
```

Tokens:

```text
KW_RETURN
IDENT(x)
PLUS
INT_LITERAL(1)
SEMICOLON
```

Qué se gana:

- ya no trabajas con caracteres
- trabajas con piezas clasificadas

## Paso 3: qué ve el parser

AST conceptual:

```text
return
  +
 / \
x   1
```

Y para la llamada:

```text
assign
 /    \
PORTB  call add1
          |
          3
```

Qué se gana:

- ya hay estructura
- todavía no hay tipos resueltos del todo

## Paso 4: qué decide la semántica

La semántica determina:

- `x` es `unsigned char`
- `1` se adapta al ancho correcto
- `add1` recibe un parámetro
- `add1` devuelve `unsigned char`
- `PORTB` es un SFR válido del target

También clasifica:

- `PORTB` es lvalue
- `add1(3)` es rvalue

## Paso 5: qué forma toma en IR

Versión simplificada, pedagógica:

```text
fn add1:
  t0 = x + 1
  return t0

fn main:
  TRISB = 0
  t0 = call add1(3)
  PORTB = t0
  return void
```

Qué se gana:

- la llamada ya es una operación explícita
- la suma ya es una operación explícita
- el backend no necesita mirar AST

## Paso 6: qué hace el ABI

`add1` recibe un parámetro de 1 byte.

Con el ABI actual:

- el caller empuja ese byte
- hace `call`
- el callee crea frame
- el retorno de 8 bits sale por `W`
- el caller limpia el byte del argumento

Diagrama simplificado de la llamada:

```text
main frame
  push 3
  call add1

add1 frame
  arg x
  saved FP
  temp t0
```

## Paso 7: qué hace el backend con `return x + 1`

Conceptualmente:

1. leer `x` desde el frame
2. cargarlo en `W`
3. sumar 1
4. dejar el resultado en `W`
5. epilogue

Por qué no hace falta `return_high`:

- porque el valor cabe en un byte

## Paso 8: qué hace el backend con `PORTB = add1(3)`

Conceptualmente:

1. empujar argumento `3`
2. llamar a `add1`
3. leer retorno en `W`
4. seleccionar banco de `PORTB` si hace falta
5. escribir `W` a `PORTB`

## Paso 9: qué artefactos te ayudan a estudiarlo

### AST

Te enseña estructura.

### IR

Te enseña lowering.

### ASM

Te enseña ABI y codegen.

### MAP

Te enseña símbolos y ubicaciones.

### LST

Te enseña la forma final legible del programa.

## Qué aprender de este caso si quieres construir tu propio compilador

Incluso un ejemplo diminuto ya toca casi todas las capas importantes:

- parser
- semántica
- IR
- ABI
- backend
- artefactos finales

Ésa es una lección poderosa: no hace falta un lenguaje grande para aprender problemas reales de compiladores.

---

# 32. Apéndice C: caso práctico completo 2, arrays y punteros

Programa conceptual:

```c
unsigned int words[2];

void main(void) {
    unsigned int *p = words;
    words[0] = 0x0123;
    words[1] = 0x0040;
    PORTB = p[0];
}
```

## Qué nuevos problemas aparecen

Ya no basta con:

- sumar
- llamar
- escribir un símbolo fijo

Ahora aparecen:

- array global
- decay a puntero
- puntero de 16 bits
- indexación
- acceso indirecto

## Qué significa `unsigned int words[2]`

Semánticamente:

- array de 2 elementos
- cada elemento ocupa 2 bytes

Layout conceptual:

```text
words[0].low
words[0].high
words[1].low
words[1].high
```

## Qué significa `unsigned int *p = words`

En C, el nombre del array en contexto de valor decae a puntero a su primer elemento.

Qué es “decay”:

transformar el array, en contexto de valor, en una dirección base.

Por qué existe:

- porque el lenguaje C usa mucho esta conversión implícita

Cómo se implementa aquí:

- la semántica la hace explícita
- el lowering la convierte en `AddrOf`

## Qué significa `p[0]`

No es una magia especial distinta de los punteros.

Conceptualmente:

```text
p[0] == *(p + 0)
```

Si fuera `p[1]`, sería:

```text
*(p + 1 * tamaño_del_elemento)
```

Como el elemento es `unsigned int`, el tamaño es 2.

## Qué hace el lowering

Versión conceptual:

```text
t0 = &words
p = t0
t1 = p + 0
t2 = load_indirect(t1)
PORTB = low_byte(t2)
```

Para `words[1] = 0x0040`, la idea sería:

```text
base = &words
addr = base + 2
store_indirect(addr, 0x0040)
```

## Qué hace el backend

Para un `load_indirect`:

1. cargar dirección a `FSR`
2. ajustar `IRP`
3. leer `INDF`
4. si es 16-bit, repetir para el segundo byte

Esto es exactamente el punto en que se ve por qué Phase 3 necesitó introducir una IR de memoria explícita.

## Qué aprender de este caso

Si diseñas tu propio compilador y llegas a arrays y punteros, la gran pregunta ya no será “cómo parseo `[]`”.

La gran pregunta será:

```text
¿cuándo convierto sintaxis de alto nivel en dirección + desplazamiento + acceso indirecto?
```

En este repositorio, la respuesta correcta es:

- durante semántica y lowering
- no como parche tardío en el backend mirando AST

---

# 33. Apéndice D: caso práctico completo 3, llamadas anidadas y frames

Programa:

```c
int f(int x) { return x + 1; }
int g(int y) { return f(y) + 2; }
int h(int z) { return g(z) + 3; }

int main(void) {
    return h(5);
}
```

## Qué problema ilustra este caso

Ilustra por qué el ABI y el frame model no son un lujo, sino una necesidad.

Si no hubiera frames:

- `f`, `g` y `h` podrían pelear por las mismas ubicaciones temporales
- el retorno intermedio podría corromperse

## Qué hace el caller

Por ejemplo, cuando `g` llama a `f`:

1. evalúa `y`
2. empuja bytes de `y`
3. hace `call f`
4. limpia los bytes de argumento
5. usa el retorno

## Qué hace el callee

En `f`:

1. guarda el `FP` anterior
2. fija su `FP`
3. reserva temporales
4. computa `x + 1`
5. devuelve el resultado
6. restaura `FP`

## Diagrama de anidamiento

```text
frame main
  frame h
    frame g
      frame f
```

Si `f` termina:

```text
frame main
  frame h
    frame g
```

Ese orden de vida es exactamente lo que el stack model resuelve bien.

## Qué te enseña esto

Muchísimas veces el “primer ABI que funciona” no es el ABI que escala.

Phase 4 muestra el momento en que el compilador deja de ser un experimento de funciones pequeñas y empieza a comportarse como un compilador de verdad con llamadas generales.

---

# 34. Apéndice E: caso práctico completo 4, multiplicación y helpers

Programa:

```c
unsigned int mul16(unsigned int a, unsigned int b) {
    return a * b;
}
```

## Qué problema ilustra

Que el lenguaje pide una operación que la ISA no ofrece directamente.

## Qué decisiones son posibles

El compilador podría:

1. expandir siempre inline una rutina larga
2. llamar siempre a un helper
3. mezclar inline en casos simples y helper en casos complejos

`pic16cc` elige la tercera.

## Qué pasa en un caso general

Para `a * b` de 16 bits:

- se selecciona `__rt_mul_u16` o `__rt_mul_i16`
- se empujan argumentos
- se llama al helper
- el helper usa su frame
- devuelve 16 bits en `W + return_high`

## Qué pasa en un caso barato

Si la operación fuera:

```c
value * 2
```

el compilador puede convertirlo en:

```text
shift left 1
```

sin helper.

## Qué aprender de este caso

El lenguaje y la ISA no tienen por qué alinearse.

Ahí es donde un compilador deja de ser “traductor sintáctico” y se convierte en diseñador de estrategias de implementación.

---

# 35. Apéndice F: caso práctico completo 5, interrupción y contexto

Programa conceptual:

```c
void __interrupt isr(void) {
    if (T0IF) {
        T0IF = 0;
        PORTB = PORTB ^ 0x01;
    }
}
```

## Qué problema ilustra

Que una ISR no es “otra función más”.

Interrumpe un estado arbitrario del programa.

## Qué debe ocurrir antes del cuerpo de la ISR

El prologue ISR debe guardar:

- `W`
- `STATUS`
- `PCLATH`
- `FSR`
- slots ABI críticos
- stack/frame pointers

## Qué puede hacer el cuerpo

Bajo las restricciones actuales:

- leer y escribir SFR
- hacer comparaciones simples
- operar inline

## Qué no puede hacer

- llamar funciones normales
- disparar helpers aritméticos

## Qué debe ocurrir al salir

El epilogue ISR:

1. restaura contexto
2. deja la CPU coherente
3. termina con `retfie`

## Qué aprender de este caso

Una ISR es una frontera excelente para distinguir dos estilos de diseño:

- diseño optimista: “ya veremos qué hace falta salvar”
- diseño conservador: “salvemos claramente todo lo necesario y restrinjamos lo peligroso”

Este proyecto eligió, con buen criterio, el segundo.

---

# 36. Apéndice G: método recomendado para construir tu propio compilador inspirado en éste

Si alguien quisiera empezar su propio compilador para un microcontrolador parecido, un camino razonable sería éste.

## Etapa 1: lenguaje mínimo y pipeline completo

Objetivo:

- un pequeño subconjunto
- variables
- asignaciones
- `if`
- `return`
- `.hex` real al final

No empieces por:

- structs
- punteros complejos
- ISR

Empieza por:

- demostrar que toda la cadena vive

## Etapa 2: tipos más anchos y comparaciones

Añade:

- enteros de 16 bits
- signed vs unsigned
- booleanos explícitos

Razón:

- te obligará a cerrar representación y casts

## Etapa 3: memoria real

Añade:

- arrays
- `&`
- `*`
- loads/stores indirectos

Razón:

- aquí el compilador deja de ser puramente aritmético y entra en el mundo real de C

## Etapa 4: ABI serio

Hazlo antes de que tu compilador crezca demasiado.

Si esperas demasiado:

- el coste de migración será alto

## Etapa 5: runtime helpers

Sólo cuando tu ABI ya sea robusto.

Razón:

- los helpers son llamadas internas generadas por el compilador
- si tu ABI es frágil, lo descubrirás muy pronto

## Etapa 6: interrupciones

Sólo cuando:

- ya tengas backend estable
- ya entiendas qué estado puede estar vivo en cualquier punto

## Etapa 7: optimización

Sólo cuando:

- ya confíes en la corrección base

Empieza por:

- constantes
- DCE
- peephole

No por:

- register allocation global compleja

## Qué documentos te conviene escribir en tu propio proyecto

Inspirado en este repositorio, conviene documentar:

- ABI
- memory model
- lowering de llamadas
- helpers
- ISR
- optimización

No por burocracia, sino porque documentar bien obliga a pensar bien.

---

# 37. Apéndice H: lectura guiada del orquestador real

## Por qué existe este apéndice

Hasta ahora el libro ha explicado conceptos, arquitectura y evolución histórica.

Este apéndice cambia de ángulo:

- deja de mirar el compilador “desde lejos”
- empieza a leer el código real que une todas las piezas
- usa el repositorio como texto principal de estudio

La pregunta que guía esta parte no es sólo “qué hace el compilador”.

La pregunta es:

```text
¿en qué archivo exacto ocurre cada decisión importante?
```

## El corazón real del pipeline está en `src/lib.rs`

Si hubiera que enseñar este compilador abriendo un solo archivo primero, sería `src/lib.rs`.

Ahí aparece la ruta completa, en orden, sin metáforas:

```rust
    let mut preprocessor = Preprocessor::new(
        target,
        command.include_dirs.clone(),
        command.defines.clone(),
        &mut source_manager,
    );
    let preprocessed = preprocessor.process(main_source, &mut diagnostics);
    if diagnostics.has_errors() {
        return Err(diagnostics);
    }

    let preprocessed = preprocessed.expect("preprocessor result checked");
    if command.artifacts.emit_tokens {
        let tokens = Lexer::new(&preprocessed, &mut diagnostics).collect_debug();
        write_artifact(&command.output, "tokens", &tokens)?;
    }

    let mut lexer = Lexer::new(&preprocessed, &mut diagnostics);
    let tokens = lexer.tokenize();
    if diagnostics.has_errors() {
        return Err(diagnostics);
    }

    let mut parser = Parser::new(tokens, &preprocessed, &mut diagnostics);
    let ast: TranslationUnit = parser.parse_translation_unit();
    if diagnostics.has_errors() {
        return Err(diagnostics);
    }

    if command.artifacts.emit_ast {
        write_artifact(&command.output, "ast", &ast.render())?;
    }

    let semantic = SemanticAnalyzer::new(target);
    let typed_program = semantic.analyze(ast, &mut diagnostics);
    if diagnostics.has_errors() {
        return Err(diagnostics);
    }
    let typed_program = typed_program.expect("semantic result checked");

    let mut ir_program = IrLowerer::new(target).lower(&typed_program, &mut diagnostics);
    if diagnostics.has_errors() {
        return Err(diagnostics);
    }
```

Y después continúa así:

```rust
    let mut optimization_report = OptimizationReport::default();
    match command.optimization {
        OptimizationLevel::O0 => {}
        OptimizationLevel::O1 | OptimizationLevel::O2 | OptimizationLevel::Os => {
            optimization_report.constant_fold = constant_fold(&mut ir_program);
            optimization_report.dead_code = dead_code_elimination(&mut ir_program);
            optimization_report.temp_compaction = compact_temps(&mut ir_program);
        }
    }

    if command.artifacts.emit_ir {
        write_artifact(&command.output, "ir", &ir_program.render())?;
    }

    let assembled = compile_program(target, &typed_program, &ir_program, &mut diagnostics);
    if diagnostics.has_errors() {
        return Err(diagnostics);
    }
    let assembled = assembled.expect("backend result checked");
    optimization_report.backend = assembled.optimization;

    if command.artifacts.emit_asm {
        write_artifact(&command.output, "asm", &assembled.program.render())?;
    }
```

Esto es didácticamente valiosísimo porque convierte ideas abstractas en secuencia verificable.

No dice “el compilador más o menos hace esto”.

Dice:

1. preprocesa
2. tokeniza
3. parsea
4. tipa y valida
5. baja a IR
6. optimiza IR
7. genera asm
8. codifica a palabras máquina
9. serializa a Intel HEX

## Qué aprende un lector serio de este fragmento

Primera lección:

```text
pipeline bueno = etapas pequeñas + chequeos de error entre etapas
```

Segunda lección:

```text
artefactos de depuración no son extras cosméticos;
son ventanas de observación sobre estados internos distintos
```

Tercera lección:

```text
optimización ocurre sobre IR, no sobre texto fuente ni sobre HEX final
```

## El detalle que mucha gente subestima: mismo `DiagnosticBag` durante todo el viaje

El archivo no crea un sistema de errores distinto por etapa.

Va pasando la misma bolsa de diagnósticos:

```rust
    let mut diagnostics = DiagnosticBag::new(command.warning_profile);
```

y luego:

```rust
    let preprocessed = preprocessor.process(main_source, &mut diagnostics);
    let mut lexer = Lexer::new(&preprocessed, &mut diagnostics);
    let mut parser = Parser::new(tokens, &preprocessed, &mut diagnostics);
    let typed_program = semantic.analyze(ast, &mut diagnostics);
    let mut ir_program = IrLowerer::new(target).lower(&typed_program, &mut diagnostics);
    let assembled = compile_program(target, &typed_program, &ir_program, &mut diagnostics);
```

Esto enseña una idea de ingeniería muy útil:

- error reporting no es capa externa
- forma parte del contrato entre etapas
- cada fase puede enriquecer el mismo informe acumulado

## Cómo se escriben artefactos auxiliares de forma uniforme

Otra pieza pequeña pero pedagógicamente importante:

```rust
fn write_artifact(output: &Path, extension: &str, contents: &str) -> StageResult<()> {
    let path = change_extension(output, extension);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            DiagnosticBag::single(
                Severity::Error,
                "io",
                format!("failed to create artifact directory `{}`: {error}", parent.display()),
            )
        })?;
    }

    fs::write(&path, contents).map_err(|error| {
        DiagnosticBag::single(
            Severity::Error,
            "io",
            format!("failed to write artifact `{}`: {error}", path.display()),
        )
    })?;
    Ok(())
}
```

Qué enseña esto:

- el compilador produce producto principal
- pero también produce evidencia de compilación
- y esa evidencia tiene API uniforme

Eso vuelve más fácil:

- depurar
- enseñar
- probar
- comparar `-O0` contra `-O2`

## Caso mental completo: `PORTB = 1;`

Tomemos ejemplo mínimo:

```c
PORTB = 1;
```

Al leer `src/lib.rs`, ya puedes obligarte a pasar por cada capa sin saltos:

1. preprocesador: quizá no cambia nada, pero fija origen y contexto
2. lexer: produce identificador `PORTB`, símbolo `=`, número `1`, `;`
3. parser: reconoce una asignación
4. semántica: resuelve `PORTB` como símbolo de registro de dispositivo, lo trata como lvalue `volatile unsigned char`
5. IR: expresa “copiar constante y luego almacenar en símbolo”
6. backend: emite `movlw 0x01` seguido de store al SFR correcto
7. encoder: transforma asm interno en palabras de 14 bits
8. HEX: empaqueta palabras con checksum y EOF

Ese ejercicio mental, repetido muchas veces, convierte lectura pasiva en comprensión real.

## Punto de entrada visible para usuario: `src/main.rs`

Aunque `src/lib.rs` contiene tubería real, el ejecutable empieza incluso antes:

```rust
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let options = match pic16cc::cli::CliOptions::parse(args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };

    if let Err(error) = pic16cc::execute(options) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
```

Es pequeño. Precisamente por eso es buen diseño.

Lección:

- `main` no conoce semántica
- `main` no conoce PIC16
- `main` no conoce IR
- `main` sólo parsea CLI y delega

## Regla de lectura recomendada para este archivo

Si enseñas este compilador a otra persona, pídele que haga esto:

1. lea `execute`
2. lea `compile_command`
3. anote cada transición de representación
4. para cada transición, abra archivo de implementación correspondiente

Esa disciplina evita error muy común:

```text
leer funciones sueltas fuera de contexto y perder sentido global
```

---

# 38. Apéndice I: lectura guiada del frontend real

## Preprocesador real: no sólo expande texto, también preserva origen

La estructura central aparece muy pronto:

```rust
pub struct Preprocessor<'a> {
    target: &'a TargetDevice,
    include_dirs: Vec<PathBuf>,
    macros: BTreeMap<String, MacroDef>,
    source_manager: &'a mut SourceManager,
    include_guard: HashSet<PathBuf>,
}
```

Esto ya dice mucho.

No es un simple `String -> String`.

Tiene:

- target
- macros predefinidas
- rutas de include
- gestor de fuentes
- protección contra inclusiones recursivas

La rutina de recorrido muestra bien filosofía de diseño:

```rust
    fn process_file(
        &mut self,
        source_id: SourceId,
        output: &mut PreprocessedSource,
        conditions: &mut Vec<ConditionFrame>,
        diagnostics: &mut DiagnosticBag,
    ) {
        let path = self.source_manager.path(source_id).to_path_buf();
        if !self.include_guard.insert(path.clone()) {
            return;
        }

        let file = self.source_manager.file(source_id).clone();
        for (line_index, line) in file.text.lines().enumerate() {
            let point = SourcePoint {
                file: source_id,
                line: line_index + 1,
                column: 1,
            };
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') {
                self.handle_directive(
                    &path,
                    source_id,
                    line_index + 1,
                    trimmed,
                    output,
                    conditions,
                    diagnostics,
                );
                continue;
            }

            if is_active(conditions) {
                let expanded = self.expand_line(line, point, diagnostics);
                output.push_str(&expanded, point);
                output.push_char('\n', point);
            }
        }

        self.include_guard.remove(&path);
    }
```

Qué conviene observar aquí:

1. el preprocesador recorre línea a línea
2. detecta directivas sólo cuando la línea empieza con `#`
3. conserva `SourcePoint`
4. decide si una línea está activa según pila de condicionales

Eso último importa mucho para diagnósticos buenos.

Un compilador educativo pobre habría hecho sólo expansión textual y habría perdido trazabilidad.

Éste no.

## Lexer real: pequeño, directo, suficiente

El tipo de token es deliberadamente simple:

```rust
pub enum TokenKind {
    Identifier(String),
    Number(i64),
    Keyword(Keyword),
    Symbol(Symbol),
    Eof,
}
```

Y la parte más instructiva del lexer está en `next_token`:

```rust
        if ch.is_ascii_alphabetic() || ch == '_' {
            self.index += 1;
            while self.index < bytes.len() {
                let current = bytes[self.index] as char;
                if current.is_ascii_alphanumeric() || current == '_' {
                    self.index += 1;
                } else {
                    break;
                }
            }
            let text = &self.source.text[start..self.index];
            return Token {
                kind: keyword_or_ident(text),
                span: Span::new(start, self.index),
            };
        }

        if ch.is_ascii_digit() {
            self.index += 1;
            if ch == '0'
                && self.index < bytes.len()
                && matches!(bytes[self.index] as char, 'x' | 'X')
            {
                self.index += 1;
                while self.index < bytes.len() && (bytes[self.index] as char).is_ascii_hexdigit() {
                    self.index += 1;
                }
            } else {
                while self.index < bytes.len() && (bytes[self.index] as char).is_ascii_digit() {
                    self.index += 1;
                }
            }
```

Esto enseña una verdad importante:

```text
lexer útil no necesita ser “ingenioso”;
necesita ser claro, determinista y fácil de depurar
```

Además, la tabla de símbolos dobles deja ver prioridad correcta:

```rust
    fn try_double_symbol(&mut self) -> Option<Symbol> {
        let rest = &self.source.text[self.index..];
        let table = [
            ("==", Symbol::EqualEqual),
            ("!=", Symbol::BangEqual),
            ("<=", Symbol::LessEqual),
            ("<<", Symbol::LessLess),
            (">=", Symbol::GreaterEqual),
            (">>", Symbol::GreaterGreater),
            ("&&", Symbol::AndAnd),
            ("||", Symbol::OrOr),
        ];
```

Siempre hay que reconocer primero operadores largos, o romperás tokens válidos en piezas incorrectas.

## Parser real: decide “función o global” mirando forma, no adivinando intención

La decisión central de `parse_item` merece leerse despacio:

```rust
    fn parse_item(&mut self) -> Item {
        let start = self.current_span().start;
        let decl = self.parse_decl_specifiers();
        let (name, name_span, ty) = self.parse_declarator(decl.ty);

        if self.match_symbol(Symbol::LParen) {
            let params = self.parse_params();
            let span = Span::new(start, self.previous_span().end);
            if self.match_symbol(Symbol::LBrace) {
                let body = self.parse_block_after_open(span.start);
                return Item::Function(FunctionDecl {
                    name,
                    return_type: ty,
                    storage_class: decl.storage_class,
                    is_interrupt: decl.is_interrupt,
                    params,
                    body: Some(body),
                    span: Span::new(start, self.previous_span().end),
                });
            }
            self.expect_symbol(Symbol::Semicolon);
            Item::Function(FunctionDecl {
                name,
                return_type: ty,
                storage_class: decl.storage_class,
                is_interrupt: decl.is_interrupt,
                params,
                body: None,
                span: Span::new(start, self.previous_span().end),
            })
        } else {
            if decl.is_interrupt {
                self.diagnostics.error(
                    "parser",
                    Some(Span::new(start, name_span.end)),
                    "`__interrupt` is only valid on function declarations",
                    Some("declare the interrupt handler as `void __interrupt isr(void)`".to_string()),
                );
            }
```

Aquí se ve muy bien qué significa parsear:

- reunir especificadores
- parsear declarador
- mirar siguiente forma sintáctica
- clasificar top-level item

No está “entendiendo” todavía si una función es legal para ABI o ISR.

Sólo está construyendo estructura.

Ésa es la separación sana entre parser y semántica.

## Semántica real: el compilador deja de ver texto y empieza a ver programa

El núcleo del archivo `src/frontend/semantic.rs` está en sus tipos intermedios:

```rust
pub enum TypedExprKind {
    IntLiteral(i64),
    Symbol(SymbolId),
    Unary {
        op: UnaryOp,
        expr: Box<TypedExpr>,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<TypedExpr>,
        rhs: Box<TypedExpr>,
    },
    ArrayDecay(Box<TypedExpr>),
    AddressOf(Box<TypedExpr>),
    Deref(Box<TypedExpr>),
    Assign {
        target: Box<TypedExpr>,
        value: Box<TypedExpr>,
    },
    Call {
        function: SymbolId,
        args: Vec<TypedExpr>,
    },
    Cast {
        kind: CastKind,
        expr: Box<TypedExpr>,
    },
}
```

Esto es una lección muy potente para quien quiera construir compiladores.

El AST fuente todavía dice:

```text
“el usuario escribió esto”
```

La semántica ya dice:

```text
“esto significa exactamente esto”
```

Por ejemplo:

- `Call` ya no guarda nombre textual, guarda `SymbolId`
- `Cast` ya no es implícito, queda explícito
- `ArrayDecay` deja visible regla C que muchas veces pasa desapercibida
- `AddressOf` y `Deref` ya son nodos distintos y tipados

## Por qué sembrar SFR como símbolos al principio

Uno de los fragmentos más iluminadores del archivo es éste:

```rust
    fn seed_device_registers(&mut self) {
        for register in self.target.sfrs {
            let symbol = self.insert_symbol(Symbol {
                id: self.symbols.len(),
                name: register.name.to_string(),
                ty: Type::new(ScalarType::U8).with_qualifiers(Qualifiers {
                    is_const: false,
                    is_volatile: true,
                }),
                storage_class: StorageClass::Extern,
                is_interrupt: false,
                kind: SymbolKind::DeviceRegister,
                span: Span::new(0, 0),
                fixed_address: Some(register.address),
                is_defined: true,
                is_referenced: false,
                parameter_types: Vec::new(),
            });
            self.globals_by_name.insert(register.name.to_string(), symbol);
        }
    }
```

Esto merece pausa.

No modela `PORTB`, `TRISB` o `INTCON` como magia del backend.

Los modela como símbolos semánticos reales:

- tienen nombre
- tienen tipo
- son `volatile`
- tienen dirección fija

Resultado:

- el frontend puede resolverlos como nombres normales
- el backend puede tratarlos como almacenamiento absoluto
- el diagnóstico puede hablar de ellos como entidades del programa

Éste es un ejemplo excelente de cómo un target específico puede entrar pronto en la cadena sin romper modularidad.

## Casts y truncaciones: semántica útil, no sólo aceptadora

Otro fragmento importante es `coerce_expr`.

Parte del corazón está aquí:

```rust
        if warn_on_truncate
            && expr.ty.bit_width() > target_ty.bit_width()
            && !is_representable_integer_constant(&expr, target_ty)
        {
            diagnostics.warning(
                "semantic",
                Some(expr.span),
                format!("conversion from `{}` to `{}` truncates", expr.ty, target_ty),
                "W1001",
            );
        }
```

Qué enseña esto:

- semántica no es sólo decir sí o no
- también puede explicar riesgo
- puede distinguir entre literal representable y truncación real

Ejemplo pequeño:

```c
unsigned char x = 300;
```

Aquí no basta con “compila” o “no compila”.

Hay una política concreta de warning, y el compilador la implementa con lógica explícita.

## ISR: regla semántica visible y verificable

La validación del manejador de interrupción está escrita con claridad admirable:

```rust
    fn validate_interrupt_signature(
        &mut self,
        function: &FunctionDecl,
        diagnostics: &mut DiagnosticBag,
    ) {
        if !function.is_interrupt {
            return;
        }

        if !function.return_type.is_void() {
            diagnostics.error(
                "semantic",
                Some(function.span),
                format!(
                    "interrupt handler `{}` must return `void` in phase 6",
                    function.name
                ),
                Some("declare it as `void __interrupt isr(void)`".to_string()),
            );
        }

        if !function.params.is_empty() {
            diagnostics.error(
                "semantic",
                Some(function.span),
                format!(
                    "interrupt handler `{}` cannot take parameters in phase 6",
                    function.name
                ),
                Some("remove the parameters and use `void`".to_string()),
            );
        }
```

Esto es muy didáctico porque vuelve política de lenguaje en código legible.

No hace falta leer documentación externa para saber subset soportado.

Está ahí, en forma ejecutable.

Ejemplo correcto del repositorio:

```c
void __interrupt isr(void) {
    if ((INTCON & 0x04) != 0) {
        tick_count = tick_count + 1;
        PORTB = PORTB ^ 0x01;
        INTCON = INTCON & 0xFB;
    }
}
```

Ejemplo incorrecto:

```c
unsigned int __interrupt isr(unsigned char reason) {
    return reason;
}
```

Ese segundo ejemplo viola dos reglas a la vez:

- devuelve valor
- recibe parámetros

## Recursión prohibida: no por capricho, por modelo de stack

Una de las mejores secciones del archivo es rechazo de ciclos de llamada:

```rust
    fn reject_recursive_calls(&self, diagnostics: &mut DiagnosticBag) {
        let mut state = BTreeMap::<SymbolId, VisitState>::new();
        for function in &self.functions {
            self.visit_call_graph(function.symbol, &mut state, &mut Vec::new(), diagnostics);
        }
    }
```

y dentro de DFS:

```rust
                if matches!(state.get(&callee).copied(), Some(VisitState::Active)) {
                    let cycle = stack
                        .iter()
                        .map(|id| self.symbols[*id].name.as_str())
                        .chain(std::iter::once(self.symbols[callee].name.as_str()))
                        .collect::<Vec<_>>()
                        .join(" -> ");
                    diagnostics.error(
                        "semantic",
                        Some(self.symbols[callee].span),
                        format!("recursive call cycle `{cycle}` is not supported in phase 4"),
                        Some("phase 4 computes software-stack usage statically; keep the call graph acyclic".to_string()),
                    );
                    continue;
                }
```

Esto es oro pedagógico.

Muestra una idea muy madura:

```text
regla del lenguaje no nace de teoría abstracta;
nace de decisión de implementación del ABI y del análisis de stack
```

Ejemplo prohibido:

```c
unsigned int fact(unsigned int n) {
    if (n <= 1) {
        return 1;
    }
    return n * fact(n - 1);
}
```

Si el compilador quiere calcular profundidad máxima de stack de forma estática y simple, esta forma de recursión rompe esa suposición.

## Punteros a locales de stack: otra restricción con motivación real

También merece atención esta comprobación:

```rust
                            if self.returns_stack_local_address(&expr) {
                                diagnostics.error(
                                    "semantic",
                                    Some(expr.span),
                                    "returning the address of a stack local is not supported",
                                    Some(
                                        "return a global/static object address or write through an output parameter"
                                            .to_string(),
                                    ),
                                );
                            }
```

Conceptualmente es lo correcto aunque el C “normal” también lo considera error lógico clásico.

En este proyecto, además, está ligado a cómo se materializan frames en Phase 4.

Ejemplo que debe rechazarse:

```c
unsigned char *bad(void) {
    unsigned char local = 3;
    return &local;
}
```

## Qué estudiar primero dentro de `semantic.rs`

Orden recomendado:

1. tipos de datos públicos: `TypedProgram`, `Symbol`, `TypedStmt`, `TypedExpr`
2. `analyze`
3. `seed_device_registers`
4. `define_function`
5. `coerce_expr`
6. `validate_interrupt_signature`
7. `reject_recursive_calls`
8. restricciones de punteros de stack

Si se sigue ese orden, el archivo deja de parecer enorme y empieza a parecer muy coherente.

---

# 39. Apéndice J: IR y lowering reales, leídos desde código

## La forma real de la IR

El archivo `src/ir/model.rs` contiene definición más importante para entender mitad central del compilador:

```rust
pub enum IrInstr {
    Copy {
        dst: TempId,
        src: Operand,
    },
    AddrOf {
        dst: TempId,
        symbol: SymbolId,
    },
    Cast {
        dst: TempId,
        kind: CastKind,
        src: Operand,
    },
    Unary {
        dst: TempId,
        op: UnaryOp,
        src: Operand,
    },
    Binary {
        dst: TempId,
        op: BinaryOp,
        lhs: Operand,
        rhs: Operand,
    },
    Store {
        target: SymbolId,
        value: Operand,
    },
    LoadIndirect {
        dst: TempId,
        ptr: Operand,
    },
    StoreIndirect {
        ptr: Operand,
        value: Operand,
        ty: Type,
    },
    Call {
        dst: Option<TempId>,
        function: SymbolId,
        args: Vec<Operand>,
    },
}
```

Y el control flow explícito:

```rust
pub enum IrTerminator {
    Return(Option<Operand>),
    Jump(BlockId),
    Branch {
        condition: IrCondition,
        then_block: BlockId,
        else_block: BlockId,
    },
    Unreachable,
}
```

Esto ya deja claras varias cosas:

- la IR ya no es árbol
- la IR ya vive en bloques básicos
- llamadas, indirectos y stores ya son operaciones explícitas
- el control flow se representa con terminadores separados

## Por qué esta IR es buena para enseñar

Porque tiene suficiente estructura para:

- optimizar
- modelar llamadas
- modelar saltos
- modelar indirección

pero no tanta complejidad como para ocultar intención.

No hay SSA completa.

No hay phi nodes.

No hay alias analysis complicada.

Eso la hace ideal para aprendizaje serio.

## Lowering real de `if`

La traducción de un `if` se ve con nitidez en `src/ir/lowering.rs`:

```rust
            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let then_block = self.new_block("if.then");
                let else_block = self.new_block("if.else");
                let join_block = self.new_block("if.end");
                let else_target = if else_branch.is_some() { else_block } else { join_block };
                self.lower_condition(condition, then_block, else_target);

                self.current = then_block;
                self.lower_stmt(then_branch);
                self.ensure_jump(join_block);

                if let Some(else_branch) = else_branch {
                    self.current = else_block;
                    self.lower_stmt(else_branch);
                    self.ensure_jump(join_block);
                }

                self.current = join_block;
            }
```

Éste es uno de los fragmentos que más conviene estudiar si alguien viene de ASTs puros.

Muestra transición mental esencial:

```text
estructura sintáctica → bloques + aristas + join explícito
```

## Lowering real de `while`

También es muy claro el caso del bucle:

```rust
            TypedStmt::While {
                condition, body, ..
            } => {
                let header = self.new_block("while.head");
                let body_block = self.new_block("while.body");
                let end = self.new_block("while.end");
                self.ensure_jump(header);

                self.current = header;
                self.loop_stack.push((header, end));
                self.lower_condition(condition, body_block, end);

                self.current = body_block;
                self.lower_stmt(body);
                self.ensure_jump(header);
                self.loop_stack.pop();

                self.current = end;
            }
```

Qué ve aquí un constructor de compiladores:

1. bloque cabecera
2. evaluación de condición
3. salto a cuerpo o salida
4. salto de vuelta
5. pila de destinos para `break` y `continue`

## Caso guiado con código real: `sum_bytes`

El ejemplo de `examples/pic16f628a/stack_abi.c` es excelente porque mezcla bucle, punteros y acumulación:

```c
unsigned int sum_bytes(unsigned char *ptr, unsigned int len, unsigned int bias) {
    unsigned int i = 0;
    unsigned int acc = bias;

    while (i < len) {
        acc = acc + ptr[i];
        i = i + 1;
    }

    return acc;
}
```

Haz ejercicio mental completo:

1. `i` y `acc` ya son símbolos locales tipados
2. `ptr[i]` ya no se entiende como sintaxis bonita, sino como aritmética de direcciones + carga indirecta
3. `while (i < len)` ya no es nodo “while”, sino cabecera + branch
4. `return acc` ya es terminador `Return(Some(...))`

Este ejemplo enseña más que muchos ejemplos artificiales porque es pequeño pero fuerza al compilador a usar:

- parámetros
- locals
- array decay
- indexado
- loop control
- retorno de 16 bits

## Render textual de IR: herramienta didáctica subestimada

`IrProgram::render()` existe para depuración, pero también para aprender:

```rust
        for function in &self.functions {
            let _ = writeln!(
                output,
                "{}fn #{} entry=b{}",
                if function.is_interrupt { "interrupt " } else { "" },
                function.symbol,
                function.entry
            );
            for block in &function.blocks {
                let _ = writeln!(output, "  b{} ({}):", block.id, block.name);
                for instruction in &block.instructions {
                    let _ = writeln!(output, "    {}", render_instr(instruction));
                }
                let _ = writeln!(output, "    {}", render_terminator(&block.terminator));
            }
        }
```

Esto convierte representación interna en objeto legible.

Para enseñanza, eso es enorme.

Permite comparar:

- fuente C
- AST
- IR textual
- asm

sin necesidad de depurador especial.

## Optimización real: propagación y folding de constantes

El pase `constant_fold` no es teórico. Está muy concreto:

```rust
                    IrInstr::Binary { dst, op, lhs, rhs } => {
                        let original = (lhs, rhs);
                        let lhs = resolve_operand(lhs, &constants);
                        let rhs = resolve_operand(rhs, &constants);
                        if (lhs, rhs) != original {
                            stats.operands_propagated += usize::from(lhs != original.0)
                                + usize::from(rhs != original.1);
                        }
                        *instr = IrInstr::Binary { dst, op, lhs, rhs };
                        if let (Operand::Constant(lhs), Operand::Constant(rhs)) = (lhs, rhs) {
                            let ty = function.temp_types[dst];
                            let result = eval_binary(op, lhs, rhs, ty, ty);
                            *instr = IrInstr::Copy {
                                dst,
                                src: Operand::Constant(result),
                            };
                            constants.insert(dst, result);
                            stats.expressions_folded += 1;
                        }
                    }
```

Qué enseña esto:

- primero se propagan operandos conocidos
- luego se reescribe instrucción
- luego, si ambos lados ya son constantes, se colapsa a `Copy`

No hay magia.

Hay reescritura explícita y contabilidad explícita.

## Optimización real: simplificación de ramas

Más abajo, mismo pase hace algo muy instructivo:

```rust
        for block in &mut function.blocks {
            if let IrTerminator::Branch {
                condition,
                then_block,
                else_block,
            } = &block.terminator
                && let Some(value) = eval_condition(condition)
            {
                block.terminator = IrTerminator::Jump(if value { *then_block } else { *else_block });
                stats.branches_simplified += 1;
            }
        }
```

Esto es importante porque demuestra que optimización útil no empieza por register allocation heroica.

Empieza por cosas como:

- si la condición ya se conoce
- no dejes rama condicional
- conviértela en salto simple

## Optimización real: borrado de bloques inalcanzables

Y `dead_code_elimination` completa la lección:

```rust
            if !reachable.contains(&block.id) {
                stats.unreachable_blocks_cleared += usize::from(!block.instructions.is_empty());
                stats.instructions_removed += block.instructions.len();
                block.instructions.clear();
                block.terminator = IrTerminator::Unreachable;
                continue;
            }
```

Este fragmento enseña una idea central:

```text
una optimización no sólo cambia instrucciones;
también puede cambiar topología efectiva del CFG
```

## Qué mirar cuando abras un `.ir`

Método recomendado:

1. busca nombre de función
2. identifica bloque de entrada
3. anota cuántos bloques hay
4. sigue ramas y joins
5. pregunta qué temporales existen sólo por necesidad de orden
6. observa qué comparaciones ya desaparecieron en `-O2`

Preguntas útiles:

- ¿qué partes siguen siendo simbólicas?
- ¿qué partes ya son completamente explícitas?
- ¿dónde aparece primera vez un `call`?
- ¿dónde deja de existir el concepto “while” como tal?

---

# 40. Apéndice K: backend real, stack real, helpers reales

## La primera gran verdad del backend: antes de emitir asm, asigna memoria

La función `compile_program` deja clara la secuencia:

```rust
pub fn compile_program(
    target: &TargetDevice,
    typed_program: &TypedProgram,
    ir_program: &IrProgram,
    diagnostics: &mut DiagnosticBag,
) -> Option<BackendOutput> {
    let layout = StorageAllocator::new(target.allocatable_gpr, target.shared_gpr)
        .layout(typed_program, ir_program, diagnostics)?;

    let mut codegen = CodegenContext::new(target, typed_program, &layout);
    codegen.emit_program(ir_program, diagnostics);
```

Lección:

```text
backend bueno no “escupe instrucciones” sin antes decidir dónde vive cada cosa
```

## Layout real de helpers y frame base

La reserva de estado ABI aparece muy explícita:

```rust
        let Some(stack_ptr_lo) = allocator.next_span(2) else {
            diagnostics.error("backend", None, "not enough RAM for ABI helper slots", None);
            return None;
        };
        let Some(frame_ptr_lo) = allocator.next_span(2) else {
            diagnostics.error("backend", None, "not enough RAM for ABI helper slots", None);
            return None;
        };
        let Some(return_high) = allocator.next_span(1) else {
            diagnostics.error("backend", None, "not enough RAM for ABI helper slots", None);
            return None;
        };
```

Aquí el backend ya admite una verdad incómoda pero real:

- ABI cuesta RAM
- helpers cuestan RAM
- return values anchos cuestan RAM

Después construye estructura reusable:

```rust
        let helpers = HelperRegisters {
            stack_ptr: RegisterPair {
                lo: stack_ptr_lo,
                hi: stack_ptr_lo + 1,
            },
            frame_ptr: RegisterPair {
                lo: frame_ptr_lo,
                hi: frame_ptr_lo + 1,
            },
            return_high,
            scratch0,
            scratch1,
        };
```

## Layout real del frame por función

Otra parte excelente para estudio:

```rust
            let mut arg_bytes = 0u16;
            for param in &function.params {
                symbol_storage.insert(*param, SymbolStorage::Frame(arg_bytes));
                arg_bytes += self.symbol_width(typed_program, *param)?;
            }

            let saved_fp_offset = arg_bytes;
            let mut local_cursor = arg_bytes + 2;
            for local in &function.locals {
                let symbol = &typed_program.symbols[*local];
                if symbol.kind != SymbolKind::Local || symbol.storage_class == crate::frontend::types::StorageClass::Static {
                    continue;
                }
                symbol_storage.insert(*local, SymbolStorage::Frame(local_cursor));
                local_cursor += self.symbol_width(typed_program, *local)?;
            }
```

Este fragmento vale más que muchas descripciones verbales.

Dice exactamente:

1. argumentos al principio del frame
2. luego `saved FP`
3. luego locals
4. luego temps IR

Y eso coincide con capítulos teóricos anteriores.

Éste es buen momento para comprobar que libro y código realmente se corresponden.

## El vector de interrupción real

La lógica de arranque y vectorización está escrita de forma muy visible:

```rust
    fn emit_vectors(&mut self) {
        let interrupt = self
            .typed_program
            .symbols
            .iter()
            .find(|symbol| symbol.kind == SymbolKind::Function && symbol.is_interrupt)
            .map(|symbol| function_label(&symbol.name));
        self.program.push(AsmLine::Org(self.target.vectors.reset));
        self.program.push(AsmLine::Label("__reset_vector".to_string()));
        self.program
            .push(AsmLine::Instr(AsmInstr::Goto("__reset_dispatch".to_string())));
        self.program.push(AsmLine::Org(self.target.vectors.interrupt));
        self.program.push(AsmLine::Label("__interrupt_vector".to_string()));
        if interrupt.is_some() {
            self.program
                .push(AsmLine::Instr(AsmInstr::Goto("__interrupt_dispatch".to_string())));
        } else {
            self.program.push(AsmLine::Instr(AsmInstr::Retfie));
        }
```

Qué conviene entender:

- aunque usuario no escriba vector, backend sí
- si no hay ISR, deja `retfie` seguro
- si hay ISR, emite salto a dispatcher

Este diseño es muy limpio porque separa:

- semántica de “existe ISR”
- mecánica de “cómo se materializa vector en target”

## Comentarios asm como documentación viva del ABI

Una idea especialmente didáctica del backend es que emite comentarios útiles:

```rust
        self.program.push(AsmLine::Comment(format!(
            "frame args={} saved_fp={} locals={} temps={} frame_bytes={}",
            arg_bytes, saved_fp_offset, local_bytes, temp_bytes, frame_bytes
        )));
```

y también:

```rust
        self.program.push(AsmLine::Comment(format!(
            "stack base=0x{:04X} end=0x{:04X} capacity={} max_depth={}",
            self.layout.stack_base,
            self.layout.stack_end,
            self.layout.stack_capacity,
            self.layout.max_stack_depth
        )));
```

Esto convierte asm emitido en documento semiautodescriptivo.

Para aprendizaje, eso es magnífico.

## Lowering real de llamada de función

La rutina `emit_call` enseña muy bien cómo Phase 4 aterriza en máquina:

```rust
        let callee_symbol = &self.typed_program.symbols[callee];
        for (index, arg) in args.iter().enumerate() {
            let Some(param_ty) = callee_symbol.parameter_types.get(index).copied() else {
                diagnostics.error(
                    "backend",
                    None,
                    "call passes more arguments than callee signature",
                    None,
                );
                continue;
            };
            self.push_operand(function.symbol, *arg, param_ty);
        }

        let label = function_label(self.symbol_name(callee));
        self.program.push(AsmLine::Instr(AsmInstr::SetPage(label.clone())));
        self.program.push(AsmLine::Instr(AsmInstr::Call(label)));

        let arg_bytes = self.function_arg_bytes(callee);
        if arg_bytes != 0 {
            self.add_immediate_to_pair(self.layout.helpers.stack_ptr, negate_u16(arg_bytes));
        }
```

Es decir:

1. empuja argumentos ya tipados
2. prepara página
3. hace `call`
4. ajusta `SP`

Ésa es la ABI en código.

No en documento aparte.

## Helpers runtime reales: catálogo tipado

El archivo `src/backend/pic16/midrange14/runtime.rs` no es mero listado; es especificación ejecutable:

```rust
            Self::MulU16 => RuntimeHelperInfo {
                label: "__rt_mul_u16",
                operand_ty: Type::new(ScalarType::U16),
                arg_bytes: 4,
                local_bytes: 4,
                frame_bytes: 6,
            },
            Self::DivU16 => RuntimeHelperInfo {
                label: "__rt_div_u16",
                operand_ty: Type::new(ScalarType::U16),
                arg_bytes: 4,
                local_bytes: 4,
                frame_bytes: 6,
            },
            Self::Shl16 => RuntimeHelperInfo {
                label: "__rt_shl16",
                operand_ty: Type::new(ScalarType::U16),
                arg_bytes: 4,
                local_bytes: 0,
                frame_bytes: 2,
            },
```

Qué se aprende aquí:

- helper no es sólo nombre
- helper tiene contrato de operandos
- helper tiene huella de frame

Eso permite que backend razone sobre coste y sobre profundidad máxima de stack.

## Optimización backend concreta: evitar helpers cuando divisor es potencia de dos

Uno de los detalles más instructivos del backend es que no acepta siempre llamada a helper como destino final.

Mantiene optimizaciones concretas. La base matemática visible está aquí:

```rust
fn normalized_power_of_two_shift(value: Option<i64>, ty: Type) -> Option<usize> {
    let value = normalize_value(value?, ty) as u64;
    if value == 0 || !value.is_power_of_two() {
        return None;
    }
    Some(value.trailing_zeros() as usize)
}
```

Idea:

- si `x / 8`
- y `8` es potencia de dos
- quizá no hace falta helper de división
- quizá basta shift correcto

Ésta es optimización pequeña, segura y muy rentable en microcontrolador pequeño.

## Caso práctico real 1: `expression_test.c`

Código fuente:

```c
unsigned int expression_test(unsigned int a, unsigned int b, unsigned int c) {
    return (a * b) + (c / 3) - (a % 5);
}
```

Qué conviene esperar:

- multiplicación de 16 bits probablemente helper
- división por `3` probablemente helper
- módulo por `5` probablemente helper
- suma/resta alrededor quizá inline

Este ejemplo es magnífico para estudiar límites entre:

- aritmética que PIC16 puede resolver localmente
- aritmética que conviene delegar a runtime

## Caso práctico real 2: `timer_interrupt.c`

Código fuente:

```c
void __interrupt isr(void) {
    if ((INTCON & 0x04) != 0) {
        tick_count = tick_count + 1;
        PORTB = PORTB ^ 0x01;
        INTCON = INTCON & 0xFB;
    }
}
```

Qué debe pasar por debajo:

1. vector de interrupción salta a dispatcher
2. backend guarda contexto mínimo necesario
3. ejecuta cuerpo inline-safe
4. restaura contexto
5. emite `retfie`

Y, muy importante, el frontend ya se ocupó antes de impedir operaciones que forzarían helper o llamada normal.

## Pregunta clave para estudiar backend

Cuando abras `codegen.rs`, no preguntes primero “¿cómo se genera cada instrucción?”.

Pregunta primero:

```text
¿qué invariantes ya garantiza frontend + IR para simplificar backend?
```

Esa pregunta cambia por completo calidad de lectura.

---

# 41. Apéndice L: laboratorio reproducible de estudio

## Objetivo

Este apéndice no explica teoría nueva.

Explica cómo estudiar el compilador de forma experimental, repetible y útil.

## Comando base recomendado

Si quieres seguir caso por caso y obtener artefactos:

```bash
cargo run -- --target pic16f628a -I include -O2 --emit-ast --emit-ir --emit-asm --map --list-file -o /tmp/blink.hex examples/pic16f628a/blink.c
```

Variantes frecuentes:

```bash
cargo run -- --target pic16f628a -I include -O2 --emit-ir --emit-asm -o /tmp/stack.hex examples/pic16f628a/stack_abi.c
```

```bash
cargo run -- --target pic16f877a -I include -O2 --emit-ir --emit-asm --opt-report -o /tmp/expr.hex examples/pic16f877a/expression_test.c
```

Artefactos a inspeccionar junto al `.hex`:

- `.ast`
- `.ir`
- `.asm`
- `.map`
- `.lst`

## Ruta 1: `blink.c`

Archivo:

```text
examples/pic16f628a/blink.c
```

Qué estudiar aquí:

- inicialización de SFR
- stores directos
- control flow mínimo
- diferencia entre `TRISB` y `PORTB`

Preguntas guía:

1. ¿cuándo se reconoce `PORTB` como símbolo preexistente?
2. ¿cuándo deja de ser nombre y pasa a ser dirección fija?
3. ¿qué parte del pipeline decide si hay banking?

## Ruta 2: `stack_abi.c`

Archivo:

```text
examples/pic16f628a/stack_abi.c
```

Qué estudiar aquí:

- varios argumentos
- locals en frame
- array local
- llamada anidada
- retorno de 16 bits

Fragmento clave:

```c
value = sum4(1, 2, build_local(3, 1, 2), 4);
```

Este ejemplo es excelente porque obliga a pensar en:

- orden de evaluación
- push de argumentos
- frame temporal de `build_local`
- posterior llamada a `sum4`

Si entiendes bien este archivo, entiendes de verdad por qué existe Phase 4.

## Ruta 3: `pointer16.c`

Archivo:

```text
examples/pic16f877a/pointer16.c
```

Fragmentos clave:

```c
void store_word(unsigned int *ptr, unsigned int value) {
    ptr[1] = value;
}
```

```c
volatile unsigned char *port = &PORTB;
```

```c
if (cursor == words) {
    *port = 0x5A;
}
```

Qué estudiar aquí:

- decay de arrays
- comparación de punteros soportada
- indirección
- escala por tamaño de elemento
- paso de punteros como parámetros

## Ruta 4: `expression_test.c`

Archivo:

```text
examples/pic16f877a/expression_test.c
```

Qué estudiar aquí:

- lowering de expresión compuesta
- aparición de helpers
- interacción entre optimización IR y backend
- relación entre `--opt-report` y asm resultante

Preguntas guía:

1. ¿cuántos temporales IR aparecen antes de asm?
2. ¿qué partes se quedan inline?
3. ¿qué helper labels aparecen en `.asm`?

## Ruta 5: `timer_interrupt.c`

Archivos:

```text
examples/pic16f628a/timer_interrupt.c
examples/pic16f877a/timer_interrupt.c
examples/pic16f877a/gpio_interrupt.c
```

Qué estudiar aquí:

- restricción de ISR
- vector de interrupción
- contexto compartido
- `retfie`

Pregunta guía crucial:

```text
¿qué parte del comportamiento ISR está garantizada por semántica
y qué parte la materializa backend?
```

## El propio test suite enseña cómo estudiar el compilador

`tests/compiler_pipeline.rs` no es sólo verificación automática.

También es catálogo de casos pedagógicos.

Mira cómo compila fixtures con artefactos:

```rust
fn compile_input(target: &str, input: PathBuf) -> PathBuf {
    let output = temp_file("out.hex");
    let options = CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: target.to_string(),
            input,
            output: output.clone(),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O2,
            artifacts: OutputArtifacts {
                emit_ast: true,
                emit_ir: true,
                emit_asm: true,
                map: true,
                list_file: true,
                ..OutputArtifacts::default()
            },
            verbose: false,
            opt_report: false,
            warning_profile: WarningProfile {
                wall: true,
                wextra: true,
                werror: false,
            },
        }),
    };
    execute(options).expect("compile example");
    output
}
```

Este helper de test, leído con calma, ya te dice cuál es la manera canónica de observar compilación completa.

## Test específico que explica ABI mejor que muchas páginas

Por ejemplo:

```rust
fn assert_phase4_stack_metadata(output: &Path) {
    let asm = read_artifact(output, "asm");
    let map = read_artifact(output, "map");
    let listing = read_artifact(output, "lst");

    assert!(asm.contains("frame args="));
    assert!(asm.contains("stack base="));
    assert!(map.contains("__abi.stack_ptr.lo"));
    assert!(map.contains("__abi.frame_ptr.lo"));
    assert!(map.contains("__stack.base"));
    assert!(map.contains("__stack.end"));
    assert!(listing.contains("frame args="));
}
```

Esto es pedagógico por dos razones:

1. muestra qué huellas concretas deja Phase 4
2. enseña cómo verificar ABI sin leer todo el asm manualmente

## Nombres de tests que conviene leer como índice temático

Algunos especialmente útiles:

- `compiles_phase4_stack_abi_example`
- `compiles_phase4_nested_call_regression_fixture`
- `compiles_phase5_helper_nested_expression_fixture`
- `phase7_avoids_helper_for_unsigned_power_of_two_division`
- `reports_interrupt_function_calls`
- `reports_unsupported_recursion`
- `reports_returning_stack_local_address`

Sólo leyendo esos nombres ya puedes reconstruir mapa conceptual del proyecto.

## Método de estudio de 90 minutos

Sesión recomendada:

1. compila `blink.c` con artefactos
2. compila `stack_abi.c` con artefactos
3. compila `expression_test.c` con `--opt-report`
4. compila `timer_interrupt.c` con artefactos
5. compara `.ir` y `.asm` entre los cuatro

Resultado:

- en poco tiempo recorres casi todo diseño del compilador
- sin perderte en miles de líneas seguidas

---

# 42. Apéndice M: errores reales y prohibiciones conscientes

## Por qué este apéndice importa

Muchos lectores entienden un compilador sólo por lo que acepta.

Eso es insuficiente.

En un compilador serio también hay que entender:

- qué rechaza
- por qué lo rechaza
- en qué fase lo rechaza

## Caso 1: inicializadores de arrays no implementados en Phase 3

Regla visible en semántica:

```rust
                    diagnostics.error(
                        "semantic",
                        Some(decl.span),
                        "array initializers are not implemented in phase 3",
                        Some("declare the array without an initializer and fill it in code".to_string()),
                    );
```

Ejemplo rechazado:

```c
unsigned char table[3] = {1, 2, 3};
```

Por qué probablemente está prohibido:

- obliga a modelar inicialización compuesta
- afecta globals y locals
- complica lowering y layout

Lección:

```text
limitar subset temprano puede ser decisión correcta, no carencia vergonzosa
```

## Caso 2: recursión

Nombre de test útil:

```text
reports_unsupported_recursion
```

Ejemplo:

```c
unsigned int f(unsigned int x) {
    if (x == 0) {
        return 0;
    }
    return f(x - 1);
}
```

Por qué se rechaza aquí:

- stack de software con profundidad estática
- análisis de profundidad necesita grafo acíclico

No es “porque PIC16 no pueda llamar funciones”.

Es porque este diseño concreto quiere mantener cálculo estático de stack simple y seguro.

## Caso 3: devolver dirección de local

Nombre de test útil:

```text
reports_returning_stack_local_address
```

Ejemplo:

```c
unsigned int *bad(void) {
    unsigned int local = 7;
    return &local;
}
```

Razón profunda:

- el frame muere al salir
- puntero quedaría colgando
- en este compilador, además, la restricción se expresa explícitamente para mantener modelo sano

## Caso 4: llamadas dentro de ISR

Nombre de test útil:

```text
reports_interrupt_function_calls
```

Ejemplo:

```c
void blink_once(void) {
    PORTB = PORTB ^ 0x01;
}

void __interrupt isr(void) {
    blink_once();
}
```

Por qué se rechaza:

- ISR de Phase 6 es deliberadamente conservadora
- no debe depender de ABI normal
- no debe invocar helpers ni llamadas ordinarias

Aquí se ve coordinación entre semántica y backend:

- semántica prohíbe forma fuente
- backend ya puede asumir cuerpo ISR inline-safe

## Caso 5: narrowing problemático

Nombres de tests útiles:

```text
rejects_out_of_range_constant_to_unsigned_char
rejects_non_constant_int_to_unsigned_char_under_werror
```

Ejemplos:

```c
unsigned char a = 260;
```

```c
unsigned int big = 300;
unsigned char b = big;
```

Qué enseña este caso:

- no todo narrowing es igual
- literal representable no es mismo caso que valor no constante potencialmente truncado
- warnings también forman parte del diseño del compilador

## Caso 6: firma ISR inválida

Nombres de tests útiles:

```text
reports_interrupt_return_type_mismatch
reports_interrupt_parameter_mismatch
reports_multiple_interrupt_handlers
```

Ejemplos:

```c
unsigned int __interrupt isr(void) {
    return 0;
}
```

```c
void __interrupt timer_isr(unsigned char reason) {
}
```

```c
void __interrupt isr0(void) {
}

void __interrupt isr1(void) {
}
```

Todo esto enseña misma lección:

```text
subset bien definido = reglas visibles + diagnósticos precisos + tests que fijan contrato
```

## Lo importante no es sólo rechazo, sino momento del rechazo

Pregunta formativa excelente:

```text
¿esta restricción se detecta mejor en parser, semántica, IR o backend?
```

Ejemplos:

- `__interrupt` sobre variable: parser o semántica temprana
- firma ISR: semántica
- helper no permitido en ISR: semántica
- falta de RAM para stack: backend
- write final fallido: I/O final

Ésta es una de las mejores formas de aprender diseño por capas.

---

# 43. Apéndice N: autoevaluación seria para comprobar comprensión real

## Cómo usar este apéndice

No lo leas como trivialidades de memoria.

Úsalo así:

1. intenta responder sin mirar código
2. luego verifica en archivo real
3. si tu respuesta no apunta a archivo y motivo, aún no dominas concepto

## Pregunta 1

¿Por qué `PORTB` puede analizarse como símbolo normal y no como caso especial del parser?

Pista:

- mira `seed_device_registers`
- piensa en `fixed_address` + `volatile`

## Pregunta 2

¿En qué momento exacto `while` deja de existir como construcción de alto nivel?

Pista:

- no ocurre en lexer
- no ocurre en parser
- observa `lower_stmt`

## Pregunta 3

¿Por qué Phase 4 necesita rechazar recursión aunque PIC16 sí tenga pila hardware para retornos?

Pista:

- distingue pila hardware de return addresses
- distingue stack de software para locals y args

## Pregunta 4

¿Qué gana el proyecto al emitir `.ir` textual además de `.asm`?

Pista:

- piensa en depuración de decisiones antes de backend
- piensa en optimización

## Pregunta 5

¿Por qué `ArrayDecay` merece nodo semántico explícito?

Pista:

- en C parece implícito
- en compilador pequeño, hacerlo explícito simplifica mucha lógica posterior

## Pregunta 6

¿Qué diferencia conceptual hay entre `SymbolStorage::Absolute` y `SymbolStorage::Frame`?

Pista:

- uno describe dirección fija global o SFR
- otro describe offset relativo a frame activo

## Pregunta 7

¿Por qué un helper runtime tiene `arg_bytes`, `local_bytes` y `frame_bytes` como metadata?

Pista:

- no basta nombre de rutina
- backend necesita coste estructural

## Pregunta 8

¿Qué dos capas cooperan para que una ISR termine con `retfie` correcto?

Pista:

- semántica restringe cuerpo
- backend emite prólogo, epílogo y vector

## Pregunta 9

¿Qué tipo de optimización es más fácil justificar primero en proyecto como éste: folding de constantes o register allocation global?

Pista:

- piensa en coste de implementación
- piensa en riesgo de romper corrección

## Pregunta 10

Si ves en `.asm` comentario `frame args=8 saved_fp=8 locals=4 temps=6 frame_bytes=12`, ¿qué te está diciendo realmente?

Pista:

- no lo leas como decoración
- léelo como radiografía del ABI de esa función

## Pregunta 11

¿Por qué `tests/compiler_pipeline.rs` es casi un manual del proyecto?

Pista:

- nombres de tests
- helpers de compilación
- asserts sobre artefactos

## Pregunta 12

Si tuvieras que ampliar este compilador con una feature nueva, ¿qué tres sitios mirarías primero?

Pista de respuesta razonable:

1. AST o parser, si cambia sintaxis
2. semántica, si cambia significado o restricciones
3. IR/backend, si cambia cómo se ejecuta

## Señal de comprensión real

Has entendido bien el proyecto cuando puedes hacer esto sin improvisar:

```text
tomar una línea C concreta,
nombrar qué estructura produce cada fase,
decir qué archivo la transforma,
y justificar por qué esa fase es la correcta para hacerlo.
```

---

# 44. Epílogo pedagógico

Si has llegado hasta aquí, ya puedes mirar este repositorio de otra forma.

Ya no es sólo “código Rust que genera código PIC16”.

Es un ejemplo muy claro de varias ideas profundas:

- los compiladores no son una única transformación
- cada capa existe para resolver un problema concreto
- el hardware manda mucho más de lo que parece
- un buen ABI puede cambiar por completo la robustez del sistema
- una IR bien diseñada simplifica tanto el backend como la optimización
- las restricciones bien documentadas son una fortaleza, no una debilidad

Y quizá la enseñanza más importante de todas es ésta:

```text
construir un compilador no consiste en acumular features;
consiste en tomar decisiones arquitectónicas coherentes
y hacerlas visibles, comprobables y mantenibles.
```

Ésa es, precisamente, una de las mejores lecciones que se pueden extraer de `pic16cc`.
