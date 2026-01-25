//! Test program path resolution for automatically built ELF files.
//!
//! This crate provides a helper function to locate ELF binaries that are
//! automatically built from the rust-test-program project during the build process.

use std::path::PathBuf;

/// Returns the path to a test program ELF file.
///
/// # Arguments
///
/// * `filename` - The name of the ELF file (e.g., "test_video_pattern.elf")
///
/// # Returns
///
/// Returns a `Result` with the full path to the ELF file if it exists,
/// or an error message if the file was not found.
///
/// # Examples
///
/// ```no_run
/// # use sim_tests::test_program_path;
/// let path = test_program_path("test_video_pattern.elf")
///     .expect("Failed to find test_video_pattern.elf");
/// ```
pub fn test_program_path(filename: &str) -> Result<PathBuf, String> {
    let out_dir = env!("OUT_DIR");
    let path = PathBuf::from(out_dir).join(filename);

    if path.exists() {
        Ok(path)
    } else {
        Err(format!(
            "Test program '{}' not found at {}. Available files: {:?}",
            filename,
            path.display(),
            std::fs::read_dir(out_dir)
                .ok().map(|entries| entries
                            .filter_map(|e| e.ok())
                            .filter(|e| e.path().extension().is_some_and(|ext| ext == "elf"))
                            .map(|e| e.file_name().to_string_lossy().to_string())
                            .collect::<Vec<_>>())
                .unwrap_or_default()
        ))
    }
}
