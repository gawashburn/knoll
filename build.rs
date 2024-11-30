fn main() {
    println!("cargo::rustc-check-cfg=cfg(coverage_nightly)");
    println!(
        "cargo:rustc-link-search=framework={}",
        "/System/Library/Frameworks"
    );
    println!(
        "cargo:rustc-link-search=framework={}",
        "/System/Library/PrivateFrameworks"
    );
}
