// Re-export marlin for convenience
pub use marlin::verilator::vcd::Vcd;
pub use marlin::verilator::{VerilatedModelConfig, VerilatorRuntime, VerilatorRuntimeOptions};
pub use marlin::verilog::prelude::*;

// Disassembler module
pub mod disasm;

// Instruction trace module
pub mod trace;

// Instruction encoding utilities
pub mod instruction;

// Define the Cpu module (CPU core)
#[verilog(src = "../rtl/cpu.sv", name = "cpu")]
pub struct Cpu;

// Define the Top module (top-level wrapper with RTL peripherals)
#[verilog(src = "../rtl/top.sv", name = "top")]
pub struct Top;

// Define ALU module
#[verilog(src = "../rtl/alu.sv", name = "alu")]
pub struct Alu;

// Define RegFile module
#[verilog(src = "../rtl/regfile.sv", name = "regfile")]
pub struct RegFile;

// Define Decompress module
#[verilog(src = "../rtl/decompress.sv", name = "decompress")]
pub struct Decompress;

// Define FP RegFile module
#[verilog(src = "../rtl/fp_regfile.sv", name = "fp_regfile")]
pub struct FpRegFile;

// Define FPU module
#[verilog(src = "../rtl/fpu.sv", name = "fpu")]
pub struct Fpu;

// Define LED Controller module
#[verilog(
    src = "../rtl/peripherals/led_controller_peripheral.sv",
    name = "led_controller_peripheral"
)]
pub struct LedControllerPeripheral;

// Define UART core module (no FIFOs, ready/valid interface)
#[verilog(src = "../rtl/uart.sv", name = "uart")]
pub struct Uart;

// Define Clock Peripheral module
#[verilog(
    src = "../rtl/peripherals/clock_peripheral.sv",
    name = "clock_peripheral"
)]
pub struct ClockPeripheral;

// Define System Controller module
#[verilog(
    src = "../rtl/peripherals/system_controller.sv",
    name = "system_controller"
)]
pub struct SystemController;

// Define Host Bus Interface module
#[verilog(src = "../rtl/host_bus_interface.sv", name = "host_bus_interface")]
pub struct HostBusInterface;

// Define Host RX Buffer module (bidirectional packet buffering)
#[verilog(src = "../rtl/host_rx_buffer.sv", name = "host_rx_buffer")]
pub struct HostRxBuffer;

// Define Bus Arbiter module
#[verilog(src = "../rtl/bus_arbiter.sv", name = "bus_arbiter")]
pub struct BusArbiter;

// Define FPU submodules
#[verilog(src = "../rtl/fpu_classifier.sv", name = "fpu_classifier")]
pub struct FpuClassifier;

#[verilog(src = "../rtl/fpu_comparator.sv", name = "fpu_comparator")]
pub struct FpuComparator;

#[verilog(src = "../rtl/fpu_int_to_float.sv", name = "fpu_int_to_float")]
pub struct FpuIntToFloat;

#[verilog(src = "../rtl/fpu_float_to_int.sv", name = "fpu_float_to_int")]
pub struct FpuFloatToInt;

#[verilog(src = "../rtl/fpu_sqrt.sv", name = "fpu_sqrt")]
pub struct FpuSqrt;

// Define FF synchronizer default wrapper module
#[verilog(
    src = "../rtl/ff_sync_default_wrapper.sv",
    name = "ff_sync_default_wrapper"
)]
pub struct FfSyncDefaultWrapper;

// Define FF synchronizer parameterized wrapper module (2-stage, 4-bit)
#[verilog(
    src = "../rtl/ff_sync_param_wrapper.sv",
    name = "ff_sync_param_wrapper"
)]
pub struct FfSyncParamWrapper;

// Define Async FIFO wrapper modules for CDC FIFO tests
#[verilog(
    src = "../rtl/async_fifo_test_wrapper.sv",
    name = "async_fifo_test_wrapper"
)]
pub struct AsyncFifoTestWrapper;

#[verilog(
    src = "../rtl/async_fifo_sync3_wrapper.sv",
    name = "async_fifo_sync3_wrapper"
)]
pub struct AsyncFifoSync3Wrapper;

// Helper function to determine RTL path
fn get_rtl_path() -> &'static str {
    if std::path::Path::new("rtl").exists() {
        "rtl"
    } else {
        "../rtl"
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
    // This includes the main RTL directory and subdirectories (e.g., peripherals/)
    let include_paths = [rtl_path];

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
        "top.sv",                                   // Top-level wrapper with RTL peripherals
        "reset_controller.sv",                      // Power-on reset controller
        "bus.sv",                                   // System bus for address decoding
        "cpu.sv",                                   // CPU core
        "sync_fifo.sv",                             // Generic synchronous FIFO
        "host_bus_interface.sv", // Host bus interface for serialized transactions
        "peripherals/led_controller_peripheral.sv", // LED controller peripheral
        "peripherals/clock_peripheral.sv", // Clock peripheral
        "peripherals/system_controller.sv", // System controller peripheral
        "fetch_buffer.sv",       // RV32C fetch buffer
        "decompress.sv",         // RV32C decompressor
        "alu.sv",
        "div_unit.sv",
        "mul_unit.sv",
        "regfile.sv",
        "decoder.sv",
        "branch_unit.sv",
        "csr_file.sv",
        "mem_interface.sv",
        "writeback_mux.sv",
        "fp_regfile.sv", // RV32F FP register file
        "fpu.sv",        // RV32F floating point unit
    ])
}

// Helper function to create a runtime for the ALU
pub fn create_alu_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["alu.sv", "div_unit.sv", "mul_unit.sv"])
}

// Helper function to create a runtime for the MulUnit
pub fn create_mul_unit_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["mul_unit.sv"])
}

// Helper function to create a runtime for the RegFile
pub fn create_regfile_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["regfile.sv"])
}

// Helper function to create a runtime for the Decompressor
pub fn create_decompress_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["decompress.sv"])
}

// Helper function to create a runtime for the FP RegFile
pub fn create_fp_regfile_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["fp_regfile.sv"])
}

// Helper function to create a runtime for the UART core
pub fn create_uart_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["uart.sv"])
}

// Helper function to create a runtime for the Clock Peripheral
pub fn create_clock_peripheral_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["peripherals/clock_peripheral.sv"])
}

// Helper function to create a runtime for the System Controller
pub fn create_system_controller_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["peripherals/system_controller.sv"])
}

// Helper function to create a runtime for the Host Bus Interface
pub fn create_host_bus_interface_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["host_bus_interface.sv", "host_rx_buffer.sv"])
}

// Helper function to create a runtime for the Host RX Buffer (standalone testing)
pub fn create_host_rx_buffer_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["host_rx_buffer.sv"])
}

// Helper function to create a runtime for the Bus Arbiter
pub fn create_bus_arbiter_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["bus_arbiter.sv"])
}

// Helper function to create a runtime for the FPU
pub fn create_fpu_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "fpu.sv",
        "div_unit.sv",
        "fpu_classifier.sv",
        "fpu_comparator.sv",
        "fpu_int_to_float.sv",
        "fpu_float_to_int.sv",
        "fpu_sqrt.sv",
        "fpu_div_setup.sv",
        "fpu_div_assemble.sv",
        "fpu_fma.sv",
    ]) // FPU now uses modular design with separate submodules
}

// Helper functions to create runtimes for FPU submodules
pub fn create_fpu_classifier_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["fpu_classifier.sv"])
}

pub fn create_fpu_comparator_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["fpu_comparator.sv"])
}

pub fn create_fpu_int_to_float_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["fpu_int_to_float.sv"])
}

pub fn create_fpu_float_to_int_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["fpu_float_to_int.sv"])
}

pub fn create_fpu_sqrt_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["fpu_sqrt.sv"])
}

// Helper function to create a runtime for the FF synchronizer
pub fn create_ff_sync_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["ff_sync.sv", "ff_sync_default_wrapper.sv"])
}

// Helper function to create a runtime for parameterized FF synchronizer wrapper
pub fn create_ff_sync_param_wrapper_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>>
{
    create_runtime(&["ff_sync.sv", "ff_sync_param_wrapper.sv"])
}

// Helper function to create a runtime for async FIFO wrapper (DEPTH=4, SYNC_STAGES=2)
pub fn create_async_fifo_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "ff_sync.sv",
        "sync_dpram.sv",
        "async_fifo.sv",
        "async_fifo_test_wrapper.sv",
    ])
}

// Helper function to create a runtime for async FIFO wrapper with SYNC_STAGES=3
pub fn create_async_fifo_sync3_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "ff_sync.sv",
        "sync_dpram.sv",
        "async_fifo.sv",
        "async_fifo_sync3_wrapper.sv",
    ])
}
