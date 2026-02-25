fn main() {
    // Link mscms.dll for WCS color profile APIs
    println!("cargo:rustc-link-lib=mscms");
    // Link user32.dll for device notification and window APIs
    println!("cargo:rustc-link-lib=user32");
}
