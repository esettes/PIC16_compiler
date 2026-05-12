<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Phase 26 HEX Validation

Phase 26 validates final Intel HEX after codegen and config resolution.

Always checked:

- reset vector word exists at `0x0000`
- interrupt vector word exists at `0x0004`
- config word is emitted at target config address
- program words do not overlap config word
- program words stay inside target program-memory range
- emitted instruction words are 14-bit PIC16 words
- HEX bytes match encoded words
- Intel HEX checksums are correct
- EOF record exists

`--verify-hex` prints the validation report. Validation errors are hard compile failures even when `--size` is not requested.

This closes the Phase 25 gap where detailed resource reports were optional but invalid target output could still be generated for old oversized examples.
