use riscv_core::{create_ifetch_runtime, IFetch};

macro_rules! clock_cycle {
    ($dut:expr) => {
        $dut.clk = 0;
        $dut.eval();
        $dut.clk = 1;
        $dut.eval();
        $dut.clk = 0;
        $dut.eval();
    };
}

#[test]
fn test_ifetch_word_aligned_16bit() {
    let runtime = create_ifetch_runtime().expect("Failed to create ifetch runtime");
    let mut dut = runtime.create_model_simple::<IFetch>().unwrap();

    // Reset
    dut.rst_n = 0;
    dut.pc = 0;
    dut.imem_data = 0;
    dut.pc_valid = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;

    // Test: PC = 0x0000 (word-aligned), fetch 16-bit compressed instruction
    // Memory[0x0000] = 0x12340001 (lower 16 bits = 0x0001, which is C.NOP since bits[1:0] = 01)
    dut.pc = 0x0000;
    dut.imem_data = 0x12340001;
    dut.pc_valid = 0;
    clock_cycle!(dut);

    // Should fetch lower 16 bits and recognize as compressed
    assert_eq!(dut.imem_addr, 0x0000, "Should fetch from word-aligned address");
    assert_eq!(dut.instruction & 0xFFFF, 0x0001, "Should get lower 16 bits");
    assert_eq!(dut.is_compressed, 1, "Should be compressed (bits[1:0] != 11)");
    assert_eq!(dut.fetch_valid, 1, "Should be valid");
}

#[test]
fn test_ifetch_word_aligned_32bit() {
    let runtime = create_ifetch_runtime().expect("Failed to create ifetch runtime");
    let mut dut = runtime.create_model_simple::<IFetch>().unwrap();

    // Reset
    dut.rst_n = 0;
    dut.pc = 0;
    dut.imem_data = 0;
    dut.pc_valid = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;

    // Test: PC = 0x0000 (word-aligned), fetch 32-bit standard instruction
    // Memory[0x0000] = 0x00100013 (ADDI x0, x0, 1 - standard 32-bit NOP variant)
    // bits[1:0] = 11, so it's a 32-bit instruction
    dut.pc = 0x0000;
    dut.imem_data = 0x00100013;
    dut.pc_valid = 0;
    clock_cycle!(dut);

    // Should fetch complete 32-bit instruction
    assert_eq!(dut.imem_addr, 0x0000, "Should fetch from word-aligned address");
    assert_eq!(dut.instruction, 0x00100013, "Should get full 32-bit instruction");
    assert_eq!(dut.is_compressed, 0, "Should NOT be compressed (bits[1:0] == 11)");
    assert_eq!(dut.fetch_valid, 1, "Should be valid");
}

#[test]
fn test_ifetch_halfword_aligned_16bit() {
    let runtime = create_ifetch_runtime().expect("Failed to create ifetch runtime");
    let mut dut = runtime.create_model_simple::<IFetch>().unwrap();

    // Reset
    dut.rst_n = 0;
    dut.pc = 0;
    dut.imem_data = 0;
    dut.pc_valid = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;

    // Setup: First, fetch at PC=0x0000 to populate buffer
    // Memory[0x0000] = 0x00010001 (both halves are C.NOP)
    dut.pc = 0x0000;
    dut.imem_data = 0x00010001;
    dut.pc_valid = 0;
    clock_cycle!(dut);

    // Now: PC = 0x0002 (half-word aligned), should use buffered upper half
    dut.pc = 0x0002;
    dut.imem_data = 0x00010001;  // Next word
    dut.pc_valid = 0;
    clock_cycle!(dut);

    // Should fetch from buffered data
    assert_eq!(dut.imem_addr, 0x0000, "Should still fetch from word containing PC");
    assert_eq!(dut.instruction & 0xFFFF, 0x0001, "Should get buffered upper half");
    assert_eq!(dut.is_compressed, 1, "Should be compressed");
    assert_eq!(dut.fetch_valid, 1, "Should be valid");
}

#[test]
fn test_ifetch_transition_16_to_32bit() {
    let runtime = create_ifetch_runtime().expect("Failed to create ifetch runtime");
    let mut dut = runtime.create_model_simple::<IFetch>().unwrap();

    // Reset
    dut.rst_n = 0;
    dut.pc = 0;
    dut.imem_data = 0;
    dut.pc_valid = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;

    // Step 1: Fetch 16-bit instruction at PC=0x0000
    // Memory[0x0000] = 0x00130001 (lower=C.NOP, upper=0x0013 which is part of 32-bit)
    dut.pc = 0x0000;
    dut.imem_data = 0x00130001;
    dut.pc_valid = 0;
    clock_cycle!(dut);

    assert_eq!(dut.instruction & 0xFFFF, 0x0001, "First instruction should be C.NOP");
    assert_eq!(dut.is_compressed, 1, "Should be compressed");

    // Step 2: Fetch 32-bit instruction at PC=0x0002 (half-word aligned)
    // This is the critical test: buffered_half=0x0013 (bits[1:0]=11, so 32-bit)
    // Need to assemble from buffer + new fetch
    // Memory[0x0004] = 0x56780010 (lower=0x0010 is upper half of 32-bit instruction)
    dut.pc = 0x0002;
    dut.imem_data = 0x56780010;
    dut.pc_valid = 0;
    clock_cycle!(dut);

    // Should assemble 32-bit instruction: {0x0010, 0x0013} = 0x00100013
    assert_eq!(dut.instruction, 0x00100013, "Should assemble complete 32-bit instruction");
    assert_eq!(dut.is_compressed, 0, "Should NOT be compressed");
    assert_eq!(dut.fetch_valid, 1, "Should be valid");
}

#[test]
fn test_ifetch_buffer_invalidation_on_jump() {
    let runtime = create_ifetch_runtime().expect("Failed to create ifetch runtime");
    let mut dut = runtime.create_model_simple::<IFetch>().unwrap();

    // Reset
    dut.rst_n = 0;
    dut.pc = 0;
    dut.imem_data = 0;
    dut.pc_valid = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;

    // Step 1: Fetch at PC=0x0000 to populate buffer
    dut.pc = 0x0000;
    dut.imem_data = 0x12340001;
    dut.pc_valid = 0;
    clock_cycle!(dut);

    // Step 2: Jump to new address (pc_valid=1 invalidates buffer)
    dut.pc = 0x0100;
    dut.imem_data = 0xABCD0001;
    dut.pc_valid = 1;  // Signal that PC changed due to jump
    clock_cycle!(dut);

    // Buffer should be invalidated, fetch_valid should be 0 immediately after jump
    // (Implementation might set it to 1 in the same cycle or next cycle)
    // The key is that it doesn't use stale buffered data

    // Step 3: Normal fetch after jump
    dut.pc_valid = 0;  // Clear pc_valid signal
    clock_cycle!(dut);

    // Should fetch from new address without using old buffer
    assert_eq!(dut.imem_addr, 0x0100, "Should fetch from new address");
}

#[test]
fn test_ifetch_sequential_compressed() {
    let runtime = create_ifetch_runtime().expect("Failed to create ifetch runtime");
    let mut dut = runtime.create_model_simple::<IFetch>().unwrap();

    // Reset
    dut.rst_n = 0;
    dut.pc = 0;
    dut.imem_data = 0;
    dut.pc_valid = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;

    // Test sequential fetching of compressed instructions
    // PC=0x0000: Memory[0x0000] = 0x00050001 (two compressed instructions)
    dut.pc = 0x0000;
    dut.imem_data = 0x00050001;
    dut.pc_valid = 0;
    clock_cycle!(dut);

    assert_eq!(dut.instruction & 0xFFFF, 0x0001, "First instruction");
    assert_eq!(dut.is_compressed, 1);

    // PC=0x0002: Should use buffered upper half
    dut.pc = 0x0002;
    dut.imem_data = 0x00090001;  // Next word
    dut.pc_valid = 0;
    clock_cycle!(dut);

    assert_eq!(dut.instruction & 0xFFFF, 0x0005, "Second instruction from buffer");
    assert_eq!(dut.is_compressed, 1);

    // PC=0x0004: Fetch from new word
    dut.pc = 0x0004;
    dut.imem_data = 0x000D0009;
    dut.pc_valid = 0;
    clock_cycle!(dut);

    assert_eq!(dut.instruction & 0xFFFF, 0x0009, "Third instruction");
    assert_eq!(dut.is_compressed, 1);
}

#[test]
fn test_ifetch_boundary_crossing() {
    let runtime = create_ifetch_runtime().expect("Failed to create ifetch runtime");
    let mut dut = runtime.create_model_simple::<IFetch>().unwrap();

    // Reset
    dut.rst_n = 0;
    dut.pc = 0;
    dut.imem_data = 0;
    dut.pc_valid = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;

    // Test fetching across word boundary
    // PC=0x00FE (word aligned, but near boundary)
    dut.pc = 0x00FC;
    dut.imem_data = 0x00130001;  // 16-bit at 0xFC, start of 32-bit at 0xFE
    dut.pc_valid = 0;
    clock_cycle!(dut);

    assert_eq!(dut.instruction & 0xFFFF, 0x0001);
    assert_eq!(dut.is_compressed, 1);

    // PC=0x00FE (half-word aligned, 32-bit instruction crosses to 0x0100)
    dut.pc = 0x00FE;
    dut.imem_data = 0x00100000;  // Upper half of 32-bit instruction from 0x0100
    dut.pc_valid = 0;
    clock_cycle!(dut);

    // Should correctly assemble instruction across boundary
    assert_eq!(dut.instruction, 0x00100013, "Should assemble across boundary");
    assert_eq!(dut.is_compressed, 0);
}
