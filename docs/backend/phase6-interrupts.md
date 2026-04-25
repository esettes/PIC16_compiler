# Phase 6 Interrupts

Targets:

- `PIC16F628A`
- `PIC16F877A`

Chosen ISR syntax:

- `void __interrupt isr(void)`

Vector layout:

- `0x0000`: reset vector, direct `goto __reset_dispatch`
- `0x0004`: interrupt vector
- with ISR: direct `goto __interrupt_dispatch`
- without ISR: `retfie`

Dispatch stubs after `0x0004` handle `PCLATH` setup before branching to:

- `__start`
- ISR label

ISR context save/restore:

- save `W`
- save `STATUS`
- save `PCLATH`
- save `FSR`
- save `return_high`
- save `scratch0`
- save `scratch1`
- save `stack_ptr`
- save `frame_ptr`

Context storage lives in shared GPR addresses so final `W` restore can use `swapf` after `STATUS` is restored.

ISR frame policy:

- ISR reuses the normal Phase 4 stack-first frame model
- save interrupted context first
- then run normal frame prologue
- then lower ISR body
- then run frame epilogue
- then restore interrupted context
- end with `retfie`

Phase 6 policy for calls/helpers:

- no normal calls inside ISR
- no Phase 5 helper calls inside ISR
- helper-requiring `*`, `/`, `%`, and dynamic shifts are rejected in semantic analysis

Output expectations:

- `.map` shows `__interrupt_vector` and `__isr_ctx.*`
- `.lst` shows vector stub, ISR body, save/restore sequence, and `retfie`
- `.hex` still includes config word at `0x2007`
