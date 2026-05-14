# Phase 30 ROM Float Lowering

`const __rom float[]` indexing lowers through the existing ROM-read IR:

```text
t = rom32 table[index]
```

The result temp has type `float` and carries raw 32-bit f32 bits. No separate ROM-float pointer type is introduced.

Lowering behavior:

- constant index: backend may inline the four RETLW payload bytes.
- dynamic index: backend computes byte offset `index * 4` and performs four byte reads.
- out-of-range dynamic reads return zero bytes, matching existing ROM read behavior.
- ROM float objects do not produce startup RAM initialization.
- helper-backed arithmetic/comparison after a ROM read lowers exactly like ordinary float work and remains resource-heavy.

Limitations:

- no ROM/data pointer conversion
- no address-of ROM element
- no multidimensional ROM float arrays
- no dynamic ROM read inside ISR
