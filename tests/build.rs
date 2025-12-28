fn main() {
    // Tell Cargo to rerun this build script if RTL files change
    println!("cargo:rerun-if-changed=../rtl");
}
