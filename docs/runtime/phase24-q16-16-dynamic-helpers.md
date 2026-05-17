<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 24 Q16.16 Dynamic Helpers

Phase 24 implements dynamic Q16.16 and UQ16.16 multiply/divide without exposing `long long` as a public C type.

Helper labels:

- `__rt_mul_q16_16`
- `__rt_mul_uq16_16`
- `__rt_div_q16_16`
- `__rt_div_uq16_16`

ABI behavior:

- arguments are raw 32-bit fixed values, pushed low byte first
- helpers use the existing Stack-first ABI
- returns use `W`, `return_high`, `return_upper0`, and `return_upper1`
- helper labels appear in `.map` and `.lst`
- helper frame and argument cost is included in stack reports and `--stack-check`

Algorithms:

- multiply computes partial 16x16 products and accumulates `(raw_a * raw_b) >> 16`
- divide performs unsigned 32-bit division for the integer part, then 16 fractional restoring-division steps
- signed helpers normalize input signs, run the unsigned core, then negate the raw result when needed

Division by zero:

- constant fixed division by zero remains a diagnostic
- dynamic fixed division by zero returns raw `0`, matching current integer helper policy

Restrictions:

- fixed modulo remains unsupported
- helper-backed fixed operations remain rejected inside ISRs
- helper-heavy programs can exceed small-device stack or program-memory limits and are diagnosed through existing stack/program constraints

Phase 27 float helpers reuse the same resource-reporting and stack-accounting paths. They store public values as f32 bits but use an internal Q16.16 work format, so helper-heavy float programs may also exceed target limits.
## Phase 33 Cost Reporting

Q16.16 dynamic helpers are categorized as fixed helpers in Phase 33. Reports include actual emitted words, estimated catalog words, ABI argument bytes, helper frame bytes, and page placement. Use `--memory-report` to see whether Q16.16 helpers dominate the target image.
