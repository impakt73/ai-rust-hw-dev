use riscv_core::{create_cpu_runtime, Cpu};

const S_FETCH: u8 = 0x1;
const S_HALT: u8 = 0xA;

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

fn reset_and_boot_to_fetch(dut: &mut Cpu) {
    dut.rst_n = 0;
    dut.boot = 0;
    dut.req_halt = 0;
    dut.mem_ready = 0;
    dut.mem_rdata = 0;
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

#[test]
fn test_cpu_req_halt_gates_fetch_and_enters_halt() {
    let runtime = create_cpu_runtime().expect("Failed to create CPU runtime");
    let mut dut = runtime
        .create_model_simple::<Cpu>()
        .expect("Failed to create CPU model");

    reset_and_boot_to_fetch(&mut dut);

    assert_eq!(dut.debug_fsm_state, S_FETCH, "CPU should be in FETCH state");

    dut.req_halt = 0;
    dut.mem_ready = 0;
    dut.eval();
    assert_eq!(dut.mem_req, 1, "FETCH should request instruction memory");

    dut.req_halt = 1;
    dut.eval();
    assert_eq!(
        dut.mem_req, 0,
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
