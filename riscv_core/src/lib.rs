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
    VerilatorRuntime::new(
        "target/verilator".into(),
        &[
            "../rtl/top.sv".as_ref(),
            "../rtl/alu.sv".as_ref(),
            "../rtl/regfile.sv".as_ref(),
            "../rtl/decoder.sv".as_ref(),
        ],
        &[],
        [],
        VerilatorRuntimeOptions::default(),
    )
    .unwrap()
}

// Helper function to create a runtime for the ALU
pub fn create_alu_runtime() -> VerilatorRuntime {
    VerilatorRuntime::new(
        "target/verilator".into(),
        &["../rtl/alu.sv".as_ref()],
        &[],
        [],
        VerilatorRuntimeOptions::default(),
    )
    .unwrap()
}

// Helper function to create a runtime for the RegFile
pub fn create_regfile_runtime() -> VerilatorRuntime {
    VerilatorRuntime::new(
        "target/verilator".into(),
        &["../rtl/regfile.sv".as_ref()],
        &[],
        [],
        VerilatorRuntimeOptions::default(),
    )
    .unwrap()
}
