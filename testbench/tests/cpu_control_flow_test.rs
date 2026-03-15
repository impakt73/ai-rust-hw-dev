use riscv_core::instruction::{addi, beq, c_ebreak, c_jal, jal, jalr};
use riscv_core::{create_cpu_runtime, Cpu};

const S_FETCH: u8 = 0x1;
const S_DECODE: u8 = 0x2;
const S_EXECUTE: u8 = 0x3;
const S_BRANCH: u8 = 0x8;
const S_REG_READ: u8 = 0xC;
const S_REG_READ_WAIT: u8 = 0xD;
const S_DECODE_WAIT: u8 = 0xE;
const WORD_BYTES: usize = 4;

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

fn write_u16(program: &mut [u8], addr: usize, value: u16) {
    program[addr] = (value & 0x00ff) as u8;
    program[addr + 1] = (value >> 8) as u8;
}

fn write_u32(program: &mut [u8], addr: usize, value: u32) {
    for byte_index in 0..WORD_BYTES {
        program[addr + byte_index] = ((value >> (byte_index * 8)) & 0x0000_00ff) as u8;
    }
}

fn read_u32(program: &[u8], addr: u32) -> u32 {
    let addr = addr as usize;
    let mut value = 0_u32;

    for byte_index in 0..WORD_BYTES {
        let byte = program.get(addr + byte_index).copied().unwrap_or(0);
        value |= u32::from(byte) << (byte_index * 8);
    }

    value
}

fn reset_to_fetch(dut: &mut Cpu) {
    dut.rst_n = 0;
    dut.boot = 0;
    dut.req_halt = 0;
    dut.mem_a_ready = 1;
    dut.mem_d_valid = 0;
    dut.mem_d_rdata = 0;
    dut.eval();
    clock_cycle!(dut);
    clock_cycle!(dut);

    dut.rst_n = 1;
    dut.boot = 1;
    dut.eval();
    clock_cycle!(dut);

    dut.boot = 0;
    dut.eval();
}

fn step_with_memory(dut: &mut Cpu, program: &[u8], pending_response: &mut Option<u32>) {
    dut.mem_a_ready = if pending_response.is_some() { 0 } else { 1 };
    dut.mem_d_valid = if pending_response.is_some() { 1 } else { 0 };
    dut.mem_d_rdata = pending_response.unwrap_or(0);
    dut.eval();

    let d_handshake = pending_response.is_some() && dut.mem_d_ready != 0;
    let a_handshake = dut.mem_a_valid != 0 && dut.mem_a_ready != 0;

    if d_handshake {
        *pending_response = None;
    }

    if a_handshake && pending_response.is_none() {
        let response = if dut.mem_a_we != 0 {
            0
        } else {
            read_u32(program, dut.mem_a_addr)
        };
        *pending_response = Some(response);
    }

    clock_cycle!(dut);
}

fn wait_for_instr_complete(dut: &mut Cpu, program: &[u8], pending_response: &mut Option<u32>) {
    let mut instr_complete_seen = false;

    for _ in 0..32 {
        step_with_memory(dut, program, pending_response);

        if dut.instr_complete != 0 {
            instr_complete_seen = true;
        }

        if instr_complete_seen && pending_response.is_none() {
            return;
        }
    }

    panic!(
        "instruction did not complete; state={:#x} current_pc={:#x}",
        dut.debug_fsm_state, dut.debug_current_pc
    );
}

#[test]
fn test_cpu_branch_taken_redirects_with_unified_target_register() {
    let runtime = create_cpu_runtime().expect("Failed to create CPU runtime");
    let mut dut = runtime
        .create_model_simple::<Cpu>()
        .expect("Failed to create CPU model");

    let mut program = vec![0_u8; 16];
    write_u32(&mut program, 0x0, beq(0, 0, 8));
    write_u32(&mut program, 0x4, addi(2, 0, 99));
    write_u32(&mut program, 0x8, addi(3, 0, 7));

    reset_to_fetch(&mut dut);
    assert_eq!(dut.debug_fsm_state, S_FETCH, "CPU should boot into FETCH");

    let mut pending_response = None;
    wait_for_instr_complete(&mut dut, &program, &mut pending_response);

    assert_eq!(dut.debug_pc, 0, "Branch should complete at the branch PC");
    assert_eq!(
        dut.debug_instruction,
        beq(0, 0, 8),
        "Completed instruction should be the taken branch"
    );
    assert_eq!(
        dut.debug_current_pc, 8,
        "Taken branch should redirect the next fetch to the branch target"
    );
}

#[test]
fn test_cpu_branch_uses_execute_stage_before_branch_completion() {
    let runtime = create_cpu_runtime().expect("Failed to create CPU runtime");
    let mut dut = runtime
        .create_model_simple::<Cpu>()
        .expect("Failed to create CPU model");

    let mut program = vec![0_u8; 16];
    write_u32(&mut program, 0x0, beq(0, 0, 8));
    write_u32(&mut program, 0x4, addi(2, 0, 99));
    write_u32(&mut program, 0x8, addi(3, 0, 7));

    reset_to_fetch(&mut dut);

    let mut pending_response = None;
    let mut observed_states = Vec::new();

    for _ in 0..8 {
        step_with_memory(&mut dut, &program, &mut pending_response);
        observed_states.push(dut.debug_fsm_state);

        if dut.instr_complete != 0 {
            break;
        }
    }

    assert!(
        observed_states.windows(6).any(|window| {
            window
                == [
                    S_DECODE,
                    S_DECODE_WAIT,
                    S_REG_READ,
                    S_REG_READ_WAIT,
                    S_EXECUTE,
                    S_BRANCH,
                ]
        }),
        "branch should flow through DECODE -> DECODE_WAIT -> REG_READ -> REG_READ_WAIT -> EXECUTE -> BRANCH, observed {observed_states:?}"
    );
    assert_eq!(
        dut.debug_current_pc, 8,
        "taken branch should still redirect to the branch target after the extra execute cycle"
    );
}

#[test]
fn test_cpu_branch_not_taken_falls_through_after_registered_compare() {
    let runtime = create_cpu_runtime().expect("Failed to create CPU runtime");
    let mut dut = runtime
        .create_model_simple::<Cpu>()
        .expect("Failed to create CPU model");

    let mut program = vec![0_u8; 20];
    write_u32(&mut program, 0x0, addi(1, 0, 1));
    write_u32(&mut program, 0x4, beq(1, 0, 8));
    write_u32(&mut program, 0x8, addi(2, 0, 99));
    write_u32(&mut program, 0xc, addi(3, 0, 7));

    reset_to_fetch(&mut dut);

    let mut pending_response = None;
    wait_for_instr_complete(&mut dut, &program, &mut pending_response);
    wait_for_instr_complete(&mut dut, &program, &mut pending_response);

    assert_eq!(dut.debug_pc, 4, "Branch should complete at the branch PC");
    assert_eq!(
        dut.debug_instruction,
        beq(1, 0, 8),
        "Completed instruction should be the not-taken branch"
    );
    assert_eq!(
        dut.debug_current_pc, 8,
        "Not-taken branch should fall through to the next sequential instruction"
    );
}

#[test]
fn test_cpu_c_jal_writes_halfword_link_address() {
    let runtime = create_cpu_runtime().expect("Failed to create CPU runtime");
    let mut dut = runtime
        .create_model_simple::<Cpu>()
        .expect("Failed to create CPU model");

    let mut program = vec![0_u8; 16];
    write_u16(&mut program, 0x0, c_jal(4));
    write_u16(&mut program, 0x2, c_ebreak());
    write_u32(&mut program, 0x4, addi(2, 0, 1));

    reset_to_fetch(&mut dut);

    let mut pending_response = None;
    wait_for_instr_complete(&mut dut, &program, &mut pending_response);

    assert_eq!(dut.debug_pc, 0, "C.JAL should complete at PC 0");
    assert_eq!(
        dut.debug_instruction,
        jal(1, 4),
        "Fetch/decompress path should present C.JAL as JAL x1, 4"
    );
    assert_eq!(
        dut.debug_rd_data, 2,
        "Compressed JAL must write PC+2 as the link address"
    );
    assert_eq!(
        dut.debug_current_pc, 4,
        "Compressed JAL should redirect to the decoded target"
    );
}

#[test]
fn test_cpu_jalr_masks_target_and_uses_fallthrough_link_address() {
    let runtime = create_cpu_runtime().expect("Failed to create CPU runtime");
    let mut dut = runtime
        .create_model_simple::<Cpu>()
        .expect("Failed to create CPU model");

    let mut program = vec![0_u8; 20];
    write_u32(&mut program, 0x0, addi(5, 0, 13));
    write_u32(&mut program, 0x4, jalr(1, 5, 0));
    write_u32(&mut program, 0x8, addi(2, 0, 99));
    write_u32(&mut program, 0xc, addi(6, 0, 42));

    reset_to_fetch(&mut dut);

    let mut pending_response = None;
    wait_for_instr_complete(&mut dut, &program, &mut pending_response);
    wait_for_instr_complete(&mut dut, &program, &mut pending_response);

    assert_eq!(dut.debug_pc, 4, "JALR should complete at its own PC");
    assert_eq!(
        dut.debug_instruction,
        jalr(1, 5, 0),
        "Completed instruction should be the JALR under test"
    );
    assert_eq!(
        dut.debug_rd_data, 8,
        "JALR must write the sequential fall-through PC as the link address"
    );
    assert_eq!(
        dut.debug_current_pc, 12,
        "JALR target should be masked to an even address before redirect"
    );
}
