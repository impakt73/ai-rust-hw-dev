#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;

/// Panic handler for bare metal - infinite loop on panic
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

/// Entry point for the bare metal Rust program
/// This function implements the same test logic as test.s
#[no_mangle]
#[link_section = ".text"]
pub extern "C" fn _start() -> ! {
    unsafe {
        // Tohost address for signaling completion
        const TOHOST_ADDR: u32 = 0xFFFF_FFF0;
        const SUCCESS_CODE: u32 = 42;
        const FAILURE_CODE: u32 = 1;

        // ====== Test 1: Arithmetic ALU Operations ======
        let x1: u32;
        let x2: u32;
        let x3: u32;
        let x4: u32;
        let x5: u32;

        asm!(
            "addi {0}, x0, 10",      // x1 = 10
            "addi {1}, x0, 20",      // x2 = 20
            "add {2}, {0}, {1}",     // x3 = 10 + 20 = 30
            "sub {3}, {1}, {0}",     // x4 = 20 - 10 = 10
            "addi {4}, {0}, 5",      // x5 = 10 + 5 = 15
            out(reg) x1,
            out(reg) x2,
            out(reg) x3,
            out(reg) x4,
            out(reg) x5,
        );

        // Verify arithmetic operations
        if x1 != 10 || x2 != 20 || x3 != 30 || x4 != 10 || x5 != 15 {
            write_tohost(TOHOST_ADDR, FAILURE_CODE);
        }

        // ====== Test 2: Logical ALU Operations ======
        let x6: u32;
        let x7: u32;
        let x8: u32;
        let x9: u32;
        let x10: u32;
        let x11: u32;

        asm!(
            "and {2}, {0}, {1}",     // x6 = 10 & 20 = 0
            "or {3}, {0}, {1}",      // x7 = 10 | 20 = 30
            "xor {4}, {0}, {1}",     // x8 = 10 ^ 20 = 30
            "andi {5}, {0}, 15",     // x9 = 10 & 15 = 10
            "ori {6}, {0}, 5",       // x10 = 10 | 5 = 15
            "xori {7}, {0}, 7",      // x11 = 10 ^ 7 = 13
            in(reg) x1,
            in(reg) x2,
            out(reg) x6,
            out(reg) x7,
            out(reg) x8,
            out(reg) x9,
            out(reg) x10,
            out(reg) x11,
        );

        if x6 != 0 || x7 != 30 || x8 != 30 || x9 != 10 || x10 != 15 || x11 != 13 {
            write_tohost(TOHOST_ADDR, FAILURE_CODE);
        }

        // ====== Test 3: Shift Operations ======
        let x12: u32;
        let x13: u32;
        let x14: u32;
        let x15: u32;
        let x16: u32;

        asm!(
            "addi {0}, x0, 8",       // x12 = 8
            "slli {1}, {0}, 2",      // x13 = 8 << 2 = 32
            "srli {2}, {1}, 1",      // x14 = 32 >> 1 = 16
            "addi {3}, x0, -8",      // x15 = -8 (0xFFFFFFF8)
            "srai {4}, {3}, 1",      // x16 = -8 >>> 1 = -4 (0xFFFFFFFC)
            out(reg) x12,
            out(reg) x13,
            out(reg) x14,
            out(reg) x15,
            out(reg) x16,
        );

        if x12 != 8 || x13 != 32 || x14 != 16 || x15 != 0xFFFF_FFF8 || x16 != 0xFFFF_FFFC {
            write_tohost(TOHOST_ADDR, FAILURE_CODE);
        }

        // ====== Test 4: Comparison Operations ======
        let _x17: u32;
        let _x18: u32;
        let x19: u32;
        let x20: u32;
        let x21: u32;

        asm!(
            "addi {0}, x0, 5",       // x17 = 5
            "addi {1}, x0, 10",      // x18 = 10
            "slt {2}, {0}, {1}",     // x19 = 1 (5 < 10)
            "slti {3}, {0}, 3",      // x20 = 0 (5 < 3 is false)
            "sltu {4}, {0}, {1}",    // x21 = 1 (5 < 10 unsigned)
            out(reg) _x17,
            out(reg) _x18,
            out(reg) x19,
            out(reg) x20,
            out(reg) x21,
        );

        if x19 != 1 || x20 != 0 || x21 != 1 {
            write_tohost(TOHOST_ADDR, FAILURE_CODE);
        }

        // ====== Test 5: Memory Store and Load Verification ======
        let _base_addr: u32 = 0x8000_1000;
        let val1: u32 = 100;
        let val2: u32 = 200;
        let val3: u32 = 300;
        let loaded1: u32;
        let loaded2: u32;
        let loaded3: u32;

        asm!(
            "lui {0}, 0x80001",      // base = 0x80001000
            "sw {1}, 0({0})",        // mem[0x80001000] = 100
            "sw {2}, 4({0})",        // mem[0x80001004] = 200
            "sw {3}, 8({0})",        // mem[0x80001008] = 300
            "lw {4}, 0({0})",        // loaded1 = mem[0x80001000]
            "lw {5}, 4({0})",        // loaded2 = mem[0x80001004]
            "lw {6}, 8({0})",        // loaded3 = mem[0x80001008]
            out(reg) _,
            in(reg) val1,
            in(reg) val2,
            in(reg) val3,
            out(reg) loaded1,
            out(reg) loaded2,
            out(reg) loaded3,
        );

        if loaded1 != val1 || loaded2 != val2 || loaded3 != val3 {
            write_tohost(TOHOST_ADDR, FAILURE_CODE);
        }

        // ====== Test 6: Loop with Constant Counter ======
        let mut accumulator: u32 = 0;
        let mut counter: u32 = 5;

        #[allow(unused_assignments)]
        {
            asm!(
                "2:",  // const_loop label
                "addi {0}, {0}, 1",   // accumulator++
                "addi {1}, {1}, -1",  // counter--
                "bne {1}, x0, 2b",    // Continue if counter != 0
                inout(reg) accumulator,
                inout(reg) counter,
            );
        }

        if accumulator != 5 {
            write_tohost(TOHOST_ADDR, FAILURE_CODE);
        }

        // ====== Test 7: Upper Immediate Operations ======
        let x21_test: u32;
        let _x22_test: u32;

        asm!(
            "lui {0}, 0x12345",      // x21 = 0x12345000
            "addi {0}, {0}, 0x678",  // x21 = 0x12345678
            "auipc {1}, 0",          // x22 = PC + 0
            out(reg) x21_test,
            out(reg) _x22_test,
        );

        if x21_test != 0x1234_5678 {
            write_tohost(TOHOST_ADDR, FAILURE_CODE);
        }

        // ====== All Tests Passed ======
        write_tohost(TOHOST_ADDR, SUCCESS_CODE);
    }
}

/// Helper function to write to tohost address and halt
#[inline(never)]
unsafe fn write_tohost(_addr: u32, value: u32) -> ! {
    asm!(
        "lui t0, 0x0",
        "addi t0, t0, -16",  // t0 = 0xFFFFFFF0 (tohost address)
        "sw {0}, 0(t0)",     // Store value to tohost
        "2:",                // halt loop
        "j 2b",
        in(reg) value,
        options(noreturn)
    );
}
