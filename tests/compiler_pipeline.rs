use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use pic16cc::cli::{CliCommand, CliOptions, CompileCommand, OptimizationLevel, OutputArtifacts};
use pic16cc::diagnostics::WarningProfile;
use pic16cc::execute;

/// Resolves a repository-relative path inside the test workspace.
fn repo(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
}

/// Creates a unique temporary file path for one test artifact.
fn temp_file(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("pic16cc-{stamp}-{name}"))
}

/// Compiles one input file with artifact dumps enabled and returns the HEX path.
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

/// Compiles one checked-in example file for the requested target.
fn compile_example(target: &str, input: &str) -> PathBuf {
    compile_input(target, repo(input))
}

/// Writes source text to a temporary file and compiles it like a user input.
fn compile_source(target: &str, name: &str, source: &str) -> PathBuf {
    let input = temp_file(name);
    fs::write(&input, source).expect("fixture");
    compile_input(target, input)
}

/// Compiles one temporary source expecting a diagnostic failure and returns the rendered error text.
fn compile_error(target: &str, name: &str, source: &str) -> String {
    let input = temp_file(name);
    fs::write(&input, source).expect("fixture");
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: target.to_string(),
            input,
            output: temp_file("error.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");
    format!("{error}")
}

/// Checks that the generated HEX includes both config data and the EOF record.
fn assert_hex_is_programmable(output: &Path) {
    let hex = fs::read_to_string(output).expect("hex");
    assert!(hex.contains(":02400E00"));
    assert!(hex.contains(":00000001FF"));
}

/// Reads one side artifact generated next to the compiled HEX output.
fn read_artifact(output: &Path, extension: &str) -> String {
    fs::read_to_string(output.with_extension(extension)).expect("artifact")
}

/// Counts non-overlapping substring matches in one artifact string.
fn count_occurrences(haystack: &str, needle: &str) -> usize {
    haystack.match_indices(needle).count()
}

/// Verifies Phase 4 artifacts include stack metadata in asm/map/listing outputs.
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

/// Verifies Phase 5 helper artifacts expose one runtime helper in asm/map/listing outputs.
fn assert_phase5_helper_artifacts(output: &Path, helper: &str) {
    let asm = read_artifact(output, "asm");
    let map = read_artifact(output, "map");
    let listing = read_artifact(output, "lst");

    assert!(asm.contains(&format!("call {helper}")));
    assert!(asm.contains(&format!("{helper}:")));
    assert!(map.contains(helper));
    assert!(listing.contains(helper));
}

/// Verifies Phase 6 artifacts expose the interrupt vector, ISR symbol, and saved-context slots.
fn assert_phase6_interrupt_artifacts(output: &Path, isr_symbol: &str) {
    let asm = read_artifact(output, "asm");
    let map = read_artifact(output, "map");
    let listing = read_artifact(output, "lst");

    assert!(asm.contains("org 0x0004"));
    assert!(asm.contains("__interrupt_vector:"));
    assert!(asm.contains("goto __interrupt_dispatch"));
    assert!(asm.contains(&format!("{isr_symbol}:")));
    assert!(asm.contains("retfie"));
    assert!(map.contains("__interrupt_vector"));
    assert!(map.contains("__isr_ctx.w"));
    assert!(map.contains("__isr_ctx.status"));
    assert!(map.contains("__isr_ctx.stack_ptr.lo"));
    assert!(listing.contains("__interrupt_vector"));
    assert!(listing.contains("retfie"));
}

#[test]
/// Verifies the original PIC16F628A blink example still compiles successfully.
fn compiles_pic16f628a_blink() {
    let output = compile_example("pic16f628a", "examples/pic16f628a/blink.c");
    assert_hex_is_programmable(&output);
    assert!(read_artifact(&output, "map").contains("Code Symbols"));
    assert!(read_artifact(&output, "lst").contains("Assembly"));
}

#[test]
/// Verifies the original PIC16F877A blink example still emits assembly.
fn compiles_pic16f877a_blink() {
    let output = compile_example("pic16f877a", "examples/pic16f877a/blink.c");
    let asm = read_artifact(&output, "asm");
    assert!(asm.contains("fn_main"));
    assert!(asm.contains("movwf"));
}

#[test]
/// Verifies unsigned 16-bit arithmetic and relational lowering on PIC16F628A.
fn compiles_unsigned_16bit_phase2_example() {
    let output = compile_example("pic16f628a", "examples/pic16f628a/arith16.c");
    let ir = read_artifact(&output, "ir");
    let asm = read_artifact(&output, "asm");
    let map = read_artifact(&output, "map");

    assert_hex_is_programmable(&output);
    assert!(ir.contains("Less"));
    assert!(ir.contains("GreaterEqual"));
    assert!(asm.contains("fn_add16"));
    assert!(asm.contains("call fn_add16"));
    assert!(asm.contains("addwf"));
    assert!(asm.contains("subwf"));
    assert!(map.contains("threshold"));
    assert!(map.contains("counter"));
}

#[test]
/// Verifies signed 16-bit arithmetic and relational lowering on PIC16F877A.
fn compiles_signed_16bit_phase2_example() {
    let output = compile_example("pic16f877a", "examples/pic16f877a/compare16.c");
    let ir = read_artifact(&output, "ir");
    let asm = read_artifact(&output, "asm");

    assert_hex_is_programmable(&output);
    assert!(ir.contains("Less"));
    assert!(ir.contains("LessEqual"));
    assert!(ir.contains("Greater"));
    assert!(asm.contains("fn_adjust16"));
    assert!(asm.contains("call fn_adjust16"));
    assert!(asm.contains("xorwf"));
    assert!(asm.contains("btfss"));
}

#[test]
/// Verifies byte-array decay, indexing, and indirect loads/stores lower to PIC16 assembly.
fn compiles_phase3_byte_array_example() {
    let output = compile_example("pic16f628a", "examples/pic16f628a/array_fill.c");
    let ir = read_artifact(&output, "ir");
    let asm = read_artifact(&output, "asm");
    let map = read_artifact(&output, "map");

    assert_hex_is_programmable(&output);
    assert!(ir.contains("= &s"));
    assert!(ir.contains("= *"));
    assert!(ir.contains("*t"));
    assert!(asm.contains("movwf 0x04"));
    assert!(asm.contains("movf 0x00,w"));
    assert!(asm.contains("movwf 0x00"));
    assert!(map.contains("shadow"));
    assert!(map.contains("total"));
}

#[test]
/// Verifies 16-bit arrays, pointer equality, and indirect SFR writes compile end to end.
fn compiles_phase3_word_pointer_example() {
    let output = compile_example("pic16f877a", "examples/pic16f877a/pointer16.c");
    let ir = read_artifact(&output, "ir");
    let asm = read_artifact(&output, "asm");
    let map = read_artifact(&output, "map");

    assert_hex_is_programmable(&output);
    assert!(ir.contains("= &s"));
    assert!(ir.contains("= *"));
    assert!(ir.contains("Equal"));
    assert!(asm.contains("movwf 0x04"));
    assert!(asm.contains("movf 0x00,w"));
    assert!(asm.contains("movwf 0x00"));
    assert!(map.contains("words"));
    assert!(map.contains("mirror"));
}

#[test]
/// Verifies the Phase 4 stack ABI handles 3+ arguments and nested calls on PIC16F628A.
fn compiles_phase4_stack_abi_example() {
    let output = compile_example("pic16f628a", "examples/pic16f628a/stack_abi.c");
    let asm = read_artifact(&output, "asm");
    let map = read_artifact(&output, "map");

    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert!(asm.contains("stack base="));
    assert!(asm.contains("call fn_sum4"));
    assert!(asm.contains("call fn_build_local"));
    assert!(asm.contains("call fn_sum_bytes"));
    assert!(map.contains("final_value"));
}

#[test]
/// Verifies the Phase 4 frame model handles deeper non-recursive call chains on PIC16F877A.
fn compiles_phase4_call_chain_example() {
    let output = compile_example("pic16f877a", "examples/pic16f877a/call_chain.c");
    let asm = read_artifact(&output, "asm");
    let map = read_artifact(&output, "map");

    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert!(asm.contains("stack base="));
    assert!(asm.contains("call fn_top_sum"));
    assert!(asm.contains("call fn_middle_sum"));
    assert!(asm.contains("call fn_leaf_sum"));
    assert!(map.contains("latest"));
}

#[test]
/// Verifies unsigned 8-bit multiplication lowers through the Phase 5 helper path.
fn compiles_phase5_mul8_fixture() {
    let output = compile_source(
        "pic16f628a",
        "mul8.c",
        "\
#include <pic16/pic16f628a.h>
unsigned char mul8(unsigned char a, unsigned char b) {
    return a * b;
}
void main(void) {
    TRISB = 0x00;
    PORTB = mul8(6, 7);
}
",
    );

    assert_hex_is_programmable(&output);
    assert_phase5_helper_artifacts(&output, "__rt_mul_u8");
}

#[test]
/// Verifies unsigned 16-bit multiplication lowers end to end through runtime helpers.
fn compiles_phase5_mul16_example() {
    let output = compile_example("pic16f877a", "examples/pic16f877a/mul16.c");

    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert_phase5_helper_artifacts(&output, "__rt_mul_u16");
}

#[test]
/// Verifies signed 16-bit division lowers end to end through runtime helpers.
fn compiles_phase5_div16_example() {
    let output = compile_example("pic16f877a", "examples/pic16f877a/div16.c");

    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert_phase5_helper_artifacts(&output, "__rt_div_i16");
}

#[test]
/// Verifies unsigned 16-bit modulo lowers end to end through runtime helpers.
fn compiles_phase5_mod16_example() {
    let output = compile_example("pic16f877a", "examples/pic16f877a/mod16.c");

    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert_phase5_helper_artifacts(&output, "__rt_mod_u16");
}

#[test]
/// Verifies mixed inline/runtime shift lowering emits only the dynamic helper path.
fn compiles_phase5_shift_mix_example() {
    let output = compile_example("pic16f877a", "examples/pic16f877a/shift_mix.c");
    let asm = read_artifact(&output, "asm");

    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert_phase5_helper_artifacts(&output, "__rt_shr_u16");
    assert!(!asm.contains("call __rt_shl16"));
}

#[test]
/// Verifies one expression tree can combine multiple runtime helpers safely.
fn compiles_phase5_expression_example() {
    let output = compile_example("pic16f877a", "examples/pic16f877a/expression_test.c");
    let asm = read_artifact(&output, "asm");

    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert!(asm.contains("call __rt_mul_u16"));
    assert!(asm.contains("call __rt_div_u16"));
    assert!(asm.contains("call __rt_mod_u16"));
}

#[test]
/// Verifies the PIC16F628A timer ISR example emits the interrupt vector and `retfie`.
fn compiles_phase6_pic16f628a_timer_interrupt_example() {
    let output = compile_example("pic16f628a", "examples/pic16f628a/timer_interrupt.c");

    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert_phase6_interrupt_artifacts(&output, "fn_isr");
}

#[test]
/// Verifies the PIC16F877A timer ISR example emits the interrupt vector and saved context.
fn compiles_phase6_pic16f877a_timer_interrupt_example() {
    let output = compile_example("pic16f877a", "examples/pic16f877a/timer_interrupt.c");

    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert_phase6_interrupt_artifacts(&output, "fn_isr");
}

#[test]
/// Verifies the PIC16F877A GPIO ISR example compiles with the same Phase 6 vector shape.
fn compiles_phase6_pic16f877a_gpio_interrupt_example() {
    let output = compile_example("pic16f877a", "examples/pic16f877a/gpio_interrupt.c");

    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert_phase6_interrupt_artifacts(&output, "fn_isr");
}

#[test]
/// Verifies pointer arguments, pointer returns, and local array decay through a fixture.
fn compiles_phase3_pointer_return_fixture() {
    let output = compile_source(
        "pic16f628a",
        "pointer-return.c",
        "\
#include <pic16/pic16f628a.h>
unsigned char bytes[2];
/** Returns the same pointer passed by the caller. */
unsigned char *pick(unsigned char *ptr) {
    return ptr;
}
/** Exercises pointer arguments, returns, equality, and indirect loads. */
void main(void) {
    unsigned char *cursor = pick(bytes);
    TRISB = 0x00;
    PORTB = 0x00;
    cursor[0] = 0x11;
    cursor[1] = 0x22;
    if (cursor == bytes) {
        PORTB = cursor[0];
    }
}
",
    );

    let ir = read_artifact(&output, "ir");
    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert!(ir.contains("call f"));
    assert!(ir.contains("Equal"));
    assert!(asm.contains("call fn_pick"));
    assert!(asm.contains("movwf 0x04"));
}

#[test]
/// Verifies a five-argument call compiles through the Phase 4 caller-pushed stack ABI.
fn compiles_phase4_five_argument_fixture() {
    let output = compile_source(
        "pic16f628a",
        "sum5.c",
        "\
#include <pic16/pic16f628a.h>
/** Returns the sum of five arguments through the Phase 4 ABI. */
unsigned int sum5(unsigned int a, unsigned int b, unsigned int c, unsigned int d, unsigned int e) {
    return a + b + c + d + e;
}
/** Exercises a five-argument call and 16-bit return handling. */
void main(void) {
    unsigned int value = sum5(1, 2, 3, 4, 5);
    TRISB = 0x00;
    if (value >= 15) {
        PORTB = 0x77;
    }
}
",
    );

    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert!(asm.contains("call fn_sum5"));
    assert!(asm.contains("stack base="));
}

#[test]
/// Verifies two sequential calls in one caller preserve a coherent Phase 4 stack contract.
fn compiles_phase4_sequential_call_regression_fixture() {
    let output = compile_source(
        "pic16f628a",
        "sequential-calls.c",
        "\
#include <pic16/pic16f628a.h>
int add2(int a, int b) {
    return a + b;
}
int top_sum(void) {
    int x;
    int y;
    x = add2(1, 2);
    y = add2(3, 4);
    return x + y;
}
void main(void) {
    int total = top_sum();
    TRISB = 0x00;
    PORTB = total;
}
",
    );

    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert_eq!(count_occurrences(&asm, "call fn_add2"), 2);
    assert!(asm.contains("call fn_top_sum"));
    assert!(asm.contains("frame args=4 saved_fp=4"));
}

#[test]
/// Verifies nested call chains emit consistent stack metadata and call lowering.
fn compiles_phase4_nested_call_regression_fixture() {
    let output = compile_source(
        "pic16f628a",
        "nested-calls.c",
        "\
#include <pic16/pic16f628a.h>
int f(int x) { return x + 1; }
int g(int y) { return f(y) + 2; }
int h(int z) { return g(z) + 3; }
void main(void) {
    int total = h(5);
    TRISB = 0x00;
    PORTB = total;
}
",
    );

    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert!(asm.contains("call fn_f"));
    assert!(asm.contains("call fn_g"));
    assert!(asm.contains("call fn_h"));
}

#[test]
/// Verifies temps survive nested calls when one subexpression is lowered before a call.
fn compiles_phase4_temp_liveness_nested_call_fixture() {
    let output = compile_source(
        "pic16f628a",
        "temp-nested.c",
        "\
#include <pic16/pic16f628a.h>
int inc(int x) {
    return x + 1;
}
int combine(int a, int b, int c, int d) {
    return (a + b) + inc(c + d);
}
void main(void) {
    int total = combine(1, 2, 3, 4);
    TRISB = 0x00;
    PORTB = total;
}
",
    );

    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert!(asm.contains("call fn_inc"));
    assert!(asm.contains("call fn_combine"));
}

#[test]
/// Verifies sibling call expressions keep caller temps live across two independent calls.
fn compiles_phase4_temp_liveness_sibling_calls_fixture() {
    let output = compile_source(
        "pic16f628a",
        "temp-siblings.c",
        "\
#include <pic16/pic16f628a.h>
int f(int x) {
    return x + 1;
}
void main(void) {
    int a;
    int b;
    int total;
    a = 10;
    b = 20;
    total = f(a + b) + f(a - b);
    TRISB = 0x00;
    PORTB = total;
}
",
    );

    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert_eq!(count_occurrences(&asm, "call fn_f"), 2);
}

#[test]
/// Verifies 16-bit equality and inequality lowering through a temporary fixture.
fn compiles_16bit_equality_fixture() {
    let output = compile_source(
        "pic16f628a",
        "eq16.c",
        "\
#include <pic16/pic16f628a.h>
unsigned int mix16(unsigned int lhs, unsigned int rhs) {
    if (lhs == rhs) {
        return lhs + 1;
    }
    if (lhs != rhs) {
        return rhs - lhs;
    }
    return 0;
}
void main(void) {
    unsigned int value = mix16(3, 7);
    TRISB = 0x00;
    if (value != 0) {
        PORTB = 0x11;
    }
}
",
    );

    let ir = read_artifact(&output, "ir");
    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert!(ir.contains("Equal"));
    assert!(ir.contains("NotEqual"));
    assert!(asm.contains("fn_mix16"));
    assert!(asm.contains("call fn_mix16"));
    assert!(asm.contains("subwf"));
}

#[test]
/// Verifies local arrays, `sizeof`, and pointer traversal work in one integration fixture.
fn compiles_phase3_sizeof_and_local_array_fixture() {
    let output = compile_source(
        "pic16f877a",
        "sizeof-array.c",
        "\
#include <pic16/pic16f877a.h>
unsigned char result = 0;
/** Accumulates bytes from a caller-provided span. */
unsigned char accumulate(unsigned char *ptr, unsigned int len) {
    unsigned int i = 0;
    unsigned char acc = 0;
    while (i < len) {
        acc = acc + ptr[i];
        i = i + 1;
    }
    return acc;
}
/** Exercises local arrays, `sizeof`, pointer indexing, and indirect loads. */
void main(void) {
    unsigned char local[4];
    unsigned char *cursor = local;
    TRISB = 0x00;
    ADCON1 = 0x06;
    local[0] = sizeof(char);
    local[1] = sizeof(unsigned int);
    local[2] = sizeof(cursor);
    local[3] = sizeof(local);
    result = accumulate(local, sizeof(local));
    PORTB = result;
}
",
    );

    let ir = read_artifact(&output, "ir");
    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert!(ir.contains("call f"));
    assert!(asm.contains("call fn_accumulate"));
    assert!(asm.contains("movf 0x00,w"));
}

#[test]
/// Verifies taking the address of a stack local and passing it across a call compiles.
fn compiles_phase4_address_of_local_fixture() {
    let output = compile_source(
        "pic16f877a",
        "addr-local.c",
        "\
#include <pic16/pic16f877a.h>
/** Loads one byte through a caller-provided pointer. */
unsigned char load_byte(unsigned char *ptr, unsigned int index, unsigned char fallback) {
    if (index != 0) {
        return ptr[index];
    }
    return fallback;
}
/** Exercises `&local`, pointer arguments, and stack-backed local scalars. */
void main(void) {
    unsigned char local = 0x21;
    unsigned char *ptr = &local;
    TRISB = 0x00;
    ADCON1 = 0x06;
    PORTB = load_byte(ptr, 0, local);
}
",
    );

    let asm = read_artifact(&output, "asm");
    assert_hex_is_programmable(&output);
    assert!(asm.contains("call fn_load_byte"));
}

#[test]
/// Verifies helper calls survive alongside nested function calls and temp lifetimes.
fn compiles_phase5_helper_nested_expression_fixture() {
    let output = compile_source(
        "pic16f628a",
        "helper-nested.c",
        "\
#include <pic16/pic16f628a.h>
unsigned int inc(unsigned int x) {
    return x + 1;
}
unsigned int combine(unsigned int a, unsigned int b, unsigned int c, unsigned int d) {
    return (a + b) + inc(c * d);
}
void main(void) {
    TRISB = 0x00;
    PORTB = combine(1, 2, 3, 4);
}
",
    );
    let asm = read_artifact(&output, "asm");

    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert!(asm.contains("call __rt_mul_u16"));
    assert!(asm.contains("call fn_inc"));
}

#[test]
/// Verifies helper calls coexist with pointer and local-array lowering from earlier phases.
fn compiles_phase5_pointer_array_helper_fixture() {
    let output = compile_source(
        "pic16f877a",
        "pointer-shift.c",
        "\
#include <pic16/pic16f877a.h>
unsigned int shift_first(unsigned int *ptr, unsigned char n) {
    return ptr[0] >> n;
}
void main(void) {
    unsigned int words[2];
    words[0] = 0x0123;
    words[1] = 0x0040;
    ADCON1 = 0x06;
    TRISB = 0x00;
    PORTB = shift_first(words, 3);
}
",
    );

    assert_hex_is_programmable(&output);
    assert_phase4_stack_metadata(&output);
    assert_phase5_helper_artifacts(&output, "__rt_shr_u16");
}

#[test]
/// Verifies division by constant zero is rejected before lowering.
fn reports_division_by_constant_zero() {
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: {
                let input = temp_file("div-zero.c");
                fs::write(
                    &input,
                    "\
#include <pic16/pic16f628a.h>
unsigned int bad(unsigned int value) {
    return value / 0;
}
void main(void) {
    TRISB = 0x00;
    PORTB = bad(7);
}
",
                )
                .expect("fixture");
                input
            },
            output: temp_file("div-zero.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("division by constant zero"));
}

#[test]
/// Verifies modulo by constant zero is rejected before lowering.
fn reports_modulo_by_constant_zero() {
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: {
                let input = temp_file("mod-zero.c");
                fs::write(
                    &input,
                    "\
#include <pic16/pic16f628a.h>
unsigned int bad(unsigned int value) {
    return value % 0;
}
void main(void) {
    TRISB = 0x00;
    PORTB = bad(7);
}
",
                )
                .expect("fixture");
                input
            },
            output: temp_file("mod-zero.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("modulo by constant zero"));
}

#[test]
/// Verifies constant shift counts wider than the operand are rejected explicitly.
fn reports_constant_shift_count_too_wide() {
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: {
                let input = temp_file("shift-wide.c");
                fs::write(
                    &input,
                    "\
#include <pic16/pic16f628a.h>
unsigned char bad(unsigned char value) {
    return value << 8;
}
void main(void) {
    TRISB = 0x00;
    PORTB = bad(1);
}
",
                )
                .expect("fixture");
                input
            },
            output: temp_file("shift-wide.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("constant shift count"));
}

#[test]
/// Verifies interrupt handlers must return `void`.
fn reports_interrupt_return_type_mismatch() {
    let error = compile_error(
        "pic16f628a",
        "isr-ret-type.c",
        "\
#include <pic16/pic16f628a.h>
int __interrupt isr(void) {
    return 1;
}
void main(void) {
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("must return `void`"));
}

#[test]
/// Verifies interrupt handlers cannot take parameters.
fn reports_interrupt_parameter_mismatch() {
    let error = compile_error(
        "pic16f628a",
        "isr-params.c",
        "\
#include <pic16/pic16f628a.h>
void __interrupt isr(int value) {
}
void main(void) {
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("cannot take parameters"));
}

#[test]
/// Verifies Phase 6 rejects multiple interrupt handlers in one program.
fn reports_multiple_interrupt_handlers() {
    let error = compile_error(
        "pic16f628a",
        "two-isr.c",
        "\
#include <pic16/pic16f628a.h>
void __interrupt isr1(void) {
    PORTB = PORTB ^ 0x01;
}
void __interrupt isr2(void) {
    PORTB = PORTB ^ 0x02;
}
void main(void) {
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("only one interrupt handler"));
}

#[test]
/// Verifies Phase 6 rejects normal function calls inside ISRs.
fn reports_interrupt_function_calls() {
    let error = compile_error(
        "pic16f628a",
        "isr-call.c",
        "\
#include <pic16/pic16f628a.h>
void helper(void) {
    PORTB = 0x33;
}
void __interrupt isr(void) {
    helper();
}
void main(void) {
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("cannot call `helper`"));
}

#[test]
/// Verifies Phase 6 rejects runtime-helper arithmetic inside ISRs.
fn reports_interrupt_runtime_helper_arithmetic() {
    let error = compile_error(
        "pic16f628a",
        "isr-helper.c",
        "\
#include <pic16/pic16f628a.h>
void __interrupt isr(void) {
    int a = 3;
    int b = 4;
    int c;
    c = a * b;
    PORTB = c;
}
void main(void) {
    TRISB = 0x00;
}
",
    );

    assert!(error.contains("runtime helper"));
}

#[test]
/// Verifies mixed-sign 16-bit comparisons are rejected instead of silently coerced.
fn reports_mixed_signedness_compare() {
    let input = temp_file("mixed-signedness.c");
    fs::write(
        &input,
        "\
#include <pic16/pic16f628a.h>
void main(void) {
    int signed_value = -1;
    unsigned int unsigned_value = 1;
    TRISB = 0x00;
    if (signed_value < unsigned_value) {
        PORTB = 0x01;
    }
}
",
    )
    .expect("fixture");

    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input,
            output: temp_file("mixed-signedness.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("mixed signedness"));
}

#[test]
/// Verifies direct recursion is rejected under the current non-reentrant Phase 4 model.
fn reports_unsupported_recursion() {
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: {
                let input = temp_file("recursion.c");
                fs::write(
                    &input,
                    "\
#include <pic16/pic16f628a.h>
int loop_forever(int value) {
    return loop_forever(value);
}
void main(void) {
    TRISB = 0x00;
    PORTB = loop_forever(1);
}
",
                )
                .expect("fixture");
                input
            },
            output: temp_file("recursion.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("recursive call cycle"));
}

#[test]
/// Verifies returning the address of a stack local is rejected clearly.
fn reports_returning_stack_local_address() {
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: {
                let input = temp_file("return-local.c");
                fs::write(
                    &input,
                    "\
#include <pic16/pic16f628a.h>
unsigned char *bad_ptr(void) {
    unsigned char local[2];
    return local;
}
void main(void) {
    unsigned char *ptr = bad_ptr();
    TRISB = 0x00;
    PORTB = ptr[0];
}
",
                )
                .expect("fixture");
                input
            },
            output: temp_file("return-local.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("stack local"));
}

#[test]
/// Verifies obvious local-pointer alias chains are rejected before lowering.
fn reports_returning_stack_local_pointer_alias() {
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: {
                let input = temp_file("return-local-alias.c");
                fs::write(
                    &input,
                    "\
#include <pic16/pic16f628a.h>
unsigned int *bad_ptr(void) {
    unsigned int local;
    unsigned int *p;
    p = &local;
    return p;
}
void main(void) {
    unsigned int *ptr = bad_ptr();
    TRISB = 0x00;
    PORTB = 0x00;
}
",
                )
                .expect("fixture");
                input
            },
            output: temp_file("return-local-alias.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("stack local"));
}

#[test]
/// Verifies statically oversized local allocations fail with a stack-capacity diagnostic.
fn reports_oversized_local_allocation() {
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: {
                let input = temp_file("oversized-local.c");
                fs::write(
                    &input,
                    "\
#include <pic16/pic16f628a.h>
void main(void) {
    unsigned char local[80];
    TRISB = 0x00;
    PORTB = local[0];
}
",
                )
                .expect("fixture");
                input
            },
            output: temp_file("oversized-local.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("software stack"));
}

#[test]
/// Verifies pointer-to-pointer declarations are rejected instead of lowering partially.
fn reports_unsupported_pointer_to_pointer() {
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: {
                let input = temp_file("ptrptr.c");
                fs::write(
                    &input,
                    "\
#include <pic16/pic16f628a.h>
void main(void) {
    unsigned char **pp;
    TRISB = 0x00;
    PORTB = 0x00;
}
",
                )
                .expect("fixture");
                input
            },
            output: temp_file("ptrptr.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("unsupported type"));
}

#[test]
/// Verifies function pointer declarators fail with an explicit Phase 3 diagnostic.
fn reports_unsupported_function_pointer() {
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: {
                let input = temp_file("fnptr.c");
                fs::write(
                    &input,
                    "\
#include <pic16/pic16f628a.h>
void main(void) {
    int (*fp)(void);
    TRISB = 0x00;
}
",
                )
                .expect("fixture");
                input
            },
            output: temp_file("fnptr.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("function pointer declarators"));
}

#[test]
/// Verifies multidimensional arrays are rejected before lowering.
fn reports_unsupported_multidimensional_array() {
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: {
                let input = temp_file("multidim.c");
                fs::write(
                    &input,
                    "\
#include <pic16/pic16f628a.h>
unsigned char table[2][2];
void main(void) {
    TRISB = 0x00;
}
",
                )
                .expect("fixture");
                input
            },
            output: temp_file("multidim.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("multidimensional arrays"));
}

#[test]
/// Verifies unsupported pointer arithmetic forms are rejected clearly.
fn reports_unsupported_pointer_pointer_subtraction() {
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: {
                let input = temp_file("ptrsub.c");
                fs::write(
                    &input,
                    "\
#include <pic16/pic16f628a.h>
unsigned char bytes[2];
void main(void) {
    unsigned char *lhs = bytes;
    unsigned char *rhs = bytes;
    TRISB = 0x00;
    if ((lhs - rhs) != 0) {
        PORTB = 0x01;
    }
}
",
                )
                .expect("fixture");
                input
            },
            output: temp_file("ptrsub.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("unsupported pointer arithmetic form"));
}

#[test]
/// Verifies array initializers fail explicitly instead of being silently ignored.
fn reports_unsupported_array_initializer() {
    let error = execute(CliOptions {
        command: CliCommand::Compile(CompileCommand {
            target: "pic16f628a".to_string(),
            input: {
                let input = temp_file("array-init.c");
                fs::write(
                    &input,
                    "\
#include <pic16/pic16f628a.h>
unsigned char bytes[2] = 0;
void main(void) {
    TRISB = 0x00;
}
",
                )
                .expect("fixture");
                input
            },
            output: temp_file("array-init.hex"),
            include_dirs: vec![repo("include")],
            defines: BTreeMap::new(),
            optimization: OptimizationLevel::O0,
            artifacts: OutputArtifacts::default(),
            verbose: false,
            warning_profile: WarningProfile::default(),
        }),
    })
    .expect_err("must fail");

    assert!(format!("{error}").contains("array initializers"));
}

#[test]
/// Verifies the README command shape still parses into the expected CLI options.
fn parses_cli_shape_requested_in_readme() {
    let options = CliOptions::parse(vec![
        "pic16cc".to_string(),
        "--target".to_string(),
        "pic16f628a".to_string(),
        "-I".to_string(),
        "include".to_string(),
        "-O2".to_string(),
        "-Wall".to_string(),
        "-Wextra".to_string(),
        "--emit-ast".to_string(),
        "--emit-ir".to_string(),
        "--emit-asm".to_string(),
        "--map".to_string(),
        "--list-file".to_string(),
        "-o".to_string(),
        "build/blink.hex".to_string(),
        "examples/pic16f628a/blink.c".to_string(),
    ])
    .expect("parse cli");

    let CliCommand::Compile(command) = options.command else {
        panic!("expected compile command");
    };
    assert_eq!(command.target, "pic16f628a");
    assert_eq!(command.output, PathBuf::from("build/blink.hex"));
    assert!(command.artifacts.emit_ast);
    assert!(command.artifacts.emit_ir);
    assert!(command.artifacts.emit_asm);
    assert!(command.artifacts.map);
    assert!(command.artifacts.list_file);
}
