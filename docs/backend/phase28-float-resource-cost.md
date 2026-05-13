# Phase 28 Float Resource Cost

Phase 28 float cast helpers are large and target fitting must be checked before hardware use.

Resource reporting:

- `--size` includes emitted float helpers in total program words
- `--memory-report` lists helper labels as `float helper`
- `--stack-report` includes helper stack contribution via `helper_extra`
- `.map` and `.lst` include helper labels and bodies

Important helper labels:

- `__rt_q16_16_to_f32`
- `__rt_f32_to_q16_16`
- `__rt_f32_add`
- `__rt_f32_sub`
- `__rt_f32_mul`
- `__rt_f32_div`

PIC16F628A may reject float-heavy programs. PIC16F877A is the recommended validation target for Phase 28 float examples.
