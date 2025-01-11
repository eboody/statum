// build.rs in your proc-macro crate
fn main() {
    eprintln!(">>> Build script for statum: I am running!");

    // If your Cargo is 1.80 or newer, you can do:
    // println!("cargo:rustc-check-cfg=names(feature)");
    // println!("cargo:rustc-check-cfg=values(feature,\"serde\")");

    // Otherwise, use the older single-line syntax:
    println!("cargo:rustc-check-cfg=cfg(feature, values(\"serde\"))");
}
