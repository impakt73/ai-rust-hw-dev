use marlin::Builder;

fn main() {
    // Tell Cargo to rerun this build script if RTL files change
    println!("cargo:rerun-if-changed=../rtl");
    
    // Configure the builder to include the RTL directory
    let mut builder = Builder::default();
    builder.include_dir("../rtl");
    builder.build();
}
