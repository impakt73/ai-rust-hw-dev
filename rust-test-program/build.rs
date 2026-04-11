use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // Put `memory.x` in our output directory and ensure it's
    // on the linker search path.
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let memory_layout = env::var("AIHWDEV_MEMORY_LAYOUT").unwrap_or_else(|_| "sram".to_string());
    let memory_script = match memory_layout.as_str() {
        "sram" => "memory.x",
        "sdram" => "memory-sdram.x",
        other => {
            panic!("Unsupported AIHWDEV_MEMORY_LAYOUT value '{other}'. Use 'sram' or 'sdram'.")
        }
    };

    // Copy the selected memory layout to the output directory as memory.x
    fs::copy(memory_script, out_dir.join("memory.x")).expect("Failed to copy selected memory.x");

    // Tell the linker where to find memory.x
    println!("cargo:rustc-link-search={}", out_dir.display());

    // Pass linker arguments for both memory.x and link.x (from riscv-rt)
    println!("cargo:rustc-link-arg=-Tmemory.x");
    println!("cargo:rustc-link-arg=-Tlink.x");

    // Tell Cargo to rerun if memory.x changes
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=memory-sdram.x");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=AIHWDEV_MEMORY_LAYOUT");
}
