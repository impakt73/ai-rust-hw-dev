use riscv_core::{create_dsp_pipe_runtime, DspPipe};

use riscv_core::AsDynamicVerilatedModel;
const ALU_ADD: u32 = 0b00000;
const ALU_SUB: u32 = 0b00001;
const ALU_AND: u32 = 0b00010;
const ALU_OR: u32 = 0b00011;
const ALU_XOR: u32 = 0b00100;
const ALU_SLL: u32 = 0b00101;
const ALU_SRL: u32 = 0b00110;
const ALU_SRA: u32 = 0b00111;
const ALU_SLT: u32 = 0b01000;
const ALU_SLTU: u32 = 0b01001;
const ALU_MUL: u32 = 0b01010;
const ALU_MULH: u32 = 0b01011;
const ALU_MULHSU: u32 = 0b01100;
const ALU_MULHU: u32 = 0b01101;

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

fn expected_result(a: u32, b: u32, alu_op: u32) -> u32 {
    match alu_op {
        ALU_ADD => a.wrapping_add(b),
        ALU_SUB => a.wrapping_sub(b),
        ALU_AND => a & b,
        ALU_OR => a | b,
        ALU_XOR => a ^ b,
        ALU_SLL => a << (b & 0x1F),
        ALU_SRL => a >> (b & 0x1F),
        ALU_SRA => ((a as i32) >> (b & 0x1F)) as u32,
        ALU_SLT => u32::from((a as i32) < (b as i32)),
        ALU_SLTU => u32::from(a < b),
        ALU_MUL => a.wrapping_mul(b),
        ALU_MULH => (((a as i32 as i64) * (b as i32 as i64)) >> 32) as u32,
        ALU_MULHSU => (((a as i32 as i64) * (b as u64 as i64)) >> 32) as u32,
        ALU_MULHU => (((a as u64) * (b as u64)) >> 32) as u32,
        _ => 0,
    }
}

fn reset_dsp_pipe(dut: &mut DspPipe) {
    dut.rst = 1;
    dut.in_valid = 0;
    clock_cycle!(dut);
    dut.rst = 0;
    clock_cycle!(dut);
}

fn run_single_operation(dut: &mut DspPipe, a: u32, b: u32, alu_op: u8) -> u32 {
    reset_dsp_pipe(dut);

    dut.a = a;
    dut.b = b;
    dut.alu_op = alu_op;
    dut.in_valid = 1;
    clock_cycle!(dut);
    dut.in_valid = 0;

    for cycle in 0..4 {
        dut.eval();
        if dut.out_valid == 1 {
            return dut.out_data;
        }
        clock_cycle!(dut);
        if cycle == 0 {
            assert_eq!(
                dut.out_valid, 0,
                "DSP pipe should not produce a result after only one post-launch cycle"
            );
        }
    }

    panic!("Timed out waiting for DSP pipe result (op={alu_op:#04x}, a={a:#010x}, b={b:#010x})");
}

#[test]
fn test_dsp_pipe_supported_operations() {
    let runtime = create_dsp_pipe_runtime().expect("Failed to create DSP pipe runtime");
    let mut dut = testbench::create_testbench_model::<DspPipe>(&runtime).unwrap();

    for (a, b, alu_op) in [
        (0x1234_5678_u32, 0x0102_0304_u32, ALU_ADD),
        (0x1234_5678_u32, 0x0000_0100_u32, ALU_SUB),
        (0xF0F0_AA55_u32, 0x0FF0_0FF0_u32, ALU_AND),
        (0xF0F0_AA55_u32, 0x0FF0_0FF0_u32, ALU_OR),
        (0xF0F0_AA55_u32, 0x0FF0_0FF0_u32, ALU_XOR),
        (0x0000_0003_u32, 4_u32, ALU_SLL),
        (0x8000_0000_u32, 4_u32, ALU_SRL),
        (0x8000_0000_u32, 4_u32, ALU_SRA),
        (0xFFFF_FFFE_u32, 1_u32, ALU_SLT),
        (1_u32, 2_u32, ALU_SLTU),
        (0x1234_5678_u32, 0x1111_1111_u32, ALU_MUL),
        (0x8000_0000_u32, 2_u32, ALU_MULH),
        (0x8000_0000_u32, 2_u32, ALU_MULHSU),
        (0xFFFF_FFFF_u32, 0xFFFF_FFFF_u32, ALU_MULHU),
    ] {
        let result = run_single_operation(&mut dut, a, b, alu_op as u8);
        assert_eq!(
            result,
            expected_result(a, b, alu_op),
            "Unexpected result for op {alu_op:#04x}"
        );
    }
}

#[test]
fn test_dsp_pipe_accepts_back_to_back_inputs() {
    let runtime = create_dsp_pipe_runtime().expect("Failed to create DSP pipe runtime");
    let mut dut = testbench::create_testbench_model::<DspPipe>(&runtime).unwrap();

    let requests = [
        (5_u32, 7_u32, ALU_ADD),
        (0xF0F0_0000_u32, 0x0FF0_0FF0_u32, ALU_AND),
        (0x1234_5678_u32, 0x0000_00F0_u32, ALU_XOR),
        (0xFFFF_FFF0_u32, 0x0000_0003_u32, ALU_MUL),
    ];

    reset_dsp_pipe(&mut dut);

    let mut observed_results = Vec::new();

    for &(a, b, alu_op) in &requests {
        dut.a = a;
        dut.b = b;
        dut.alu_op = alu_op as u8;
        dut.in_valid = 1;
        clock_cycle!(dut);
        if dut.out_valid == 1 {
            observed_results.push(dut.out_data);
        }
    }

    dut.in_valid = 0;
    for _ in 0..4 {
        clock_cycle!(dut);
        if dut.out_valid == 1 {
            observed_results.push(dut.out_data);
        }
    }

    let expected_results: Vec<u32> = requests
        .iter()
        .map(|&(a, b, alu_op)| expected_result(a, b, alu_op))
        .collect();

    assert_eq!(
        observed_results, expected_results,
        "DSP pipe should emit one result per cycle after the pipeline fills"
    );
}
