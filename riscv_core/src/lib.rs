// Re-export marlin for convenience
pub use marlin::verilator::vcd::Vcd;
pub use marlin::verilator::{
    AsDynamicVerilatedModel, VerilatedModelConfig, VerilatorRuntime, VerilatorRuntimeOptions,
};
pub use marlin::verilog::prelude::*;

// Disassembler module
pub mod disasm;

// Instruction trace module
pub mod trace;

// Instruction encoding utilities
pub mod instruction;

// Define the Cpu module (CPU core)
#[verilog(src = "../rtl/common/cpu/cpu.sv", name = "cpu")]
pub struct Cpu;

// Define the Top module (top-level wrapper with RTL peripherals)
#[verilog(src = "../rtl/common/top.sv", name = "top")]
pub struct Top;

// Define ALU module
#[verilog(src = "../rtl/common/cpu/alu.sv", name = "alu")]
pub struct Alu;

// Define DSP pipeline module
#[verilog(src = "../rtl/common/cpu/dsp_pipe.sv", name = "dsp_pipe")]
pub struct DspPipe;

// Define RegFile module
#[verilog(src = "../rtl/common/cpu/regfile.sv", name = "regfile")]
pub struct RegFile;

// Define Decompress module
#[verilog(src = "../rtl/common/cpu/decompress.sv", name = "decompress")]
pub struct Decompress;

// Define FetchBuffer module
#[verilog(src = "../rtl/common/cpu/fetch_buffer.sv", name = "fetch_buffer")]
pub struct FetchBuffer;

// Define FP RegFile module
#[verilog(src = "../rtl/common/fpu/fp_regfile.sv", name = "fp_regfile")]
pub struct FpRegFile;

// Define FPU module
#[verilog(src = "../rtl/common/fpu/fpu.sv", name = "fpu")]
pub struct Fpu;

// Define UART core module (no FIFOs, ready/valid interface)
#[verilog(src = "../rtl/common/io/uart.sv", name = "uart")]
pub struct Uart;

// Define UART wrapper module configured for 1M baud
#[verilog(
    src = "../rtl/common/wrappers/uart_1m_baud_wrapper.sv",
    name = "uart_1m_baud_wrapper"
)]
pub struct Uart1MBaud;

#[verilog(
    src = "../rtl/common/wrappers/i2s_serializer_test_wrappers.sv",
    name = "i2s_serializer_equal_width_wrapper"
)]
pub struct I2sSerializerEqualWidthWrapper;

#[verilog(
    src = "../rtl/common/wrappers/i2s_serializer_test_wrappers.sv",
    name = "i2s_serializer_expand_wrapper"
)]
pub struct I2sSerializerExpandWrapper;

#[verilog(
    src = "../rtl/common/wrappers/i2s_serializer_test_wrappers.sv",
    name = "i2s_serializer_truncate_wrapper"
)]
pub struct I2sSerializerTruncateWrapper;

// Define System Controller module
#[verilog(
    src = "../rtl/common/peripherals/system_controller_peripheral.sv",
    name = "system_controller"
)]
pub struct SystemController;

// Define System LED Controller module
#[verilog(
    src = "../rtl/common/io/sys_led_controller.sv",
    name = "sys_led_controller"
)]
pub struct SysLedController;

// Define Host Bus Interface module
#[verilog(
    src = "../rtl/common/io/host_bus_interface.sv",
    name = "host_bus_interface"
)]
pub struct HostBusInterface;

// Define Host Bus Mux module
#[verilog(src = "../rtl/common/io/host_bus_mux.sv", name = "host_bus_mux")]
pub struct HostBusMux;

// Define Host RX Buffer module (bidirectional packet buffering)
#[verilog(src = "../rtl/common/io/host_bus_rx.sv", name = "host_bus_rx")]
pub struct HostBusRx;

#[verilog(
    src = "../rtl/common/wrappers/registered_bus_wrapper.sv",
    name = "registered_bus_wrapper"
)]
pub struct RegisteredBusWrapper;

// Define FPU submodules
#[verilog(src = "../rtl/common/fpu/fpu_classifier.sv", name = "fpu_classifier")]
pub struct FpuClassifier;

#[verilog(src = "../rtl/common/fpu/fpu_comparator.sv", name = "fpu_comparator")]
pub struct FpuComparator;

#[verilog(
    src = "../rtl/common/fpu/fpu_int_to_float.sv",
    name = "fpu_int_to_float"
)]
pub struct FpuIntToFloat;

#[verilog(
    src = "../rtl/common/fpu/fpu_float_to_int.sv",
    name = "fpu_float_to_int"
)]
pub struct FpuFloatToInt;

#[verilog(src = "../rtl/common/fpu/fpu_sqrt.sv", name = "fpu_sqrt")]
pub struct FpuSqrt;

// Define FF synchronizer default wrapper module
#[verilog(
    src = "../rtl/common/wrappers/ff_sync_default_wrapper.sv",
    name = "ff_sync_default_wrapper"
)]
pub struct FfSyncDefaultWrapper;

// Define FF synchronizer parameterized wrapper module (2-stage, 4-bit)
#[verilog(
    src = "../rtl/common/wrappers/ff_sync_param_wrapper.sv",
    name = "ff_sync_param_wrapper"
)]
pub struct FfSyncParamWrapper;

// Define Async FIFO wrapper modules for CDC FIFO tests
#[verilog(
    src = "../rtl/common/wrappers/async_fifo_test_wrapper.sv",
    name = "async_fifo_test_wrapper"
)]
pub struct AsyncFifoTestWrapper;

#[verilog(
    src = "../rtl/common/wrappers/async_fifo_sync3_wrapper.sv",
    name = "async_fifo_sync3_wrapper"
)]
pub struct AsyncFifoSync3Wrapper;

#[verilog(
    src = "../rtl/common/wrappers/cdc_handshake_test_wrapper.sv",
    name = "cdc_handshake_test_wrapper"
)]
pub struct CdcHandshakeTestWrapper;

#[verilog(
    src = "../rtl/common/wrappers/cdc_handshake_param_wrapper.sv",
    name = "cdc_handshake_param_wrapper"
)]
pub struct CdcHandshakeParamWrapper;

#[verilog(
    src = "../rtl/common/wrappers/bus_cdc_bridge_test_wrapper.sv",
    name = "bus_cdc_bridge_test_wrapper"
)]
pub struct BusCdcBridgeTestWrapper;

#[verilog(
    src = "../rtl/common/wrappers/sync_fifo_test_wrapper.sv",
    name = "sync_fifo_test_wrapper"
)]
pub struct SyncFifoTestWrapper;

#[verilog(
    src = "../rtl/common/wrappers/sram_test_wrapper.sv",
    name = "sram_test_wrapper"
)]
pub struct SramTestWrapper;

#[verilog(
    src = "../rtl/common/wrappers/sram_peripheral_test_wrapper.sv",
    name = "sram_peripheral_test_wrapper"
)]
pub struct SramPeripheralTestWrapper;

#[verilog(
    src = "../rtl/common/wrappers/gfx2d_peripheral_test_wrapper.sv",
    name = "gfx2d_peripheral_test_wrapper"
)]
pub struct Gfx2dPeripheralTestWrapper;

#[verilog(
    src = "../rtl/common/wrappers/sync_sprom_test_wrapper.sv",
    name = "sync_sprom_test_wrapper"
)]
pub struct SyncSpromTestWrapper;

#[verilog(
    src = "../rtl/common/wrappers/phase_accumulator_wrapper.sv",
    name = "phase_accumulator_wrapper"
)]
pub struct PhaseAccumulatorWrapper;

#[verilog(
    src = "../rtl/common/wrappers/activity_indicator_wrapper.sv",
    name = "activity_indicator_wrapper"
)]
pub struct ActivityIndicatorWrapper;

#[verilog(
    src = "../rtl/common/wrappers/skid_buffer_wrapper.sv",
    name = "skid_buffer_wrapper"
)]
pub struct SkidBufferWrapper;

#[verilog(
    src = "../rtl/common/wrappers/square_wave_generator_wrapper.sv",
    name = "square_wave_generator_wrapper"
)]
pub struct SquareWaveGeneratorWrapper;

#[verilog(
    src = "../rtl/common/wrappers/debouncer_wrapper.sv",
    name = "debouncer_wrapper"
)]
pub struct DebouncerWrapper;

#[verilog(
    src = "../rtl/common/wrappers/debouncer_single_cycle_wrapper.sv",
    name = "debouncer_single_cycle_wrapper"
)]
pub struct DebouncerSingleCycleWrapper;

#[verilog(
    src = "../rtl/common/wrappers/video_sync_test_wrappers.sv",
    name = "video_sync_wrapper"
)]
pub struct VideoSyncWrapper;

#[verilog(
    src = "../rtl/common/wrappers/video_sync_test_wrappers.sv",
    name = "video_sync_minimal_wrapper"
)]
pub struct VideoSyncMinimalWrapper;

#[verilog(
    src = "../rtl/common/wrappers/bitmap_text_renderer_test_wrapper.sv",
    name = "bitmap_text_renderer_test_wrapper"
)]
pub struct BitmapTextRendererTestWrapper;

#[verilog(
    src = "../rtl/common/wrappers/sys_led_controller_wrapper.sv",
    name = "sys_led_controller_wrapper"
)]
pub struct SysLedControllerWrapper;

#[verilog(
    src = "../rtl/common/wrappers/sine_table_test_wrapper.sv",
    name = "sine_table_test_wrapper"
)]
pub struct SineTableTestWrapper;

#[verilog(
    src = "../rtl/common/wrappers/tone_generator_test_wrapper.sv",
    name = "tone_generator_test_wrapper"
)]
pub struct ToneGeneratorTestWrapper;

// Helper function to determine RTL path
fn get_rtl_path() -> &'static str {
    if std::path::Path::new("rtl/common/top.sv").is_file() {
        "rtl/common"
    } else {
        "../rtl/common"
    }
}

// Generic helper to create a Verilator runtime with specified files
fn create_runtime(files: &[&str]) -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    let rtl_path = get_rtl_path();
    let file_paths: Vec<String> = files
        .iter()
        .map(|file| format!("{}/{}", rtl_path, file))
        .collect();

    // Convert to references that can be passed to VerilatorRuntime::new
    let file_refs: Vec<&str> = file_paths.iter().map(|s| s.as_str()).collect();

    // Set up include paths for Verilator to find all RTL modules
    // This includes the main RTL directory and all subdirectories
    let cpu_path = format!("{}/cpu", rtl_path);
    let fpu_path = format!("{}/fpu", rtl_path);
    let memory_path = format!("{}/memory", rtl_path);
    let primitives_path = format!("{}/primitives", rtl_path);
    let io_path = format!("{}/io", rtl_path);
    let wrappers_path = format!("{}/wrappers", rtl_path);
    let peripherals_path = format!("{}/peripherals", rtl_path);
    let include_paths = [
        rtl_path,
        cpu_path.as_str(),
        fpu_path.as_str(),
        memory_path.as_str(),
        primitives_path.as_str(),
        io_path.as_str(),
        wrappers_path.as_str(),
        peripherals_path.as_str(),
    ];

    VerilatorRuntime::new(
        "target/verilator".into(),
        &file_refs.iter().map(|s| (*s).as_ref()).collect::<Vec<_>>(),
        &include_paths
            .iter()
            .map(|s| (*s).as_ref())
            .collect::<Vec<_>>(),
        [],
        VerilatorRuntimeOptions::default(),
    )
    .map_err(|e| e.into())
}

// Helper function to create a runtime for the full CPU
pub fn create_cpu_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "top.sv",                         // Top-level wrapper with RTL peripherals
        "primitives/reset_controller.sv", // Power-on reset controller
        "primitives/ff_sync.sv",
        "primitives/cdc_handshake.sv",
        "primitives/bus_cdc_bridge.sv",
        "primitives/video_sync.sv",
        "primitives/bitmap_text_renderer.sv",
        "cpu/cpu.sv",               // CPU core
        "memory/sync_dpram.sv",     // BRAM-friendly simple dual-port RAM
        "memory/registered_bus.sv", // Registered bus helper used by top-level peripherals
        "memory/sync_sprom.sv",
        "primitives/sync_fifo.sv",             // Generic synchronous FIFO
        "primitives/square_wave_generator.sv", // System LED boot blink generator
        "primitives/activity_indicator.sv",    // System LED activity indicators
        "io/host_bus_mux.sv", // CPU routing mux between system bus and host bus interface
        "io/host_bus_interface.sv", // Host bus interface for serialized transactions
        "io/host_bus_rx.sv",  // Host bus receive path used by host_bus_interface
        "io/host_bus_tx.sv",  // Host bus transmit path used by host_bus_interface
        "io/sys_led_controller.sv", // System LED controller
        "peripherals/gfx2d_peripheral.sv", // GFX2D peripheral
        "peripherals/sram_peripheral.sv", // SRAM peripheral
        "memory/sram.sv",     // SRAM module used by SRAM peripheral
        "peripherals/system_controller_peripheral.sv", // System controller peripheral
        "cpu/fetch_buffer.sv", // RV32C fetch buffer
        "cpu/decompress.sv",  // RV32C decompressor
        "cpu/alu.sv",
        "cpu/div_unit.sv",
        "cpu/mul_unit.sv",
        "cpu/regfile.sv",
        "cpu/decoder.sv",
        "cpu/branch_unit.sv",
        "cpu/csr_file.sv",
        "cpu/mem_interface.sv",
        "cpu/writeback_mux.sv",
        "fpu/fp_regfile.sv", // RV32F FP register file
        "fpu/fpu.sv",        // RV32F floating point unit
    ])
}

// Helper function to create a runtime for the ALU
pub fn create_alu_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["cpu/alu.sv", "cpu/div_unit.sv", "cpu/mul_unit.sv"])
}

// Helper function to create a runtime for the DSP pipeline
pub fn create_dsp_pipe_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["cpu/dsp_pipe.sv"])
}

// Helper function to create a runtime for the MulUnit
pub fn create_mul_unit_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["cpu/mul_unit.sv"])
}

// Helper function to create a runtime for the RegFile
pub fn create_regfile_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["cpu/regfile.sv"])
}

// Helper function to create a runtime for the Decompressor
pub fn create_decompress_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["cpu/decompress.sv"])
}

// Helper function to create a runtime for the Fetch Buffer
pub fn create_fetch_buffer_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["cpu/fetch_buffer.sv", "cpu/decompress.sv"])
}

// Helper function to create a runtime for the FP RegFile
pub fn create_fp_regfile_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["fpu/fp_regfile.sv"])
}

// Helper function to create a runtime for the UART core
pub fn create_uart_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["io/uart.sv"])
}

// Helper function to create a runtime for the UART core at 1M baud
pub fn create_uart_1m_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["io/uart.sv", "wrappers/uart_1m_baud_wrapper.sv"])
}

pub fn create_i2s_serializer_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "io/i2s_serializer.sv",
        "wrappers/i2s_serializer_test_wrappers.sv",
    ])
}

// Helper function to create a runtime for the System Controller
pub fn create_system_controller_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["peripherals/system_controller_peripheral.sv"])
}

// Helper function to create a runtime for the System LED Controller
pub fn create_sys_led_controller_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "primitives/activity_indicator.sv",
        "primitives/square_wave_generator.sv",
        "io/sys_led_controller.sv",
        "wrappers/sys_led_controller_wrapper.sv",
    ])
}

// Helper function to create a runtime for the Host Bus Interface
pub fn create_host_bus_interface_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "io/host_bus_interface.sv",
        "io/host_bus_rx.sv",
        "io/host_bus_tx.sv",
    ])
}

// Helper function to create a runtime for the Host Bus Mux
pub fn create_host_bus_mux_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["io/host_bus_mux.sv"])
}

// Helper function to create a runtime for the Host RX Buffer (standalone testing)
pub fn create_host_bus_rx_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["io/host_bus_rx.sv"])
}

// Helper function to create a runtime for the Registered Bus
pub fn create_registered_bus_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "memory/registered_bus.sv",
        "wrappers/registered_bus_wrapper.sv",
    ])
}

// Helper function to create a runtime for the FPU
pub fn create_fpu_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "fpu/fpu.sv",
        "cpu/div_unit.sv",
        "fpu/fpu_classifier.sv",
        "fpu/fpu_comparator.sv",
        "fpu/fpu_int_to_float.sv",
        "fpu/fpu_float_to_int.sv",
        "fpu/fpu_sqrt.sv",
        "fpu/fpu_div_setup.sv",
        "fpu/fpu_div_assemble.sv",
        "fpu/fpu_fma.sv",
    ]) // FPU now uses modular design with separate submodules
}

// Helper functions to create runtimes for FPU submodules
pub fn create_fpu_classifier_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["fpu/fpu_classifier.sv"])
}

pub fn create_fpu_comparator_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["fpu/fpu_comparator.sv"])
}

pub fn create_fpu_int_to_float_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["fpu/fpu_int_to_float.sv"])
}

pub fn create_fpu_float_to_int_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["fpu/fpu_float_to_int.sv"])
}

pub fn create_fpu_sqrt_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["fpu/fpu_sqrt.sv"])
}

// Helper function to create a runtime for the FF synchronizer
pub fn create_ff_sync_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "primitives/ff_sync.sv",
        "wrappers/ff_sync_default_wrapper.sv",
    ])
}

// Helper function to create a runtime for parameterized FF synchronizer wrapper
pub fn create_ff_sync_param_wrapper_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>>
{
    create_runtime(&["primitives/ff_sync.sv", "wrappers/ff_sync_param_wrapper.sv"])
}

// Helper function to create a runtime for async FIFO wrapper (DEPTH=4, SYNC_STAGES=2)
pub fn create_async_fifo_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "primitives/ff_sync.sv",
        "memory/sync_dpram.sv",
        "primitives/async_fifo.sv",
        "wrappers/async_fifo_test_wrapper.sv",
    ])
}

// Helper function to create a runtime for async FIFO wrapper with SYNC_STAGES=3
pub fn create_async_fifo_sync3_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "primitives/ff_sync.sv",
        "memory/sync_dpram.sv",
        "primitives/async_fifo.sv",
        "wrappers/async_fifo_sync3_wrapper.sv",
    ])
}

// Helper function to create a runtime for the default CDC handshake wrapper
pub fn create_cdc_handshake_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "primitives/ff_sync.sv",
        "primitives/cdc_handshake.sv",
        "wrappers/cdc_handshake_test_wrapper.sv",
    ])
}

// Helper function to create a runtime for the parameterized CDC handshake wrapper
pub fn create_cdc_handshake_param_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>>
{
    create_runtime(&[
        "primitives/ff_sync.sv",
        "primitives/cdc_handshake.sv",
        "wrappers/cdc_handshake_param_wrapper.sv",
    ])
}

pub fn create_bus_cdc_bridge_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "primitives/ff_sync.sv",
        "primitives/cdc_handshake.sv",
        "primitives/bus_cdc_bridge.sv",
        "wrappers/bus_cdc_bridge_test_wrapper.sv",
    ])
}

// Helper function to create a runtime for sync FIFO wrapper (DEPTH=4)
pub fn create_sync_fifo_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "memory/sync_dpram.sv",
        "primitives/sync_fifo.sv",
        "wrappers/sync_fifo_test_wrapper.sv",
    ])
}

pub fn create_sram_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["memory/sram.sv", "wrappers/sram_test_wrapper.sv"])
}

pub fn create_sram_peripheral_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "memory/sram.sv",
        "peripherals/sram_peripheral.sv",
        "wrappers/sram_peripheral_test_wrapper.sv",
    ])
}

pub fn create_gfx2d_peripheral_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "primitives/ff_sync.sv",
        "primitives/cdc_handshake.sv",
        "primitives/bus_cdc_bridge.sv",
        "memory/sync_sprom.sv",
        "primitives/video_sync.sv",
        "primitives/bitmap_text_renderer.sv",
        "peripherals/gfx2d_peripheral.sv",
        "wrappers/gfx2d_peripheral_test_wrapper.sv",
    ])
}

pub fn create_sync_sprom_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "memory/sync_sprom.sv",
        "wrappers/sync_sprom_test_wrapper.sv",
    ])
}

pub fn create_phase_accumulator_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "primitives/phase_accumulator.sv",
        "wrappers/phase_accumulator_wrapper.sv",
    ])
}

pub fn create_activity_indicator_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "primitives/activity_indicator.sv",
        "wrappers/activity_indicator_wrapper.sv",
    ])
}

pub fn create_skid_buffer_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "primitives/skid_buffer.sv",
        "wrappers/skid_buffer_wrapper.sv",
    ])
}

pub fn create_square_wave_generator_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>>
{
    create_runtime(&[
        "primitives/square_wave_generator.sv",
        "wrappers/square_wave_generator_wrapper.sv",
    ])
}

pub fn create_debouncer_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["primitives/debouncer.sv", "wrappers/debouncer_wrapper.sv"])
}

pub fn create_debouncer_single_cycle_runtime(
) -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "primitives/debouncer.sv",
        "wrappers/debouncer_single_cycle_wrapper.sv",
    ])
}

pub fn create_video_sync_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "primitives/video_sync.sv",
        "wrappers/video_sync_test_wrappers.sv",
    ])
}

pub fn create_bitmap_text_renderer_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>>
{
    create_runtime(&[
        "memory/sync_sprom.sv",
        "primitives/video_sync.sv",
        "primitives/bitmap_text_renderer.sv",
        "wrappers/bitmap_text_renderer_test_wrapper.sv",
    ])
}

pub fn create_sine_table_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "memory/sync_sprom.sv",
        "primitives/sine_table.sv",
        "wrappers/sine_table_test_wrapper.sv",
    ])
}

pub fn create_tone_generator_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "memory/sync_sprom.sv",
        "primitives/sine_table.sv",
        "primitives/tone_generator.sv",
        "wrappers/tone_generator_test_wrapper.sv",
    ])
}
