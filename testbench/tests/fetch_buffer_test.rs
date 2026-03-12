use riscv_core::instruction::{addi, c_addi};
use riscv_core::{create_fetch_buffer_runtime, FetchBuffer};

fn create_runtime() -> riscv_core::VerilatorRuntime {
    create_fetch_buffer_runtime().expect("Failed to create FetchBuffer runtime")
}

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

fn reset_dut(dut: &mut FetchBuffer) {
    dut.clk = 0;
    dut.rst_n = 0;
    dut.imem_data = 0;
    dut.imem_ready = 0;
    dut.pc = 0;
    dut.ir_write = 0;
    dut.pc_write = 0;
    dut.is_branch = 0;
    dut.is_writeback = 0;
    dut.eval();
    clock_cycle!(dut);

    dut.rst_n = 1;
    dut.eval();
}

#[test]
fn test_fetch_buffer_decompresses_compressed_instruction() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<FetchBuffer>().unwrap();

    reset_dut(&mut dut);

    let compressed = c_addi(10, 5);
    let expected = addi(10, 10, 5);

    dut.imem_data = compressed as u32;
    dut.imem_ready = 1;
    dut.ir_write = 1;
    dut.pc = 0;
    dut.eval();

    assert_eq!(dut.decomp_output, expected);
    assert_eq!(dut.decomp_is_valid, 1);

    clock_cycle!(dut);
    dut.ir_write = 0;
    dut.imem_ready = 0;
    dut.eval();

    assert_eq!(dut.current_insn_compressed, 1);
    assert_eq!(dut.pc_increment, 2);
}

#[test]
fn test_fetch_buffer_passes_through_32bit_instruction() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<FetchBuffer>().unwrap();

    reset_dut(&mut dut);

    let instruction = addi(11, 0, 42);

    dut.imem_data = instruction;
    dut.imem_ready = 1;
    dut.ir_write = 1;
    dut.pc = 0;
    dut.eval();

    assert_eq!(dut.decomp_output, instruction);
    assert_eq!(dut.decomp_is_valid, 1);

    clock_cycle!(dut);
    dut.ir_write = 0;
    dut.imem_ready = 0;
    dut.eval();

    assert_eq!(dut.current_insn_compressed, 0);
    assert_eq!(dut.pc_increment, 4);
}
