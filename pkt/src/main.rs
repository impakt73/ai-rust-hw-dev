use clap::Parser;
use pkt::package_core;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "pkt",
    about = "Package a Quartus RBF and Analogue Pocket core definition into a deployable zip"
)]
struct Args {
    /// Path to the Quartus-generated .rbf file.
    rbf: PathBuf,
    /// Path to the Analogue Pocket core source directory containing core.json and related files.
    core_source: PathBuf,
    /// Output directory for the packaged core zip.
    output_dir: PathBuf,
}

fn main() {
    let args = Args::parse();
    match package_core(&args.rbf, &args.core_source, &args.output_dir) {
        Ok(output_zip) => {
            println!("Created Pocket core package: {}", output_zip.display());
        }
        Err(error) => {
            eprintln!("Error: {error}");
            std::process::exit(1);
        }
    }
}
