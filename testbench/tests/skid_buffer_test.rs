use riscv_core::{create_skid_buffer_runtime, SkidBufferWrapper};

use riscv_core::AsDynamicVerilatedModel;
fn clock_cycle(dut: &mut SkidBufferWrapper) {
    dut.clk = 0;
    dut.eval();
    dut.clk = 1;
    dut.eval();
    dut.clk = 0;
    dut.eval();
}

fn reset_dut(dut: &mut SkidBufferWrapper) {
    dut.rst = 1;
    dut.in_valid = 0;
    dut.in_data = 0;
    dut.out_ready = 0;
    clock_cycle(dut);
    dut.rst = 0;
    clock_cycle(dut);
}

#[test]
fn test_skid_buffer_reset_state() {
    let runtime = create_skid_buffer_runtime().expect("Failed to create skid_buffer runtime");
    let mut dut = runtime
        .create_model_simple::<SkidBufferWrapper>()
        .expect("Failed to create skid_buffer model");

    dut.rst = 1;
    dut.in_valid = 0;
    dut.in_data = 0;
    dut.out_ready = 0;
    clock_cycle(&mut dut);

    assert_eq!(dut.out_valid, 0, "out_valid should clear during reset");
    assert_eq!(dut.in_ready, 1, "in_ready should be asserted after reset");
}

#[test]
fn test_skid_buffer_basic_flow() {
    let runtime = create_skid_buffer_runtime().expect("Failed to create skid_buffer runtime");
    let mut dut = runtime
        .create_model_simple::<SkidBufferWrapper>()
        .expect("Failed to create skid_buffer model");

    reset_dut(&mut dut);

    dut.out_ready = 1;
    dut.in_valid = 1;
    dut.in_data = 0xA5;
    clock_cycle(&mut dut);

    assert_eq!(
        dut.out_valid, 1,
        "input beat should appear at output register"
    );
    assert_eq!(
        dut.out_data, 0xA5,
        "output data should match accepted input"
    );

    dut.in_valid = 0;
    clock_cycle(&mut dut);
    assert_eq!(
        dut.out_valid, 0,
        "output should drain when out_ready is high"
    );
}

#[test]
fn test_skid_buffer_two_entry_backpressure() {
    let runtime = create_skid_buffer_runtime().expect("Failed to create skid_buffer runtime");
    let mut dut = runtime
        .create_model_simple::<SkidBufferWrapper>()
        .expect("Failed to create skid_buffer model");

    reset_dut(&mut dut);

    dut.out_ready = 0;
    dut.in_valid = 1;
    dut.in_data = 0x11;
    clock_cycle(&mut dut);

    dut.in_data = 0x22;
    clock_cycle(&mut dut);

    assert_eq!(
        dut.out_valid, 1,
        "output should hold first beat during stall"
    );
    assert_eq!(dut.out_data, 0x11, "first beat should remain at output");
    assert_eq!(
        dut.in_ready, 0,
        "in_ready should deassert when both entries are full"
    );

    dut.in_data = 0x33;
    clock_cycle(&mut dut);
    assert_eq!(
        dut.out_data, 0x11,
        "third beat must not overwrite buffered data"
    );

    dut.in_valid = 0;
    dut.out_ready = 1;
    clock_cycle(&mut dut);

    assert_eq!(
        dut.out_valid, 1,
        "second buffered beat should move to output"
    );
    assert_eq!(dut.out_data, 0x22, "second beat should follow first beat");
    assert_eq!(
        dut.in_ready, 1,
        "in_ready should reassert after one beat drains"
    );

    clock_cycle(&mut dut);
    assert_eq!(
        dut.out_valid, 0,
        "buffer should drain after both beats are consumed"
    );
}
