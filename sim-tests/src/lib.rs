//! Test program path resolution for automatically built ELF files.
//!
//! This crate provides a helper function to locate ELF binaries that are
//! automatically built from the rust-test-program project during the build process.

use std::path::PathBuf;

/// Returns the path to a test program ELF file.
///
/// # Arguments
///
/// * `filename` - The name of the ELF file (e.g., "test_video_pattern")
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
/// let path = test_program_path("test_video_pattern")
///     .expect("Failed to find test_video_pattern");
/// ```
pub fn test_program_path(filename: &str) -> Result<PathBuf, String> {
    let out_dir = env!("OUT_DIR");
    let path = PathBuf::from(out_dir).join(filename);

    if path.exists() {
        Ok(path)
    } else {
        Err(format!(
            "Test program '{}' not found at {}",
            filename,
            path.display()
        ))
    }
}
