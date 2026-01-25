use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let rust_test_program_dir = manifest_dir.parent().unwrap().join("rust-test-program");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Ensure rust-test-program directory exists
    if !rust_test_program_dir.exists() {
        panic!(
            "rust-test-program directory not found at: {}",
            rust_test_program_dir.display()
        );
    }

    // Build rust-test-program using cargo
    println!("cargo:warning=Building rust-test-program in release mode...");
    
    let status = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .current_dir(&rust_test_program_dir)
        .status()
        .expect("Failed to execute cargo build for rust-test-program");

    if !status.success() {
        panic!("Failed to build rust-test-program");
    }

    // Find the built binaries and copy them to OUT_DIR with .elf extension
    // The binaries are built to: rust-test-program/target/riscv32imafc-unknown-none-elf/release/
    let target_dir = rust_test_program_dir
        .join("target")
        .join("riscv32imafc-unknown-none-elf")
        .join("release");

    // Copy all binaries to OUT_DIR with .elf extension
    // Get list of binaries from Cargo.toml
    let binaries = vec![
        "rust_test",
        "hello_world",
        "packet_test",
        "simple_test",
        "minimal_postcard_test",
        "minimal_postcard_test2",
        "minimal_debug_test",
        "test_allocator",
        "test_heap_directly",
        "test_stack_memory",
        "test_static_heap",
        "test_byte_store_simple",
        "test_alloc_only",
        "println_test",
        "test_memory_pattern",
        "test_image_data",
        "test_panic",
        "test_atomic",
        "test_atomic_simple",
        "test_fp_math",
        "test_dma_copy",
        "test_video_pattern",
        "test_audio_pattern",
        "test_sim_view",
        "test_video_loop",
        "test_audio_loop",
    ];

    for binary in &binaries {
        let src = target_dir.join(binary);
        let dst = out_dir.join(format!("{}.elf", binary));
        
        if src.exists() {
            std::fs::copy(&src, &dst)
                .unwrap_or_else(|e| panic!("Failed to copy {} to {}: {}", src.display(), dst.display(), e));
            println!("cargo:warning=Copied {} -> {}", binary, dst.display());
        } else {
            println!("cargo:warning=Binary not found: {}", src.display());
        }
    }

    // Tell cargo to rerun if rust-test-program source files change
    println!("cargo:rerun-if-changed={}", rust_test_program_dir.join("src").display());
    println!("cargo:rerun-if-changed={}", rust_test_program_dir.join("Cargo.toml").display());
    println!("cargo:rerun-if-changed={}", rust_test_program_dir.join("build.rs").display());
    println!("cargo:rerun-if-changed={}", rust_test_program_dir.join("memory.x").display());
}
