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

// Define the Top module that can be shared across the workspace
#[verilog(src = "../rtl/top.sv", name = "top")]
pub struct Top;

// Define the TopWithPeripherals module (wrapper with RTL peripherals)
#[verilog(src = "../rtl/top_with_peripherals.sv", name = "top_with_peripherals")]
pub struct TopWithPeripherals;

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

// Define FPU submodules
#[verilog(src = "../rtl/fpu_classifier.sv", name = "fpu_classifier")]
pub struct FpuClassifier;

#[verilog(src = "../rtl/fpu_comparator.sv", name = "fpu_comparator")]
pub struct FpuComparator;

#[verilog(src = "../rtl/fpu_adder.sv", name = "fpu_adder")]
pub struct FpuAdder;

#[verilog(src = "../rtl/fpu_multiplier.sv", name = "fpu_multiplier")]
pub struct FpuMultiplier;

#[verilog(src = "../rtl/fpu_int_to_float.sv", name = "fpu_int_to_float")]
pub struct FpuIntToFloat;

#[verilog(src = "../rtl/fpu_float_to_int.sv", name = "fpu_float_to_int")]
pub struct FpuFloatToInt;

#[verilog(src = "../rtl/fpu_sqrt.sv", name = "fpu_sqrt")]
pub struct FpuSqrt;

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
        "top_with_peripherals.sv",       // Top-level wrapper with RTL peripherals
        "top.sv",                        // CPU core
        "peripherals/led_controller.sv", // LED controller peripheral
        "fetch_buffer.sv",               // RV32C fetch buffer
        "decompress.sv",                 // RV32C decompressor
        "alu.sv",
        "div_unit.sv",
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
    create_runtime(&["alu.sv", "div_unit.sv"])
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

// Helper function to create a runtime for the FPU
pub fn create_fpu_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&[
        "fpu.sv",
        "div_unit.sv",
        "fpu_classifier.sv",
        "fpu_comparator.sv",
        "fpu_adder.sv",
        "fpu_multiplier.sv",
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

pub fn create_fpu_adder_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["fpu_adder.sv"])
}

pub fn create_fpu_multiplier_runtime() -> Result<VerilatorRuntime, Box<dyn std::error::Error>> {
    create_runtime(&["fpu_multiplier.sv"])
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
