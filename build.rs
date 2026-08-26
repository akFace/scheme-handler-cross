fn main() {
    #[cfg(target_os = "macos")]
    {
        cc::Build::new()
            .file("macos/url_handler.m")
            .flag("-fobjc-arc")
            .compile("url_handler_macos");

        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rerun-if-changed=macos/url_handler.m");
    }
}
