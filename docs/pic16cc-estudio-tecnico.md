# Estudio Técnico de `pic16cc` / `picc`

Documento de estudio en español para comprender el compilador `pic16cc` en profundidad, como si se preparara una defensa técnica, una entrevista de arquitectura o una revisión de diseño.

Estado del proyecto analizado:

- crate: `pic16cc`
- binario CLI: `picc`
- fase activa: Phase 6
- targets soportados: `PIC16F628A` y `PIC16F877A`
- backend compartido: `src/backend/pic16/midrange14`

Este documento no intenta vender el proyecto; intenta explicarlo con rigor. Cuando una decisión simplifica el problema a costa de recortar C estándar, se dice explícitamente. Cuando una restricción nace del hardware PIC16 y no de una preferencia estética, también se explica.

---

# 1. Project Overview

## Qué es `pic16cc`

`pic16cc` es un compilador experimental escrito en Rust para una familia muy concreta de microcontroladores: los PIC16 mid-range clásicos de 14 bits. Su objetivo no es compilar C genérico para cualquier CPU, sino convertir un subconjunto deliberadamente acotado de C en código real para:

- `PIC16F628A`
- `PIC16F877A`

El ejecutable que usa el usuario final es `picc`. La ruta de uso actual es la de un compilador real, no la de una demo interna:

```bash
picc --target pic16f877a -Wall -Wextra -Werror -O2 -I include -o build/main.hex src/main.c
```

El resultado principal es un `.hex` programable, acompañado opcionalmente por:

- `.map`
- `.lst`
- `.asm`
- `.ir`
- `.ast`
- `.tokens`

## Por qué existe

El proyecto existe para construir una cadena nativa y entendible de extremo a extremo para una arquitectura que normalmente se resuelve con toolchains externos, históricos o muy opacos. Hay una motivación técnica clara:

- estudiar cómo se implementa un compilador real para una ISA incómoda
- validar decisiones de ABI, memoria, lowering y helpers sobre hardware muy restringido
- disponer de un backend compartido por familia y no de un generador ad hoc por ejemplo
- aprender qué partes de C son baratas y cuáles son carísimas en una MCU de 8 bits con RAM bancarizada

## Qué problema resuelve

El problema que resuelve no es “hacer C completo”, sino “hacer un subconjunto útil de C que baje de verdad a PIC16 sin fingir”. Eso implica:

- análisis semántico con tipos y diagnósticos reales
- una IR propia entre frontend y backend
- lowering explícito para arrays, punteros, llamadas, comparaciones, helpers aritméticos e ISR
- ensamblado a palabras de 14 bits
- emisión Intel HEX con config word

## Arquitectura objetivo: PIC16 mid-range de 14 bits

Los PIC16 clásicos son una mala diana si uno quiere que el compilador sea fácil. Precisamente por eso son interesantes.

Características críticas:

- arquitectura Harvard: memoria de programa y de datos separadas
- palabra de instrucción de 14 bits
- ALU orientada al registro `W`
- RAM bancarizada
- saltos y llamadas paginados con `PCLATH`
- pila hardware de retorno, pero no pila de datos de propósito general
- direccionamiento indirecto limitado vía `FSR/INDF`

## Por qué escribir un compilador para PIC16 no es trivial

En una arquitectura moderna, llamar a una función, reservar locales o hacer `a * b` se apoya en una pila hardware rica, registros generales y, a menudo, instrucciones dedicadas. En PIC16 clásico casi nada de eso existe.

Los problemas duros reales son:

- no hay registros generales abundantes; casi todo gira alrededor de `W`
- el acceso directo a RAM requiere gestionar banco con `STATUS.RP0/RP1`
- el acceso indirecto requiere `FSR`, `INDF` y `STATUS.IRP`
- `CALL` y `GOTO` necesitan preparar `PCLATH` si el destino cae en otra página
- la pila hardware sirve para direcciones de retorno, no para argumentos o variables automáticas
- las operaciones de 16 bits son siempre sintetizadas byte a byte
- multiplicación, división, módulo y shifts dinámicos requieren lowering no trivial o helpers

La consecuencia conceptual es importante: el compilador no puede limitarse a “escupir unas pocas instrucciones”. Necesita un modelo interno muy claro de memoria, frame, ABI, temporales, banking y paging.

---

# 2. Global Architecture

## Pipeline completo

El punto de entrada del usuario es `src/main.rs`, que parsea la CLI y delega en `pic16cc::execute` en `src/lib.rs`. Desde ahí se encadena todo el pipeline:

```text
input.c
  |
  v
SourceManager
  |
  v
Preprocessor
  |
  v
PreprocessedSource + source-origin mapping
  |
  v
Lexer
  |
  v
Tokens
  |
  v
Parser
  |
  v
AST
  |
  v
SemanticAnalyzer
  |
  v
TypedProgram
  |
  v
IrLowerer
  |
  v
IrProgram
  |
  +--> constant_fold
  +--> dead_code_elimination
  |
  v
PIC16 midrange14 codegen
  |
  v
AsmProgram
  |
  v
Encoder (14-bit words)
  |
  +--> render_listing(.lst)
  +--> render_map(.map)
  |
  v
IntelHexWriter
  |
  v
.hex
```

## Flujo de datos entre etapas

### 2.1 Preprocessing

`src/frontend/preprocessor.rs` procesa:

- `#include`
- `#define` de macros objeto
- `#undef`
- `#if`
- `#ifdef`
- `#ifndef`
- `#else`
- `#endif`

Punto fino importante: el preprocesador no sólo produce texto; produce un `PreprocessedSource` que guarda, carácter por carácter, el origen del texto emitido. Eso permite que los diagnósticos posteriores se mapeen de vuelta al archivo y línea originales incluso después de expandir includes y macros simples.

### 2.2 Lexing

`src/frontend/lexer.rs` transforma el texto preprocesado en tokens. El lexer reconoce:

- identificadores
- literales numéricos decimales y hexadecimales
- keywords del subconjunto soportado
- símbolos simples y dobles como `==`, `!=`, `<<`, `>>`, `&&`, `||`

También reconoce `__interrupt` como keyword específica de la fase de ISR.

### 2.3 Parsing

`src/frontend/parser.rs` construye un AST explícito:

- `TranslationUnit`
- `Item`
- `FunctionDecl`
- `VarDecl`
- `Stmt`
- `Expr`

El parser usa una jerarquía clásica de precedencias:

```text
assignment
  -> logical or
  -> logical and
  -> bitwise or
  -> bitwise xor
  -> bitwise and
  -> equality
  -> relational
  -> shift
  -> additive
  -> multiplicative
  -> unary
  -> postfix
  -> primary
```

Eso importa porque la semántica posterior depende de tener ya separadas expresiones como:

- `a + b * c`
- `a << n`
- `*ptr = value`
- `foo(x, y, z)`

### 2.4 Semantic analysis

`src/frontend/semantic.rs` es el sitio donde el compilador deja de tratar el programa como “texto estructurado” y empieza a tratarlo como programa tipado.

La semántica hace varias cosas a la vez:

- resuelve nombres
- crea y mantiene tablas de símbolos
- inyecta registros SFR del dispositivo como símbolos predefinidos
- clasifica lvalues y rvalues
- valida tipos soportados
- inserta casts explícitos (`ZeroExtend`, `SignExtend`, `Truncate`, `Bitcast`)
- diagnostica conversiones peligrosas
- valida firmas de función
- valida reglas de ISR
- detecta recursión
- detecta escapes de punteros a locales de stack

El resultado es un `TypedProgram`, no un AST “casi tipado”. Esa frontera es fuerte y útil: el backend no necesita volver a mirar el AST.

### 2.5 IR generation y lowering

`src/ir/model.rs` define una IR propia basada en CFG con:

- bloques básicos
- instrucciones explícitas
- terminadores
- operandos simbólicos o temporales
- tipos de temporales
- metadata `is_interrupt` por función

La lowering desde `TypedProgram` a `IrProgram` ocurre en `src/ir/lowering.rs`.

Idea central: la IR no intenta esconder memoria ni llamadas complejas. Al revés, las hace explícitas:

- `AddrOf`
- `LoadIndirect`
- `StoreIndirect`
- `Call`
- `Cast`
- `Binary`
- `Unary`

Eso es clave porque PIC16 castiga mucho la ambigüedad entre acceso directo e indirecto.

### 2.6 IR optimization

`src/ir/passes.rs` aplica dos pases simples:

- `constant_fold`
- `dead_code_elimination`

No hay una fase de optimización agresiva tipo SSA global, LICM o register allocation avanzado. La estrategia del proyecto es primero obtener lowering correcto sobre PIC16 y luego optimizar sólo donde sea barato y seguro.

### 2.7 Backend PIC16 específico

`src/backend/pic16/midrange14/codegen.rs` toma la IR tipada y genera `AsmProgram`.

El backend conoce:

- banking de RAM
- paging de código
- layout de globals
- helpers ABI (`stack_ptr`, `frame_ptr`, `return_high`, `scratch0`, `scratch1`)
- frame layout por función
- lowering de comparaciones 8/16 bits
- acceso indirecto con `FSR/INDF`
- helpers aritméticos de Phase 5
- vector de interrupción y prólogo/epílogo ISR de Phase 6

### 2.8 Assembly encoding

La salida del backend todavía no es binaria; es un ensamblado estructurado (`AsmProgram`) con:

- `Org`
- `Label`
- `Instr`
- `Comment`

Luego `src/backend/pic16/midrange14/encoder.rs`:

- resuelve labels
- expande pseudo-operaciones como `SetPage(label)`
- codifica instrucciones a palabras PIC16 de 14 bits

### 2.9 Intel HEX generation

`src/hex/intel_hex.rs` convierte el mapa de palabras a bytes Intel HEX.

Hay una sutileza importante:

- la memoria de programa PIC16 está indexada por palabra
- Intel HEX trabaja en direcciones de byte

Por eso cada palabra se emite como dos bytes, y además el config word se inyecta en `0x2007` (expresado en bytes como `0x400E`).

## Invariantes globales del diseño

Éstas son las invariantes que conviene memorizar porque explican casi todo el proyecto:

- el frontend no sabe nada de opcodes PIC16
- el backend no vuelve a mirar el AST
- cada símbolo SFR viene del descriptor de dispositivo, no de constantes hardcodeadas dispersas
- todos los valores de 16 bits son little-endian
- los punteros son punteros de data space, no de code space
- el ABI activo es stack-first; `arg0/arg1` sólo es historia documental
- los temporales IR relevantes para una llamada viven en el frame, no en RAM global estática
- la pila software crece hacia arriba
- la profundidad máxima de stack se calcula estáticamente; por eso no se permite recursión

---

# 3. Phase-by-phase Implementation

La cronología del proyecto es acumulativa. Cada fase no reemplaza el compilador; añade una capacidad nueva y obliga a endurecer algún punto del diseño.

## v0.1

### Goals

Establecer una tubería end-to-end: de un `.c` a un `.hex`.

### Qué se implementó

La documentación histórica de v0.1 no sobrevive como fase separada, así que esta sección es una reconstrucción razonable a partir de la arquitectura actual y de lo que Phase 2 ya da por supuesto. La base mínima que debía existir era:

- CLI
- carga de fuentes
- preprocesado simple
- lexer
- parser
- semántica básica
- backend PIC16 inicial
- encoder
- Intel HEX

También es razonable inferir que ya existía un subconjunto operativo con:

- variables escalares simples
- funciones
- `if`, `while`, `return`
- acceso a SFR por nombre
- ejemplo estilo blink

### Qué cambió en el compilador

Se fijó la separación más importante del proyecto:

- frontend por un lado
- backend por otro
- formato intermedio propio

Aunque la IR todavía no tuviera toda la riqueza de fases posteriores, la idea de no mezclar parsing con codegen ya estaba tomada.

### Key design decisions

- no usar LLVM ni un backend genérico
- apostar por un backend compartido específico de familia `midrange14`
- usar descriptores de dispositivo para que `PIC16F628A` y `PIC16F877A` compartan lowering
- producir `.hex` real desde el inicio

### Trade-offs

- subconjunto C muy pequeño
- sin 16 bits completos
- sin arrays/punteros reales
- sin pila software
- sin ambición de C estándar completo

### Limitations introduced

- capacidad de llamada muy limitada
- expresiones complejas difíciles
- poca abstracción sobre memoria

### Examples

Ejemplo representativo del espíritu v0.1:

```c
#include <pic16/pic16f628a.h>

void main(void) {
    TRISB = 0x00;
    PORTB = 0x01;
}
```

Lo importante aquí no es la complejidad semántica; es que ya existe una cadena que entiende SFRs y produce un `.hex`.

## Phase 2

### Goals

Hacer reales los enteros de 16 bits y sus comparaciones.

### Qué se implementó

- soporte efectivo para `int` y `unsigned int`
- casts explícitos en semántica e IR
- comparaciones firmadas y sin signo
- normalización booleana a `0`/`1`
- lowering 16-bit byte a byte

### Qué cambió en el compilador

Frontend:

- inferencia más precisa de literales enteros
- rechazo de algunos casos de signedness ambiguo

IR:

- `Cast`
- `IrCondition::NonZero`
- `IrCondition::Compare`

Backend:

- suma y resta de 16 bits con propagación de carry/borrow
- comparaciones firmadas y unsigned
- convención de retorno de 16 bits: `W` + `return_high`

### Key design decisions

La decisión clave de Phase 2 fue no inventar instrucciones especiales en la IR para “compare signed 16-bit”, sino añadir tipado a las condiciones. Eso deja el conocimiento de PIC16 en el backend y el conocimiento de tipos en la IR.

### Trade-offs

- ABI aún fijo por slots
- máximo dos argumentos
- sin pila software
- temporales todavía fuera de cualquier modelo de frame serio

### Limitations introduced

- el salto de complejidad se pagó con un ABI aún muy rígido
- 16 bits existe, pero llamar funciones complejas sigue siendo difícil

### Examples

```c
unsigned int add16(unsigned int a, unsigned int b) {
    return a + b;
}

if (lhs >= rhs) {
    PORTB = 0x55;
}
```

Conceptualmente, Phase 2 demuestra que el backend ya sabe pensar en dos bytes coordinados y no sólo en bytes aislados.

## Phase 3

### Goals

Introducir un modelo de memoria visible: arrays, punteros, address-of, dereference e indexing.

### Qué se implementó

- arrays unidimensionales fijos
- punteros de datos de 16 bits
- `&obj`
- `*ptr`
- `a[i]`
- `p[i]`
- `sizeof` de tipos soportados
- `LoadIndirect` y `StoreIndirect`

### Qué cambió en el compilador

Frontend:

- distinción formal lvalue/rvalue
- decay explícito de arrays
- compatibilidad limitada de punteros

IR:

- `AddrOf`
- `LoadIndirect`
- `StoreIndirect`

Backend:

- programación de `FSR`
- derivación de `STATUS.IRP`
- acceso byte a byte vía `INDF`

### Key design decisions

La decisión más importante fue hacer las operaciones de memoria explícitas en IR. En lugar de dejar que el backend adivine si una expresión es “un load indirecto disfrazado”, la IR lo dice.

### Trade-offs

- punteros sólo a data memory
- nada de function pointers
- nada de pointer-to-pointer
- nada de multidimensional arrays
- punteros relacionales limitados a `==` y `!=`

### Limitations introduced

El modelo de memoria mejora mucho, pero todavía no existe stack software real. Un array local de esta fase todavía no está apoyado sobre el frame per-call que aparecerá después.

### Examples

```c
unsigned char sum(unsigned char *ptr, unsigned int len) {
    unsigned int i = 0;
    unsigned char acc = 0;
    while (i < len) {
        acc = acc + ptr[i];
        i = i + 1;
    }
    return acc;
}
```

Este tipo de código obliga al compilador a:

- decaer arrays a puntero
- escalar índices
- programar `FSR`
- leer o escribir con `INDF`

## Phase 4

### Goals

Sustituir el ABI rígido de slots por un ABI stack-first coherente y real.

### Qué se implementó

- pila software en RAM
- `stack_ptr` y `frame_ptr`
- argumentos por stack
- limpieza de argumentos por el caller
- frame por llamada
- locales y arrays locales sobre stack
- temporales IR por frame
- soporte robusto para 3 o más argumentos
- nested calls no recursivas

### Qué cambió en el compilador

Frontend/semántica:

- rechazo de recursión por profundidad de stack estática
- mejor rechazo de punteros que escapan a locales de stack

IR:

- `Call { args }` arbitrario
- confianza en que los temps sobreviven a nested calls porque ya no son globales

Backend:

- `StorageAllocator` reserva helpers ABI y luego calcula frames
- acceso frame-relative con `FP + offset`
- prólogo/epílogo por función
- análisis estático de profundidad máxima de stack

### Key design decisions

La decisión más importante de todo el proyecto probablemente es ésta:

- caller empuja argumentos
- callee guarda caller `FP`
- callee establece `FP` sobre el área de argumentos
- callee reserva locales y temporales
- caller limpia los argumentos al volver

Esto evita que caller y callee limpien la misma zona y hace el álgebra del stack manejable.

### Trade-offs

- la pila crece hacia arriba, lo cual es poco habitual si vienes de x86/ARM pero perfectamente válido
- no hay detección dinámica de overflow de stack
- la profundidad máxima se calcula estáticamente y por tanto la recursión queda prohibida

### Limitations introduced

- si el programa necesita stack dinámico real, el diseño actual no basta
- el análisis de stack es correcto pero conservador

### Examples

```c
unsigned int sum4(unsigned int a, unsigned int b,
                  unsigned int c, unsigned int d) {
    return a + b + c + d;
}

unsigned int x = sum4(1, 2, 3, 4);
```

Y también:

```c
return a(b(c(1)));
```

Sin temporales per-call y sin un contrato ABI claro, estas formas se rompen enseguida.

## Phase 5

### Goals

Dar lowering real a:

- `*`
- `/`
- `%`
- `<<`
- `>>`

### Qué se implementó

- helpers runtime de multiplicación, división, módulo y shifts dinámicos
- constantes e identidades inline
- shifts constantes inline
- signed/unsigned separados por familia de helper
- documentación explícita del comportamiento de división por cero y signed shift

### Qué cambió en el compilador

Frontend:

- reglas semánticas para estos operadores
- diagnóstico de división/módulo por cero constante
- diagnóstico de shift count inválido
- advertencia `-Wextra` para right shift firmado

IR:

- no se añadió un “helper instruction”; se reutilizó `IrInstr::Binary`

Backend:

- selección entre inline fast path y runtime helper
- emisión perezosa de helpers sólo si se usan
- cálculo de profundidad de stack incluyendo llamadas a helpers del compilador

### Key design decisions

La decisión clave fue no contaminar la IR con helpers concretos de PIC16. La IR sigue diciendo “esto es un multiply” y el backend decide si lo emite inline o llama a `__rt_mul_u16`.

Eso mantiene la IR más limpia y concentra la dependencia de target en el backend.

### Trade-offs

- no hay multiplicador hardware, así que el coste en código y ciclos es real
- división dinámica por cero no hace trap; devuelve `0`
- el modelo de promociones sigue siendo un subconjunto simplificado de C

### Limitations introduced

- no hay lattice completa de usual arithmetic conversions
- no hay trap runtime sofisticado para errores aritméticos

### Examples

```c
unsigned int expression_test(unsigned int a,
                             unsigned int b,
                             unsigned int c) {
    return (a * b) + (c / 3) - (a % 5);
}
```

Esta expresión prueba algo muy importante: el ABI stack-first reparado soporta no sólo llamadas del usuario, sino llamadas internas generadas por el compilador a helpers runtime.

## Phase 6

### Goals

Añadir ISR real e integración con el vector de interrupción.

### Qué se implementó

- sintaxis `void __interrupt isr(void)`
- vector en `0x0004`
- dispatch ISR
- prólogo/epílogo ISR
- `retfie`
- save/restore conservador de contexto
- restricciones de seguridad sobre el cuerpo ISR

### Qué cambió en el compilador

Frontend:

- keyword `__interrupt`
- marca `is_interrupt` en AST y semántica
- validación de firma ISR

IR:

- `IrFunction.is_interrupt`

Backend:

- reset vector en `0x0000`
- interrupt vector en `0x0004`
- contexto ISR en shared GPR
- restore final de `W` con `swapf`

### Key design decisions

La gran decisión de Phase 6 fue adoptar una política conservadora:

- una sola ISR por programa
- sin llamadas normales dentro de ISR
- sin helpers Phase 5 dentro de ISR

Esto es Option A: más simple y más segura que permitir ISR reentrantes o helper-heavy.

### Trade-offs

- el subconjunto ISR es más pequeño que el subconjunto normal
- el compilador sacrifica expresividad para no corromper estado interrumpido

### Limitations introduced

- no hay llamadas desde ISR
- no hay varias ISR
- no hay un modelo de prioridades/interrupciones múltiples

### Examples

```c
void __interrupt isr(void) {
    if ((INTCON & 0x04) != 0) {
        PORTB = PORTB ^ 0x01;
        INTCON = INTCON & 0xFB;
    }
}
```

---

# 4. Memory Model Deep Dive

## PIC16 data memory

Los targets soportados se describen en `src/backend/pic16/devices.rs`. Cada `TargetDevice` declara:

- vectores
- GPR asignable
- shared GPR
- SFRs visibles
- capabilities

En el estado actual ambos devices usan:

- GPR asignable: `0x20..0x6F`
- shared GPR: `0x70..0x7F`

Esa shared GPR se usa sobre todo para contexto ISR.

## Banking

El acceso directo a RAM en PIC16 no usa la dirección completa directamente. La instrucción lleva sólo 7 bits de file register y el banco efectivo se selecciona con bits del `STATUS`.

Bits relevantes:

- `RP0` = bit 5
- `RP1` = bit 6

El backend hace esto en `select_bank(addr)`:

```text
bank = (addr >> 7) & 0x03
RP0 = bank bit 0
RP1 = bank bit 1
```

Consecuencia: el compilador no puede emitir un simple `movwf addr`. Primero tiene que preparar banco correcto y luego usar `low7(addr)`.

## STATUS bits críticos

Bits del `STATUS` que el compilador usa activamente:

- `C` bit 0: carry/borrow para suma, resta y comparaciones unsigned
- `Z` bit 2: resultado cero
- `RP0` bit 5: banking directo
- `RP1` bit 6: banking directo
- `IRP` bit 7: banking indirecto

Esto vuelve delicadas tres cosas:

- comparaciones 16-bit
- direccionamiento indirecto
- prólogos/epílogos ISR

## FSR/INDF e indirect addressing

Cuando el compilador quiere acceder a un puntero o a una celda del frame:

1. calcula una dirección efectiva
2. programa `FSR` con el byte bajo
3. deriva `IRP` del byte alto
4. lee o escribe vía `INDF`

El patrón conceptual es:

```text
ptr.lo -> FSR
ptr.hi bit0 -> STATUS.IRP
INDF <-> byte apuntado
```

El proyecto transporta punteros como valores de 16 bits completos por uniformidad, aunque en estos dispositivos el backend explote sobre todo:

- low byte
- selector IRP

## Cómo modela memoria el compilador

Hay tres clases de almacenamiento:

```text
1. Absolute
   - globals
   - static locals
   - SFRs
   - helpers ABI

2. Frame(offset)
   - parámetros
   - autos
   - local arrays
   - temporales IR

3. Shared interrupt context
   - __isr_ctx.*
```

Eso está encapsulado en:

- `SymbolStorage::Absolute(u16)`
- `SymbolStorage::Frame(u16)`

## Cómo se colocan las variables

### Globals y static locals

Se asignan en RAM absoluta con `AddressAllocator`.

### Parámetros

No tienen dirección absoluta: viven en el frame del callee.

### Autos y arrays locales

Viven por offset respecto a `FP`.

### Temporales IR

Desde la reparación de Phase 4, también viven en el frame, no en RAM estática por función.

## Por qué esto importa

La misma expresión de C:

```c
ptr[i] = local + global;
```

puede tocar tres modelos de almacenamiento distintos:

- `ptr[i]`: acceso indirecto
- `local`: frame-relative
- `global`: absolute/direct

Un compilador correcto para PIC16 tiene que saber en cada byte de cada operando de dónde sale el valor y qué bits de control debe programar para llegar a él.

---

# 5. Stack-first ABI (Phase 4)

## Por qué hacía falta un ABI

Antes del ABI stack-first, el compilador operaba con un modelo tipo `arg0/arg1` heredado de la fase histórica temprana. Eso tiene dos problemas estructurales:

- escala mal a más de dos argumentos
- complica nested calls porque los argumentos “globales” se pisan entre sí

En cuanto quieres que compile correctamente esto:

```c
return a(b(c(1)));
```

ya no basta con slots globales.

## Modelo anterior vs nuevo modelo

### Modelo antiguo

- argumentos en slots fijos
- sin pila software de datos real
- temporales más frágiles
- llamadas complejas mal escalables

### Modelo nuevo

- todos los argumentos por stack software
- caller-pushed
- caller-cleanup
- frame por invocación
- temporales por invocación

## Layout de stack

La pila crece hacia arriba:

```text
push byte:
  [SP] = value
  SP = SP + 1
```

En startup:

```text
SP = stack_base
FP = stack_base
```

## Layout de frame

Si una función tiene `A` bytes de argumentos:

```text
FP + 0 .. A-1       -> argumentos
FP + A              -> saved caller FP low
FP + A + 1          -> saved caller FP high
FP + A + 2 ..       -> locales
...                 -> arrays locales
...                 -> temporales IR
```

Diagrama resumido:

```text
                direccion creciente

FP --> [arg0...]
       [arg1...]
       [argN...]
       [saved FP lo]
       [saved FP hi]
       [local 0]
       [local 1]
       [local array...]
       [temp 0]
       [temp 1]
SP --> top del frame actual
```

## Contrato caller/callee

### Antes de llamar

```text
SP = caller_top
```

### Caller

1. evalúa argumentos
2. empuja bytes de argumentos en orden izquierda -> derecha
3. hace `CALL`
4. resta `arg_bytes` a `SP`
5. captura retorno desde `W` y, si hace falta, `return_high`

### Callee prologue

1. guarda `FP` anterior en scratch
2. calcula `FP = SP - arg_bytes`
3. empuja `saved FP`
4. avanza `SP` para reservar locales y temporales

### Callee epilogue

1. lee `saved FP` desde `FP + arg_bytes`
2. hace `SP = FP + arg_bytes`
3. restaura caller `FP`
4. ejecuta `RETURN`

## Orden de argumentos y retorno

Reglas:

- evaluación según orden actual del lowering
- bytes low-first dentro de cada escalar
- 8 bits: retorno en `W`
- 16 bits y punteros: low en `W`, high en `return_high`

## Nested calls

El motivo por el que el modelo soporta nested calls es doble:

- cada invocación tiene su propio frame
- los temporales necesarios para sobrevivir a una llamada interna están en ese frame

Ejemplo:

```c
return (a + b) + inc(c + d);
```

El compilador puede calcular `a + b`, guardarlo como temp frame-scoped, llamar a `inc`, recuperar el retorno y sumar ambos resultados sin usar RAM global compartida para los temps.

## Por qué es difícil en PIC16

En una CPU con SP hardware y addressing modes ricos, un ABI así sale “gratis”. En PIC16 hay que sintetizarlo:

- `SP` y `FP` son variables en RAM
- cada acceso `FP + offset` se convierte en preparar `FSR`
- cada push es un store indirecto seguido de incremento manual de `SP`
- no hay `push`, `pop`, `enter`, `leave`

Esa es una idea importante para explicar el proyecto: Phase 4 no añade una feature cosmética; añade una infraestructura de ejecución que el hardware no da.

---

# 6. Runtime Helpers (Phase 5)

## Por qué hacen falta helpers

PIC16 clásico no tiene:

- multiplicación hardware
- división hardware
- módulo hardware
- shifts de 16 bits de alto nivel

Si el compilador quisiera bajar todo inline siempre, el codegen crecería mucho y se volvería más difícil de razonar. La solución adoptada es mixta:

- inline cuando es barato y claro
- helper runtime cuando la operación es cara o dinámica

## Familias de helpers

El módulo `src/backend/pic16/midrange14/runtime.rs` clasifica helpers por operación, ancho y signedness:

- `__rt_mul_u8`
- `__rt_mul_i8`
- `__rt_mul_u16`
- `__rt_mul_i16`
- `__rt_div_u8`
- `__rt_div_i8`
- `__rt_div_u16`
- `__rt_div_i16`
- `__rt_mod_u8`
- `__rt_mod_i8`
- `__rt_mod_u16`
- `__rt_mod_i16`
- `__rt_shl8`
- `__rt_shl16`
- `__rt_shr_u8`
- `__rt_shr_i8`
- `__rt_shr_u16`
- `__rt_shr_i16`

## Multiplicación: algoritmo shift-add

La multiplicación unsigned usa el patrón clásico:

```text
result = 0
repeat bit_width times:
  if (multiplier & 1) result += multiplicand
  multiplicand <<= 1
  multiplier >>= 1
```

En la implementación real:

- el multiplicando vive en el propio slot de argumento
- el multiplicador también
- el resultado vive en un local del helper
- un contador recorre el ancho del tipo

### Signed multiply

La signed multiply:

1. detecta signo de ambos operandos
2. normaliza a valor absoluto si hace falta
3. corre el core unsigned
4. reaplica signo al resultado con negación a complemento a dos

## División y módulo: restoring division

La división unsigned usa un esquema de división restauradora:

```text
quotient = dividend
remainder = 0
repeat bit_width times:
  rotate_left(quotient)
  rotate_left(remainder)
  if remainder >= divisor:
    remainder -= divisor
    quotient bit0 = 1
```

En la implementación:

- `arg0` acaba conteniendo el cociente
- un local del helper contiene el resto
- módulo devuelve ese resto

### Signed divide / modulo

La signed division hace:

1. capturar signo del dividendo y divisor
2. normalizar a magnitudes unsigned
3. ejecutar core unsigned
4. restaurar signo del cociente o del resto

El resto firmado sigue el signo del dividendo, no del divisor. Esa distinción es relevante y está codificada con un flag separado.

## Shifts

### Shifts constantes

Se emiten inline en el caller cuando el count es constante y válido.

Ejemplos:

- `x << 0 -> x`
- `x >> 0 -> x`
- `x << 3` inline

### Shifts dinámicos

Van a helper:

```text
while (count != 0) {
  value = value << 1   // o >> 1
  count--;
}
```

Antes del bucle, el helper clampa `count` al ancho del operando para evitar bucles absurdos o semánticas indefinidas sin control.

## Interacción con el ABI stack-first

Éste es uno de los puntos más importantes de Phase 5:

- los helpers usan el mismo ABI que las funciones normales
- el compilador actúa como caller de sus propios helpers
- el mismo `SP/FP` y el mismo contrato de limpieza siguen valiendo

Eso convierte Phase 5 en una prueba de estrés del ABI de Phase 4: si los helpers corrompieran frames, el proyecto se desmoronaría.

## Comportamientos documentados

- multiplicación 8-bit devuelve sólo el byte bajo
- multiplicación 16-bit devuelve sólo los 16 bits bajos
- división/módulo por cero constante: error en compilación
- división/módulo por cero dinámico: devuelve `0`
- right shift unsigned: lógico
- right shift signed: aritmético

---

# 7. ISR Model (Phase 6)

## Vector de reset e interrupción

Layout actual:

```text
0x0000 -> __reset_vector
0x0004 -> __interrupt_vector
0x2007 -> config word
```

Comportamiento:

- reset vector: `goto __reset_dispatch`
- interrupt vector:
  - con ISR: `goto __interrupt_dispatch`
  - sin ISR: `retfie`

Después de los vectores, el backend emite stubs de dispatch que también resuelven el problema de `PCLATH`.

## Sintaxis ISR

La única sintaxis soportada es:

```c
void __interrupt isr(void) {
    ...
}
```

## Restricciones ISR

La semántica exige:

- retorno `void`
- sin parámetros
- una sola ISR por programa
- sin `return value`
- sin llamadas normales
- sin expresiones que requieran helpers runtime

Esto no es un capricho. Está alineado con la elección consciente de un modelo conservador.

## Save/restore de contexto

El ISR guarda:

- `W`
- `STATUS`
- `PCLATH`
- `FSR`
- `return_high`
- `scratch0`
- `scratch1`
- `stack_ptr`
- `frame_ptr`

Se guarda en shared GPR porque:

- al restaurar `STATUS` se alteran bits de banca
- `W` debe restaurarse al final sin destruir flags o direcciones
- `swapf` permite reconstruir `W` desde RAM de forma segura

## Secuencia ISR real

```text
interrupt vector
  -> dispatch
  -> save contexto CPU + ABI
  -> prologue normal de frame
  -> cuerpo ISR
  -> epilogue normal de frame
  -> restore contexto CPU + ABI
  -> retfie
```

## Por qué ISR es peligrosa

Una ISR puede interrumpir al programa en cualquier punto:

- en mitad de una llamada
- en mitad de una operación 16-bit
- con `PCLATH` apuntando a otra página
- con `FSR` apuntando a un frame o a un puntero

Por eso la política de save/restore es deliberadamente conservadora. El compilador asume que interrumpe en el peor momento posible.

## Por qué se restringen llamadas y helpers

Si una ISR pudiera llamar libremente a funciones normales o a helpers runtime, habría que resolver temas mucho más duros:

- reentrada del runtime
- reentrada indirecta del ABI
- helpers que usan scratch y frame en mitad de un contexto interrumpido
- coste extra de save/restore o convenciones separadas

El proyecto elige no abrir ese frente todavía.

---

# 8. Code Generation for PIC16

## ISA y representación de ensamblado

El backend no genera bytes directamente. Primero genera un `AsmProgram` estructurado con:

- `Org`
- `Label`
- `Instr`
- `Comment`

Las instrucciones se modelan en `AsmInstr`, por ejemplo:

- `Movlw`
- `Movwf`
- `Movf`
- `Addwf`
- `Subwf`
- `Rlf`
- `Rrf`
- `Swapf`
- `Bcf`
- `Bsf`
- `Btfsc`
- `Btfss`
- `Goto`
- `Call`
- `Return`
- `Retfie`
- `SetPage`

## Limitaciones de la ISA PIC16

Las limitaciones que más condicionan el codegen son:

- `W` como acumulador dominante
- operandos memoria/memoria inexistentes
- branch condicional basado en “skip next instruction”
- acceso directo de 7 bits + bits de banco externos
- `CALL/GOTO` con alcance paginado

Eso obliga a secuencias más largas que en arquitecturas con compare+jump ricos.

## Cómo mapea constructs de alto nivel

### Asignación escalar

```c
x = y;
```

Se traduce a:

- cargar `y` en `W`
- almacenar `W` en `x`

### Suma 16-bit

```c
a + b
```

Se traduce a:

- sumar bytes bajos
- guardar carry
- sumar bytes altos con carry explícito

### Comparación 16-bit unsigned

```c
if (lhs >= rhs)
```

Se traduce a:

- comparar high byte
- si difiere, decidir por `C`/`Z`
- si es igual, comparar low byte

### Control flow

Los `if`, `while` y `for` se convierten en CFG en IR y luego en labels + branches page-safe.

## Handling de 16-bit en una máquina de 8 bits

Éste es un mantra que conviene explicar bien en entrevista:

- el compilador nunca “piensa” que el PIC16 sea realmente de 16 bits
- un valor de 16 bits es un convenio software, no una capacidad nativa de la ALU

Por eso casi toda operación 16-bit se descompone en:

- low byte
- high byte
- flags intermedios

## Branching y paging

`SetPage(label)` es una pseudo-instrucción muy importante. El encoder la expande a cuatro palabras:

1. limpiar `PCLATH<3>`
2. limpiar `PCLATH<4>`
3. poner bit 3 si hace falta
4. poner bit 4 si hace falta

Luego se emite el `CALL` o `GOTO` real.

Esa decisión mantiene el codegen legible y concentra el problema de paging en un pseudo-op claro.

## Listing y map como herramientas de comprensión

La salida `.lst` no es adorno. Sirve para estudiar:

- direcciones reales
- palabras codificadas
- secuencia de ensamblado

La `.map` expone:

- símbolos de código
- globals
- helpers ABI
- stack base/end
- contexto ISR

Para defender el proyecto técnicamente, estas dos salidas son casi tan importantes como el `.hex`.

---

# 9. CLI and Build System

## Filosofía de la CLI

El binario de usuario es `picc`, definido en `Cargo.toml` como:

- crate: `pic16cc`
- bin: `picc`

`src/main.rs` hace muy poco: parsea la CLI y llama a `execute`.

## Opciones importantes

La CLI soporta un estilo “compiler-like”:

- `--target`
- `-o`
- `-I`
- `-D`
- `-Wall`
- `-Wextra`
- `-Werror`
- `-O0`
- `-O1`
- `-O2`
- `-Os`
- `--emit-tokens`
- `--emit-ast`
- `--emit-ir`
- `--emit-asm`
- `--map`
- `--list-file`
- `--list-targets`
- `--help`
- `--version`

## Cómo se genera el `.hex`

La salida principal no es intermedia ni de test. El flujo normal es:

```bash
picc --target pic16f628a -Wall -Wextra -Werror -O2 -I include -o build/blink.hex examples/pic16f628a/blink.c
```

Si el directorio de salida no existe, la CLI lo crea.

Los artefactos opcionales se colocan junto al `.hex` cambiando extensión:

- `build/blink.map`
- `build/blink.lst`
- `build/blink.asm`
- `build/blink.ir`
- `build/blink.ast`

## Makefiles

El proyecto ya incluye plantillas y Makefiles por ejemplo:

- `examples/pic16f628a/Makefile`
- `examples/pic16f877a/Makefile`
- `examples/Makefile.template`

La idea es que el usuario no tenga que usar `cargo run` para compilar firmware. `cargo` se usa para construir o instalar `picc`; luego el flujo normal es `picc ...`.

## Flujo típico de usuario

```text
1. cargo install --path .
2. picc --list-targets
3. picc --target ... -o build/foo.hex foo.c
4. make flash   (si el usuario configura FLASH_CMD)
```

## Punto conceptual importante

El CLI no es un wrapper superficial. Es la interfaz oficial del compilador. Toda la tubería real está accesible desde `picc`.

---

# 10. Diagnostics and Error Handling

## Filosofía de diagnósticos

El proyecto intenta evitar dos fallos clásicos de compiladores experimentales:

- aceptar código que luego se rompe silenciosamente
- rechazar demasiado sin explicar por qué

El tipo base es `DiagnosticBag`, que acumula:

- severidad
- stage
- mensaje
- span
- sugerencia opcional
- código opcional de warning

## Warnings vs errors

`WarningProfile` tiene tres flags:

- `wall`
- `wextra`
- `werror`

`-Werror` no es un modo aparte: promueve warnings a errors dentro del mismo `DiagnosticBag`.

## Emisión con contexto de fuente

Gracias al `PreprocessedSource` con mapping de orígenes, `DiagnosticEmitter` puede imprimir:

- archivo real
- línea
- columna
- línea de código
- caret
- ayuda sugerida

Eso es mejor que diagnosticar sólo sobre el texto ya preprocesado.

## Casos importantes del proyecto

### Narrowing conversions

Política actual:

- si una constante cabe exactamente, puede estrechar sin warning
- si no cabe, se diagnostica truncación
- una conversión no constante que puede truncar también se diagnostica
- con `-Werror`, eso se convierte en error

Ejemplos:

Aceptado:

```c
unsigned char x = 8;
PORTB = 0x01;
```

Rechazado bajo `-Werror`:

```c
unsigned char x = 300;
PORTB = 300;
```

### División y módulo por cero

- divisor constante cero: error semántico
- divisor dinámico cero: comportamiento documentado del helper, devuelve `0`

### Restricciones ISR

Se diagnostican explícitamente:

- retorno no `void`
- parámetros en ISR
- más de una ISR
- llamadas normales desde ISR
- expresiones que requerirían helpers runtime dentro de ISR

### Escape de dirección de local

Se rechaza:

```c
return &local;
```

y también cadenas alias simples:

```c
p = &local;
return p;
```

## Importancia de no esconder trade-offs

Un punto muy defendible del proyecto es que prefiere un error claro a una compilación “optimista” que generaría firmware incorrecto.

---

# 11. Testing Strategy

## Tipos de pruebas

El proyecto combina:

- tests unitarios dentro de módulos
- tests de integración en `tests/compiler_pipeline.rs`
- comprobaciones de artefactos `.asm`, `.map`, `.lst`, `.hex`
- regresiones por fase

## Qué se prueba

### Frontend y semántica

- parsing de `__interrupt`
- casts
- signedness
- rejects de tipos no soportados
- diagnósticos de shift/division/modulo

### IR

- lowering de condiciones
- constant folding
- DCE

### Backend

- encoding `retfie`
- `swapf`
- emisión de helpers
- shape del vector de interrupción
- metadata de stack y map

### Pipeline end-to-end

Se compilan ejemplos reales de cada fase:

- blink
- arith16
- compare16
- arrays
- pointers
- stack ABI
- runtime helpers
- ISR

## Golden tests implícitos

Aunque no haya una infraestructura llamada “golden snapshots” formal para todo, muchos tests ya hacen validación por forma:

- presencia de labels concretos
- `call __rt_mul_u16`
- `retfie`
- `__interrupt_vector`
- `__abi.stack_ptr.lo`

Eso funciona como golden testing ligero.

## Qué no se prueba

La limitación más importante:

- CI no ejecuta firmware en emulador ni en hardware real

Por tanto, la confianza proviene de:

- compilación correcta
- forma correcta del asm/listing/map/hex
- regresión estructural

No proviene de medir ejecución sobre silicio.

## Cómo defender esta estrategia

Es una estrategia razonable para una fase de estabilización porque:

- valida lowering y artefactos
- protege decisiones ABI
- evita regresiones de forma
- mantiene el coste de CI bajo

Pero no sustituye:

- pruebas en simulador
- pruebas en hardware
- validación temporal/ciclos

---

# 12. Limitations of the Compiler

Conviene enumerarlas sin maquillaje.

## Limitaciones del lenguaje

- no `struct`
- no `union`
- no `enum`
- no `float`
- no `switch`
- no function pointers de alto nivel
- no pointer-to-pointer
- no multidimensional arrays
- no array initializers

## Limitaciones del modelo de ejecución

- no recursión
- no detección runtime de overflow de stack
- promotions C incompletas
- punteros sólo a data memory
- sin code pointers

## Limitaciones ISR

- una sola ISR por programa
- sin llamadas normales desde ISR
- sin helpers Phase 5 desde ISR

## Limitaciones aritméticas

- multiplicación 8-bit devuelve sólo byte bajo
- multiplicación 16-bit devuelve sólo 16 bits bajos
- división dinámica por cero devuelve `0`

## Limitaciones de validación

- sin emulación/hardware en CI

Estas limitaciones no invalidan el proyecto; definen su frontera actual.

---

# 13. Design Trade-offs

Ésta es una de las secciones más importantes para explicar el proyecto con madurez.

## Por qué simplicidad sobre C completo

Porque el coste principal aquí no es “parsear más gramática”, sino sostener la semántica y el lowering correctos sobre un hardware muy pobre. Cada feature nueva multiplica la complejidad de:

- ABI
- memoria
- banking
- temporales
- diagnósticos

## Por qué Stack-first ABI

Porque:

- escala a 3+ argumentos
- soporta nested calls
- hace posible un frame por invocación
- evita slots globales especiales para argumentos

No se eligió por parecer moderno; se eligió porque el ABI de slots no aguantaba el crecimiento del compilador.

## Por qué helpers runtime

Porque PIC16 no tiene instrucciones para esas operaciones y porque:

- bajar todo inline sería voluminoso
- el helper encapsula algoritmos complejos
- los helpers también ejercitan el ABI reparado

## Por qué ISR restringida

Porque permitir llamadas y helpers en ISR requiere un modelo más fuerte de reentrancia y save/restore. El proyecto prefiere un subconjunto seguro antes que una ISR “más expresiva” pero dudosa.

## Qué haría falta para evolucionar el compilador

Algunas direcciones plausibles:

- casts fuente más completos
- promotions C más cercanas al estándar
- soporte de `switch`
- structs simples
- simulación o tests sobre hardware
- modelo controlado para llamadas desde ISR
- detección runtime de overflow de stack
- quizá un nivel más agresivo de optimización

Pero ninguna de esas extensiones es gratis; todas tensan el núcleo Phase 4/5/6.

---

# 14. How to Explain This Project

## Explicación de 2 minutos

“`pic16cc` es un compilador en Rust para PIC16 clásicos de 14 bits. Toma un subconjunto de C y lo compila hasta Intel HEX real. Internamente separa frontend, IR y backend compartido `midrange14`. Lo interesante no es sólo que soporte enteros de 8 y 16 bits, arrays, punteros, helpers aritméticos e ISR, sino que todo eso está adaptado a limitaciones reales de PIC16: RAM bancarizada, un único registro `W`, ausencia de pila de datos hardware y necesidad de paginar llamadas con `PCLATH`. La Phase 4 introduce un ABI stack-first con pila software; la Phase 5 añade helpers runtime para multiplicación/división/módulo/shifts; y la Phase 6 integra ISR con vector en `0x0004`, save/restore conservador y `retfie`.” 

## Explicación de 5 minutos

“La arquitectura del compilador es muy limpia. `picc` entra por CLI, hace preprocesado, lexing, parsing, semántica, lowering a una IR propia, aplica constant folding y DCE, y luego pasa a un backend PIC16 que conoce banking, paging, stack software, helpers aritméticos e ISR. El proyecto está diseñado por fases. Phase 2 hace reales los 16 bits y las comparaciones; Phase 3 añade arrays, punteros y acceso indirecto con `FSR/INDF`; Phase 4 cambia el ABI a stack-first y mete un frame por llamada; Phase 5 no finge `* / % << >>`, sino que introduce helpers runtime que usan el mismo ABI que las funciones normales; y Phase 6 añade una ISR única con sintaxis `void __interrupt isr(void)`, vector en `0x0004`, save/restore de `W`, `STATUS`, `PCLATH`, `FSR` y estado ABI, y termina con `retfie`. Lo fuerte del proyecto no es la amplitud del subconjunto C, sino la coherencia entre semántica, IR, backend y artefactos `.hex/.map/.lst`.” 

## Explicación técnica profunda

Si te piden una explicación larga, estructura la respuesta así:

1. Problema: PIC16 clásico no tiene stack de datos, tiene RAM bancarizada y un ISA estrecho.
2. Solución de arquitectura: frontend tipado + IR explícita + backend compartido por familia + descriptores de dispositivo.
3. Punto de inflexión: pasar de ABI por slots a ABI stack-first.
4. Consecuencia: locals, arrays locales y temporales IR pasan a ser frame-scoped.
5. Prueba de solidez: los helpers Phase 5 son llamadas internas del compilador que ejercitan ese ABI.
6. Cierre de seguridad: ISR Phase 6 preserva contexto y restringe llamadas/helpers.

## Preguntas típicas y respuestas

### “¿Por qué no usar LLVM?”

Porque el objetivo aquí no es un compilador retargetable general, sino entender y controlar de forma pedagógica una diana muy específica. LLVM también impondría mucha infraestructura que ocultaría justo las decisiones que aquí se quieren estudiar.

### “¿Por qué la pila software crece hacia arriba?”

Porque el proyecto eligió ese convenio y luego lo mantuvo coherente en prólogo, epílogo, cálculo de offsets y análisis de profundidad. No hay nada inherentemente incorrecto en ello.

### “¿Por qué `return_high` en vez de devolver 16 bits de otra manera?”

Porque en PIC16 el retorno de 8 bits por `W` es natural y para 16 bits hace falta un convenio complementario. `W + return_high` es simple, explícito y suficientemente estable para este proyecto.

### “¿Por qué no permitís recursión?”

Porque la profundidad de stack se calcula estáticamente y no hay detección runtime de overflow. Permitir recursión con ese modelo sería técnicamente irresponsable.

### “¿Por qué la ISR no puede llamar funciones?”

Porque el proyecto elige un modelo conservador y seguro: salvar suficiente contexto para código inline ISR es manejable; abrir llamadas y helpers dentro de ISR elevaría mucho el riesgo de corrupción del estado interrumpido.

### “¿Qué hace difícil compilar arrays y punteros en PIC16?”

Que el acceso indirecto requiere programar `FSR/INDF` e `IRP`, mientras que el acceso directo depende de `RP0/RP1`. El compilador tiene que saber continuamente si un byte sale de memoria absoluta, frame o puntero.

### “¿Dónde está la parte más valiosa del diseño?”

En la coherencia de las fronteras: semántica tipada, IR explícita, ABI consistente, backend compartido por familia y artefactos verificables.

---

# 15. Glossary

## ABI

Application Binary Interface. En este proyecto define:

- cómo se pasan argumentos
- cómo se devuelven valores
- quién limpia el stack
- cómo se organiza el frame

## IR

Intermediate Representation. Forma intermedia entre frontend y backend. En `pic16cc` es una IR propia basada en CFG.

## Lowering

Proceso de transformar una representación más abstracta en otra más cercana al target. Ejemplos:

- AST tipado -> IR
- `a[i]` -> decay + pointer arithmetic + `LoadIndirect`
- `a * b` -> helper runtime

## Frame

Espacio lógico de una invocación de función. Contiene:

- argumentos visibles por el callee
- `saved FP`
- locales
- arrays locales
- temporales IR

## Stack

Pila software en RAM usada para argumentos, frames y temporales por invocación. No confundir con la pila hardware de retornos del PIC16.

## SFR

Special Function Register. Registros mapeados en memoria para periféricos y control del micro, por ejemplo:

- `STATUS`
- `PORTB`
- `TRISB`
- `INTCON`

## Bank switching

Cambio de banco de RAM directa usando `STATUS.RP0/RP1`.

## Indirect addressing

Acceso a memoria usando `FSR` + `INDF`, con `IRP` para seleccionar banco indirecto.

## PCLATH

Registro auxiliar usado para paginar llamadas y saltos de programa en PIC16.

## `W`

Working register del PIC16. Actúa como acumulador central en gran parte del lowering.

## `return_high`

Slot helper del backend usado para devolver el byte alto de valores de 16 bits o punteros.

## `stack_ptr` / `frame_ptr`

Slots ABI en RAM que implementan la pila software y el frame activo.

## CFG

Control Flow Graph. Representación por bloques y terminadores usada en la IR.

## Constant folding

Optimización que evalúa expresiones constantes en IR antes de codegen.

## Dead code elimination

Eliminación de instrucciones que producen temporales cuyo resultado nunca se usa.

## `retfie`

Return From Interrupt Enable. Instrucción PIC16 específica para salir de ISR.

## Shared GPR

Zona de RAM común/mirroring usada aquí para contexto ISR, para poder restaurar `W` y otros registros sin depender de un banco activo inestable.

---

# Cierre

Si tuvieras que resumir el valor técnico de `pic16cc` en una sola frase, sería ésta:

> Es un compilador pequeño pero conceptualmente serio, que usa las limitaciones de PIC16 para obligarse a tomar decisiones de ABI, memoria, lowering e ISR que en arquitecturas más cómodas quedan ocultas.

Para estudiarlo bien, el mejor orden práctico es:

1. `src/lib.rs` para ver el pipeline
2. `src/frontend/semantic.rs` para entender el contrato del lenguaje
3. `src/ir/model.rs` y `src/ir/lowering.rs` para entender la frontera frontend/backend
4. `src/backend/pic16/midrange14/codegen.rs` para ver cómo todo aterriza en PIC16
5. `tests/compiler_pipeline.rs` para entender qué garantiza hoy el proyecto

Ese recorrido coincide bastante bien con cómo defenderías el proyecto ante otra persona técnica.
