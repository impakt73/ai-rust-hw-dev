use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;

use serde::Deserialize;

#[derive(Deserialize)]
struct CargoToml {
    bin: Vec<BinTarget>,
}

#[derive(Deserialize)]
struct BinTarget {
    name: String,
}

fn metadata_modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).ok()?.modified().ok()
}

fn update_latest_modified(path: &Path, latest: &mut Option<SystemTime>) {
    let Some(modified) = metadata_modified(path) else {
        return;
    };

    if latest.is_none_or(|current| current < modified) {
        *latest = Some(modified);
    }
}

fn visit_latest_modified(path: &Path, latest: &mut Option<SystemTime>) {
    update_latest_modified(path, latest);

    if !path.is_dir() {
        return;
    }

    let entries = fs::read_dir(path)
        .unwrap_or_else(|e| panic!("Failed to read directory {}: {}", path.display(), e));
    for entry in entries {
        let entry = entry.unwrap_or_else(|e| {
            panic!(
                "Failed to read directory entry under {}: {}",
                path.display(),
                e
            )
        });
        visit_latest_modified(&entry.path(), latest);
    }
}

fn rust_test_program_inputs_are_current(
    rust_test_program_dir: &Path,
    target_dir: &Path,
    binary_names: &[String],
) -> bool {
    let mut latest_input = None;
    for path in [
        rust_test_program_dir.join("src"),
        rust_test_program_dir.join("Cargo.toml"),
        rust_test_program_dir.join("build.rs"),
        rust_test_program_dir.join("memory.x"),
        rust_test_program_dir.join(".cargo").join("config.toml"),
    ] {
        visit_latest_modified(&path, &mut latest_input);
    }

    let Some(latest_input) = latest_input else {
        return false;
    };

    binary_names.iter().all(|binary_name| {
        metadata_modified(&target_dir.join(binary_name))
            .is_some_and(|artifact_time| artifact_time >= latest_input)
    })
}

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

    // Parse the Cargo.toml to get the list of binaries
    let cargo_toml_path = rust_test_program_dir.join("Cargo.toml");
    let cargo_toml_content = fs::read_to_string(&cargo_toml_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read Cargo.toml at {}: {}",
            cargo_toml_path.display(),
            e
        )
    });

    let cargo_toml: CargoToml = toml::from_str(&cargo_toml_content)
        .unwrap_or_else(|e| panic!("Failed to parse Cargo.toml: {}", e));

    let binary_names: Vec<String> = cargo_toml
        .bin
        .iter()
        .map(|bin_target| bin_target.name.clone())
        .collect();

    // Find the built binaries and copy them to OUT_DIR
    // The binaries are built to: rust-test-program/target/riscv32imafc-unknown-none-elf/release/
    let target_dir = rust_test_program_dir
        .join("target")
        .join("riscv32imafc-unknown-none-elf")
        .join("release");

    if !rust_test_program_inputs_are_current(&rust_test_program_dir, &target_dir, &binary_names) {
        let output = Command::new("cargo")
            .arg("build")
            .arg("--release")
            .current_dir(&rust_test_program_dir)
            .output()
            .expect("Failed to execute cargo build for rust-test-program");

        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!(
                "cargo:warning=Failed to build rust-test-program. Status: {}",
                output.status
            );
            if !stdout.is_empty() {
                println!("cargo:warning=rust-test-program stdout:\n{}", stdout);
            }
            if !stderr.is_empty() {
                println!("cargo:warning=rust-test-program stderr:\n{}", stderr);
            }
            panic!("Failed to build rust-test-program");
        }
    }

    // Copy all binaries to OUT_DIR without .elf extension
    for binary_name in &binary_names {
        let src = target_dir.join(binary_name);
        let dst = out_dir.join(binary_name);

        if src.exists() {
            fs::copy(&src, &dst).unwrap_or_else(|e| {
                panic!(
                    "Failed to copy {} to {}: {}",
                    src.display(),
                    dst.display(),
                    e
                )
            });
        } else {
            panic!(
                "Expected test binary not found at path: {}. \
                 Ensure the binary '{}' is defined in rust-test-program/Cargo.toml \
                 and builds successfully.",
                src.display(),
                binary_name
            );
        }
    }

    // Tell cargo to rerun if rust-test-program source files change
    println!(
        "cargo:rerun-if-changed={}",
        rust_test_program_dir.join("src").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        rust_test_program_dir.join("Cargo.toml").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        rust_test_program_dir.join("build.rs").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        rust_test_program_dir.join("memory.x").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        rust_test_program_dir
            .join(".cargo")
            .join("config.toml")
            .display()
    );
}
