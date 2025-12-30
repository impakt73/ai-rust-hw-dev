use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // Put `memory.x` in our output directory and ensure it's
    // on the linker search path.
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Copy memory.x to the output directory
    fs::copy("memory.x", out_dir.join("memory.x")).expect("Failed to copy memory.x");

    // Tell the linker where to find memory.x
    println!("cargo:rustc-link-search={}", out_dir.display());

    // Pass linker arguments for both memory.x and link.x (from riscv-rt)
    println!("cargo:rustc-link-arg=-Tmemory.x");
    println!("cargo:rustc-link-arg=-Tlink.x");

    // Tell Cargo to rerun if memory.x changes
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=build.rs");
}
