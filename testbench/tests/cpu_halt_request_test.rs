use riscv_core::instruction::{beq, lw};
use riscv_core::{create_cpu_runtime, Cpu};

const S_BOOT: u8 = 0x0; // Matches cpu.sv S_BOOT encoding.
const S_FETCH: u8 = 0x1;
const S_DECODE: u8 = 0x2;
const S_EXECUTE: u8 = 0x3;
const S_MEM_READ: u8 = 0x5;
const S_BRANCH: u8 = 0x8;
const S_HALT: u8 = 0xA;
const S_REG_READ: u8 = 0xC;
const S_REG_READ_WAIT: u8 = 0xD;

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

fn reset_to_boot(dut: &mut Cpu) {
    dut.rst_n = 0;
    dut.boot = 0;
    dut.boot_addr = 0;
    dut.req_halt = 0;
    dut.mem_a_ready = 0;
    dut.mem_d_valid = 0;
    dut.mem_d_rdata = 0;
    dut.eval();
    clock_cycle!(dut);
    clock_cycle!(dut);

    dut.rst_n = 1;
    dut.eval();
}

fn reset_and_boot_to_fetch(dut: &mut Cpu) {
    reset_to_boot(dut);

    dut.boot = 1;
    dut.eval();
    clock_cycle!(dut);
    dut.boot = 0;
    dut.eval();
}

fn fetch_instruction(dut: &mut Cpu, instruction: u32) {
    assert_eq!(
        dut.debug_fsm_state, S_FETCH,
        "CPU must be in FETCH before injecting an instruction response"
    );
    assert_eq!(
        dut.mem_a_valid, 1,
        "FETCH should drive an instruction request"
    );

    dut.mem_a_ready = 1;
    dut.mem_d_valid = 0;
    dut.eval();
    clock_cycle!(dut);

    dut.mem_a_ready = 0;
    dut.mem_d_rdata = instruction;
    dut.mem_d_valid = 1;
    dut.eval();
    clock_cycle!(dut);

    dut.mem_d_valid = 0;
    dut.mem_d_rdata = 0;
    dut.eval();
    assert_eq!(
        dut.debug_fsm_state, S_DECODE,
        "CPU should advance to DECODE after the instruction response"
    );
}

#[test]
fn test_cpu_req_halt_in_boot_enters_halt_before_fetch() {
    let runtime = create_cpu_runtime().expect("Failed to create CPU runtime");
    let mut dut = runtime
        .create_model_simple::<Cpu>()
        .expect("Failed to create CPU model");

    reset_to_boot(&mut dut);

    assert_eq!(
        dut.debug_fsm_state, S_BOOT,
        "CPU should reset into BOOT state"
    );
    assert_eq!(
        dut.mem_a_valid, 0,
        "BOOT state should not issue fetch requests"
    );

    dut.req_halt = 1;
    dut.eval();
    assert_eq!(
        dut.mem_a_valid, 0,
        "req_halt in BOOT should keep instruction fetch requests disabled"
    );

    clock_cycle!(dut);
    assert_eq!(
        dut.debug_fsm_state, S_HALT,
        "req_halt in BOOT should move CPU directly to HALT"
    );
    assert_eq!(
        dut.halted, 1,
        "CPU halted output should assert in HALT state"
    );
    assert_eq!(
        dut.mem_a_valid, 0,
        "CPU should still suppress fetch requests after halting from BOOT"
    );
}

#[test]
fn test_cpu_req_halt_gates_fetch_and_enters_halt() {
    let runtime = create_cpu_runtime().expect("Failed to create CPU runtime");
    let mut dut = runtime
        .create_model_simple::<Cpu>()
        .expect("Failed to create CPU model");

    reset_and_boot_to_fetch(&mut dut);

    assert_eq!(dut.debug_fsm_state, S_FETCH, "CPU should be in FETCH state");

    dut.req_halt = 0;
    dut.mem_d_valid = 0;
    dut.eval();
    assert_eq!(
        dut.mem_a_valid, 1,
        "FETCH should request instruction memory"
    );

    dut.req_halt = 1;
    dut.eval();
    assert_eq!(
        dut.mem_a_valid, 0,
        "req_halt should gate off instruction fetch memory requests"
    );

    clock_cycle!(dut);
    assert_eq!(
        dut.debug_fsm_state, S_HALT,
        "req_halt in FETCH should move CPU to HALT state"
    );
    assert_eq!(
        dut.halted, 1,
        "CPU halted output should assert in HALT state"
    );
    assert_eq!(
        dut.instr_complete, 0,
        "instr_complete should not assert while CPU is in HALT state"
    );

    clock_cycle!(dut);
    assert_eq!(
        dut.instr_complete, 0,
        "instr_complete must remain low while CPU stays halted"
    );
}

#[test]
fn test_cpu_branch_target_uses_execute_stage() {
    let runtime = create_cpu_runtime().expect("Failed to create CPU runtime");
    let mut dut = runtime
        .create_model_simple::<Cpu>()
        .expect("Failed to create CPU model");

    reset_and_boot_to_fetch(&mut dut);
    dut.mem_d_rdata = 0;
    dut.eval();

    fetch_instruction(&mut dut, beq(0, 0, 8));

    clock_cycle!(dut);
    assert_eq!(
        dut.debug_fsm_state, S_REG_READ,
        "branch should enter REG_READ"
    );

    clock_cycle!(dut);
    assert_eq!(
        dut.debug_fsm_state, S_REG_READ_WAIT,
        "branch should wait for register-file output"
    );

    clock_cycle!(dut);
    assert_eq!(
        dut.debug_fsm_state, S_EXECUTE,
        "branch target calculation should route through EXECUTE"
    );

    clock_cycle!(dut);
    assert_eq!(
        dut.debug_fsm_state, S_EXECUTE,
        "branch should remain in EXECUTE until the ALU target is ready"
    );

    clock_cycle!(dut);
    assert_eq!(
        dut.debug_fsm_state, S_EXECUTE,
        "branch should stay in EXECUTE until the registered ALU result is observed"
    );

    clock_cycle!(dut);
    assert_eq!(
        dut.debug_fsm_state, S_BRANCH,
        "branch should transition to BRANCH only after execute-stage target calculation"
    );

    clock_cycle!(dut);
    assert_eq!(
        dut.debug_fsm_state, S_FETCH,
        "branch should return to FETCH after BRANCH"
    );
    assert_eq!(
        dut.mem_a_addr, 8,
        "taken branch should update the fetch PC to the ALU-computed branch target"
    );
}

#[test]
fn test_cpu_load_address_uses_execute_stage() {
    let runtime = create_cpu_runtime().expect("Failed to create CPU runtime");
    let mut dut = runtime
        .create_model_simple::<Cpu>()
        .expect("Failed to create CPU model");

    reset_and_boot_to_fetch(&mut dut);
    dut.mem_d_rdata = 0;
    dut.eval();

    fetch_instruction(&mut dut, lw(1, 0, 32));

    clock_cycle!(dut);
    assert_eq!(
        dut.debug_fsm_state, S_REG_READ,
        "load should enter REG_READ"
    );

    clock_cycle!(dut);
    assert_eq!(
        dut.debug_fsm_state, S_REG_READ_WAIT,
        "load should wait for register-file output"
    );

    clock_cycle!(dut);
    assert_eq!(
        dut.debug_fsm_state, S_EXECUTE,
        "load address calculation should route through EXECUTE"
    );

    clock_cycle!(dut);
    assert_eq!(
        dut.debug_fsm_state, S_EXECUTE,
        "load should remain in EXECUTE until the ALU address is ready"
    );

    clock_cycle!(dut);
    assert_eq!(
        dut.debug_fsm_state, S_EXECUTE,
        "load should stay in EXECUTE until the registered ALU result is observed"
    );

    clock_cycle!(dut);
    assert_eq!(
        dut.debug_fsm_state, S_MEM_READ,
        "load should enter MEM_READ only after execute-stage address calculation"
    );
    assert_eq!(
        dut.mem_a_addr, 32,
        "MEM_READ should use the execute-stage ALU address result"
    );
}
