// Re-export marlin for convenience
pub use marlin::verilator::{VerilatorRuntime, VerilatorRuntimeOptions};
pub use marlin::verilog::prelude::*;

// Define the Top module that can be shared across the workspace
#[verilog(src = "../rtl/top.sv", name = "top")]
pub struct Top;

// Define ALU module
#[verilog(src = "../rtl/alu.sv", name = "alu")]
pub struct Alu;

// Define RegFile module
#[verilog(src = "../rtl/regfile.sv", name = "regfile")]
pub struct RegFile;

// Helper function to create a runtime for the full CPU
pub fn create_cpu_runtime() -> VerilatorRuntime {
    // Get the path to the RTL directory
    // When compiled, this will be relative to the workspace root
    let rtl_path = if std::path::Path::new("rtl").exists() {
        "rtl"
    } else {
        "../rtl"
    };

    VerilatorRuntime::new(
        "target/verilator".into(),
        &[
            format!("{}/top.sv", rtl_path).as_ref(),
            format!("{}/alu.sv", rtl_path).as_ref(),
            format!("{}/regfile.sv", rtl_path).as_ref(),
            format!("{}/decoder.sv", rtl_path).as_ref(),
        ],
        &[],
        [],
        VerilatorRuntimeOptions::default(),
    )
    .unwrap()
}

// Helper function to create a runtime for the ALU
pub fn create_alu_runtime() -> VerilatorRuntime {
    let rtl_path = if std::path::Path::new("rtl").exists() {
        "rtl"
    } else {
        "../rtl"
    };

    VerilatorRuntime::new(
        "target/verilator".into(),
        &[format!("{}/alu.sv", rtl_path).as_ref()],
        &[],
        [],
        VerilatorRuntimeOptions::default(),
    )
    .unwrap()
}

// Helper function to create a runtime for the RegFile
pub fn create_regfile_runtime() -> VerilatorRuntime {
    let rtl_path = if std::path::Path::new("rtl").exists() {
        "rtl"
    } else {
        "../rtl"
    };

    VerilatorRuntime::new(
        "target/verilator".into(),
        &[format!("{}/regfile.sv", rtl_path).as_ref()],
        &[],
        [],
        VerilatorRuntimeOptions::default(),
    )
    .unwrap()
}
