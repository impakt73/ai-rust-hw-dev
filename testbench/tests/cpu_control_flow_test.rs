use riscv_core::instruction::{
    addi, beq, c_ebreak, c_jal, csrrs, csrrsi, csrrw, ecall, jal, jalr, mret, slli, wfi,
};
use riscv_core::{create_cpu_runtime, Cpu};

use riscv_core::AsDynamicVerilatedModel;
const S_FETCH: u8 = 0x1;
const S_DECODE: u8 = 0x2;
const S_EXECUTE: u8 = 0x3;
const S_BRANCH: u8 = 0x8;
const S_REG_READ: u8 = 0xC;
const S_REG_READ_WAIT: u8 = 0xD;
const S_DECODE_WAIT: u8 = 0xE;
const WORD_BYTES: usize = 4;
const CSR_MSTATUS: u32 = 0x300;
const CSR_MIE: u32 = 0x304;
const CSR_MTVEC: u32 = 0x305;
const CSR_MEPC: u32 = 0x341;
const CSR_MCAUSE: u32 = 0x342;
const MSTATUS_MIE_ZIMM: u32 = 1 << 3;
const INTERRUPT_CAUSE_MEI: u32 = 0x8000_000B;

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
    dut.rst = 1;
    dut.boot = 0;
    dut.req_halt = 0;
    dut.msip = 0;
    dut.mtip = 0;
    dut.meip = 0;
    dut.mem_a_ready = 1;
    dut.mem_d_valid = 0;
    dut.mem_d_rdata = 0;
    dut.eval();
    clock_cycle!(dut);
    clock_cycle!(dut);

    dut.rst = 0;
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
fn test_cpu_masks_x0_rs1_reads_after_attempted_x0_write() {
    let runtime = create_cpu_runtime().expect("Failed to create CPU runtime");
    let mut dut = runtime
        .create_model_simple::<Cpu>()
        .expect("Failed to create CPU model");

    let mut program = vec![0_u8; 12];
    write_u32(&mut program, 0, addi(0, 0, 5));
    write_u32(&mut program, 0x4, addi(1, 0, 7));

    reset_to_fetch(&mut dut);

    let mut pending_response = None;
    wait_for_instr_complete(&mut dut, &program, &mut pending_response);
    wait_for_instr_complete(&mut dut, &program, &mut pending_response);

    assert_eq!(
        dut.debug_instruction,
        addi(1, 0, 7),
        "Second instruction should read x0 after an attempted write to x0"
    );
    assert_eq!(
        dut.debug_pc, 4,
        "Second instruction should complete at PC=4"
    );
    assert_eq!(
        dut.debug_rs1_data, 0,
        "CPU must mux x0 reads to zero even if the backing regfile storage changed"
    );
    assert_eq!(
        dut.debug_rd_data, 7,
        "Reading x0 after an attempted x0 write must still produce the immediate result"
    );
    assert_eq!(
        dut.debug_current_pc, 8,
        "CPU should advance past the x0 readback instruction"
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
fn test_cpu_fetch_stages_d_channel_response_before_decode() {
    let runtime = create_cpu_runtime().expect("Failed to create CPU runtime");
    let mut dut = runtime
        .create_model_simple::<Cpu>()
        .expect("Failed to create CPU model");

    let mut program = vec![0_u8; 8];
    write_u32(&mut program, 0x0, addi(1, 0, 42));

    reset_to_fetch(&mut dut);
    assert_eq!(dut.debug_fsm_state, S_FETCH, "CPU should boot into FETCH");

    let mut pending_response = None;

    step_with_memory(&mut dut, &program, &mut pending_response);
    assert_eq!(
        dut.debug_fsm_state, S_FETCH,
        "Fetch request acceptance should keep the CPU in FETCH while waiting for the response"
    );
    assert!(
        pending_response.is_some(),
        "Instruction fetch should queue a response after the address handshake"
    );

    step_with_memory(&mut dut, &program, &mut pending_response);
    assert_eq!(
        dut.debug_fsm_state, S_FETCH,
        "The D-channel handshake should only populate the staging register and keep the CPU in FETCH"
    );
    assert!(
        pending_response.is_none(),
        "The pending response should be consumed once the D-channel handshake completes"
    );

    dut.mem_a_ready = 1;
    dut.mem_d_valid = 0;
    dut.mem_d_rdata = 0;
    dut.eval();
    assert_eq!(
        dut.mem_a_valid, 0,
        "The CPU must not launch another instruction fetch while the staged response is waiting to be consumed"
    );

    clock_cycle!(dut);

    assert_eq!(
        dut.debug_fsm_state, S_DECODE,
        "After consuming the staged fetch response, the CPU should advance to DECODE"
    );
    assert_eq!(
        dut.debug_current_instruction,
        addi(1, 0, 42),
        "The staged fetch response should populate the instruction register before decode"
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

    for _ in 0..10 {
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
        dut.debug_rs1_data, 1,
        "Branch compare should observe the previously written x1 value on rs1"
    );
    assert_eq!(
        dut.debug_rs2_data, 0,
        "CPU must mux rs2 reads from x0 to zero for branch comparisons"
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

#[test]
fn test_cpu_ecall_redirects_to_mtvec_without_retiring() {
    let runtime = create_cpu_runtime().expect("Failed to create CPU runtime");
    let mut dut = runtime
        .create_model_simple::<Cpu>()
        .expect("Failed to create CPU model");

    let mut program = vec![0_u8; 20];
    write_u32(&mut program, 0x0, addi(5, 0, 16));
    write_u32(&mut program, 0x4, csrrw(0, 5, 0x305));
    write_u32(&mut program, 0x8, ecall());
    write_u32(&mut program, 0xc, addi(6, 0, 99));
    write_u32(&mut program, 0x10, addi(7, 0, 42));

    reset_to_fetch(&mut dut);

    let mut pending_response = None;
    wait_for_instr_complete(&mut dut, &program, &mut pending_response);
    wait_for_instr_complete(&mut dut, &program, &mut pending_response);

    for _ in 0..16 {
        step_with_memory(&mut dut, &program, &mut pending_response);
        if dut.debug_current_pc == 16 {
            break;
        }
    }

    assert_eq!(
        dut.halted, 0,
        "ECALL should enter the trap path instead of HALT"
    );
    assert_eq!(
        dut.debug_current_pc, 16,
        "ECALL should redirect fetch to mtvec immediately"
    );
    assert_eq!(
        dut.instr_complete, 0,
        "The trapping ECALL instruction must not retire"
    );
    assert_eq!(
        dut.debug_instruction,
        csrrw(0, 5, 0x305),
        "ECALL must not overwrite the last retired instruction trace entry"
    );

    wait_for_instr_complete(&mut dut, &program, &mut pending_response);
    assert_eq!(
        dut.debug_instruction,
        addi(7, 0, 42),
        "Execution should continue from the mtvec handler after trap redirect"
    );
}

#[test]
fn test_cpu_mret_redirects_to_masked_mepc_without_halting() {
    let runtime = create_cpu_runtime().expect("Failed to create CPU runtime");
    let mut dut = runtime
        .create_model_simple::<Cpu>()
        .expect("Failed to create CPU model");

    let mut program = vec![0_u8; 20];
    write_u32(&mut program, 0x0, addi(5, 0, 13));
    write_u32(&mut program, 0x4, csrrw(0, 5, 0x341));
    write_u32(&mut program, 0x8, mret());
    write_u32(&mut program, 0xc, addi(6, 0, 99));
    write_u32(&mut program, 0x10, addi(7, 0, 42));

    reset_to_fetch(&mut dut);

    let mut pending_response = None;
    wait_for_instr_complete(&mut dut, &program, &mut pending_response);
    wait_for_instr_complete(&mut dut, &program, &mut pending_response);
    wait_for_instr_complete(&mut dut, &program, &mut pending_response);

    assert_eq!(
        dut.debug_instruction,
        mret(),
        "Completed instruction should be MRET rather than an ECALL alias"
    );
    assert_eq!(dut.halted, 0, "MRET decode must not send the CPU to HALT");
    assert_eq!(
        dut.debug_current_pc, 12,
        "MRET should restore PC from mepc and mask off the compressed-alignment low bit"
    );

    wait_for_instr_complete(&mut dut, &program, &mut pending_response);
    assert_eq!(
        dut.debug_instruction,
        addi(6, 0, 99),
        "CPU should continue executing from the restored mepc target after MRET"
    );
}

#[test]
fn test_cpu_pending_disabled_interrupt_does_not_trap() {
    let runtime = create_cpu_runtime().expect("Failed to create CPU runtime");
    let mut dut = runtime
        .create_model_simple::<Cpu>()
        .expect("Failed to create CPU model");

    let mut program = vec![0_u8; 40];
    write_u32(&mut program, 0x0, addi(5, 0, 32));
    write_u32(&mut program, 0x4, csrrw(0, 5, CSR_MTVEC));
    write_u32(&mut program, 0x8, addi(6, 0, 11));
    write_u32(&mut program, 0xc, addi(7, 0, 22));
    write_u32(&mut program, 0x20, addi(8, 0, 99));

    reset_to_fetch(&mut dut);

    let mut pending_response = None;
    wait_for_instr_complete(&mut dut, &program, &mut pending_response);
    wait_for_instr_complete(&mut dut, &program, &mut pending_response);
    dut.meip = 1;
    wait_for_instr_complete(&mut dut, &program, &mut pending_response);

    assert_eq!(
        dut.debug_instruction,
        addi(6, 0, 11),
        "Disabled pending MEIP must not redirect control flow"
    );
    assert_eq!(dut.halted, 0, "Disabled interrupt must not halt the CPU");
    assert_eq!(
        dut.debug_current_pc, 12,
        "CPU should continue to the next sequential instruction when MEIP is disabled"
    );
}

#[test]
fn test_cpu_enabled_meip_traps_updates_csrs_and_mret_resumes() {
    let runtime = create_cpu_runtime().expect("Failed to create CPU runtime");
    let mut dut = runtime
        .create_model_simple::<Cpu>()
        .expect("Failed to create CPU model");

    let mut program = vec![0_u8; 64];
    write_u32(&mut program, 0x0, addi(5, 0, 32));
    write_u32(&mut program, 0x4, csrrw(0, 5, CSR_MTVEC));
    write_u32(&mut program, 0x8, addi(6, 0, 1));
    write_u32(&mut program, 0xc, slli(6, 6, 11));
    write_u32(&mut program, 0x10, csrrw(0, 6, CSR_MIE));
    write_u32(&mut program, 0x14, csrrsi(0, MSTATUS_MIE_ZIMM, CSR_MSTATUS));
    write_u32(&mut program, 0x18, addi(7, 0, 55));
    write_u32(&mut program, 0x1c, addi(8, 0, 66));
    write_u32(&mut program, 0x20, csrrs(10, 0, CSR_MEPC));
    write_u32(&mut program, 0x24, csrrs(11, 0, CSR_MCAUSE));
    write_u32(&mut program, 0x28, csrrs(12, 0, CSR_MSTATUS));
    write_u32(&mut program, 0x2c, mret());
    write_u32(&mut program, 0x30, addi(13, 0, 77));

    reset_to_fetch(&mut dut);

    let mut pending_response = None;
    for _ in 0..6 {
        wait_for_instr_complete(&mut dut, &program, &mut pending_response);
    }

    dut.meip = 1;
    for _ in 0..16 {
        step_with_memory(&mut dut, &program, &mut pending_response);
        if dut.debug_current_pc == 0x20 {
            break;
        }
    }

    assert_eq!(dut.halted, 0, "MEIP trap must not halt the CPU");
    assert_eq!(
        dut.debug_instruction,
        csrrsi(0, MSTATUS_MIE_ZIMM, CSR_MSTATUS),
        "Interrupt trap must occur between retired instructions"
    );
    assert_eq!(
        dut.debug_current_pc, 0x20,
        "Enabled MEIP must redirect fetch to mtvec"
    );

    wait_for_instr_complete(&mut dut, &program, &mut pending_response);
    assert_eq!(dut.debug_instruction, csrrs(10, 0, CSR_MEPC));
    assert_eq!(
        dut.debug_rd_data, 0x18,
        "MEPC must capture the interrupted resume PC"
    );

    wait_for_instr_complete(&mut dut, &program, &mut pending_response);
    assert_eq!(dut.debug_instruction, csrrs(11, 0, CSR_MCAUSE));
    assert_eq!(
        dut.debug_rd_data, INTERRUPT_CAUSE_MEI,
        "MCAUSE must report machine external interrupt"
    );

    wait_for_instr_complete(&mut dut, &program, &mut pending_response);
    assert_eq!(dut.debug_instruction, csrrs(12, 0, CSR_MSTATUS));
    assert_eq!(
        dut.debug_rd_data, 0x80,
        "Trap entry must move MIE into MPIE and clear MIE"
    );

    dut.meip = 0;
    wait_for_instr_complete(&mut dut, &program, &mut pending_response);
    assert_eq!(dut.debug_instruction, mret());
    assert_eq!(
        dut.debug_current_pc, 0x18,
        "MRET must restore the interrupted PC"
    );

    wait_for_instr_complete(&mut dut, &program, &mut pending_response);
    assert_eq!(
        dut.debug_instruction,
        addi(7, 0, 55),
        "Execution must resume at the interrupted instruction after MRET"
    );
}

#[test]
fn test_cpu_interrupt_priority_prefers_meip_over_msip_and_mtip() {
    let runtime = create_cpu_runtime().expect("Failed to create CPU runtime");
    let mut dut = runtime
        .create_model_simple::<Cpu>()
        .expect("Failed to create CPU model");

    let mut program = vec![0_u8; 48];
    write_u32(&mut program, 0x0, addi(5, 0, 32));
    write_u32(&mut program, 0x4, csrrw(0, 5, CSR_MTVEC));
    write_u32(&mut program, 0x8, addi(6, 0, 17));
    write_u32(&mut program, 0xc, slli(6, 6, 7));
    write_u32(&mut program, 0x10, addi(6, 6, 8));
    write_u32(&mut program, 0x14, csrrw(0, 6, CSR_MIE));
    write_u32(&mut program, 0x18, csrrsi(0, MSTATUS_MIE_ZIMM, CSR_MSTATUS));
    write_u32(&mut program, 0x1c, addi(7, 0, 1));
    write_u32(&mut program, 0x20, csrrs(10, 0, CSR_MCAUSE));
    write_u32(&mut program, 0x24, mret());
    write_u32(&mut program, 0x28, addi(11, 0, 2));

    reset_to_fetch(&mut dut);

    let mut pending_response = None;
    for _ in 0..6 {
        wait_for_instr_complete(&mut dut, &program, &mut pending_response);
    }

    dut.msip = 1;
    dut.mtip = 1;
    dut.meip = 1;

    for _ in 0..16 {
        step_with_memory(&mut dut, &program, &mut pending_response);
        if dut.debug_current_pc == 0x20 {
            break;
        }
    }

    wait_for_instr_complete(&mut dut, &program, &mut pending_response);
    assert_eq!(dut.debug_instruction, csrrs(10, 0, CSR_MCAUSE));
    assert_eq!(
        dut.debug_rd_data, INTERRUPT_CAUSE_MEI,
        "Machine external interrupt must win the documented priority order"
    );

    dut.msip = 0;
    dut.mtip = 0;
    dut.meip = 0;
    wait_for_instr_complete(&mut dut, &program, &mut pending_response);
    assert_eq!(dut.debug_instruction, mret());
}

#[test]
fn test_cpu_wfi_sleeps_until_interrupt_arrives() {
    let runtime = create_cpu_runtime().expect("Failed to create CPU runtime");
    let mut dut = runtime
        .create_model_simple::<Cpu>()
        .expect("Failed to create CPU model");

    let mut program = vec![0_u8; 64];
    write_u32(&mut program, 0x0, addi(5, 0, 32));
    write_u32(&mut program, 0x4, csrrw(0, 5, CSR_MTVEC));
    write_u32(&mut program, 0x8, addi(6, 0, 1));
    write_u32(&mut program, 0xc, slli(6, 6, 11));
    write_u32(&mut program, 0x10, csrrw(0, 6, CSR_MIE));
    write_u32(&mut program, 0x14, csrrsi(0, MSTATUS_MIE_ZIMM, CSR_MSTATUS));
    write_u32(&mut program, 0x18, wfi());
    write_u32(&mut program, 0x1c, addi(7, 0, 11));
    write_u32(&mut program, 0x20, mret());

    reset_to_fetch(&mut dut);

    let mut pending_response = None;
    for _ in 0..7 {
        wait_for_instr_complete(&mut dut, &program, &mut pending_response);
    }

    assert_eq!(
        dut.debug_instruction,
        wfi(),
        "WFI must retire before sleeping"
    );
    assert_eq!(
        dut.debug_current_pc, 0x1c,
        "WFI must preserve the resume PC"
    );

    for _ in 0..6 {
        step_with_memory(&mut dut, &program, &mut pending_response);
        assert_eq!(
            dut.debug_fsm_state, S_FETCH,
            "WFI must hold the hart in FETCH"
        );
        assert_eq!(
            dut.mem_a_valid, 0,
            "WFI sleep must stop issuing fetch requests"
        );
        assert_eq!(
            dut.debug_current_pc, 0x1c,
            "WFI must hold the next-PC stable while sleeping"
        );
    }

    dut.meip = 1;
    for _ in 0..16 {
        step_with_memory(&mut dut, &program, &mut pending_response);
        if dut.debug_current_pc == 0x20 {
            break;
        }
    }
    assert_eq!(
        dut.debug_current_pc, 0x20,
        "Interrupt must wake WFI and vector to mtvec"
    );

    dut.meip = 0;
    wait_for_instr_complete(&mut dut, &program, &mut pending_response);
    assert_eq!(dut.debug_instruction, mret());
    assert_eq!(
        dut.debug_current_pc, 0x1c,
        "MRET after WFI wake must restore the sleeping resume PC"
    );

    wait_for_instr_complete(&mut dut, &program, &mut pending_response);
    assert_eq!(
        dut.debug_instruction,
        addi(7, 0, 11),
        "CPU must resume with the post-WFI instruction after servicing the interrupt"
    );
}
