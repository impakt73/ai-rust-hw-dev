use clap::Parser;
use pkt::package_core;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(
    name = "pkt",
    about = "Package a Quartus RBF and Analogue Pocket core definition into a deployable zip"
)]
struct Args {
    /// Write the generated zip path to a file for machine-readable consumers.
    #[arg(long)]
    output_path_file: Option<PathBuf>,
    /// Path to the Quartus-generated .rbf file.
    rbf: PathBuf,
    /// Path to the Analogue Pocket core source directory containing core.json and related files.
    core_source: PathBuf,
    /// Output directory for the packaged core zip.
    output_dir: PathBuf,
}

fn main() {
    if let Err(error) = run(Args::parse()) {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let output_zip = package_core(&args.rbf, &args.core_source, &args.output_dir)?;
    if let Some(output_path_file) = args.output_path_file.as_ref() {
        write_output_path_file(output_path_file, &output_zip)?;
    }
    println!("Created Pocket core package: {}", output_zip.display());
    Ok(())
}

fn write_output_path_file(output_path_file: &Path, output_zip: &Path) -> std::io::Result<()> {
    if let Some(parent) = output_path_file.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    fs::write(output_path_file, format!("{}\n", output_zip.display()))
}

#[cfg(test)]
mod tests {
    use super::{write_output_path_file, Args};
    use clap::Parser;
    use tempfile::TempDir;

    #[test]
    fn parses_output_path_file_flag() {
        let args = Args::try_parse_from([
            "pkt",
            "--output-path-file",
            "/tmp/pocket-package-path.txt",
            "input.rbf",
            "core-source",
            "out-dir",
        ])
        .expect("parse args");

        assert_eq!(
            args.output_path_file.as_deref(),
            Some(std::path::Path::new("/tmp/pocket-package-path.txt"))
        );
    }

    #[test]
    fn writes_output_path_file_with_parent_directories() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let output_path_file = temp_dir.path().join("nested/package-path.txt");
        let output_zip = temp_dir.path().join("out/core.zip");

        write_output_path_file(&output_path_file, &output_zip).expect("write output path file");

        assert_eq!(
            std::fs::read_to_string(&output_path_file).expect("read output path file"),
            format!("{}\n", output_zip.display())
        );
    }
}
