fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("packaging/windows/icon.ico");
        res.set("ProductName", "scheme-handler");
        res.set("FileDescription", "scheme-handler");
        res.set("InternalName", "scheme-handler");
        res.set("OriginalFilename", "scheme-handler.exe");
        res.compile().expect("failed to compile Windows resources");
        println!("cargo:rerun-if-changed=packaging/windows/icon.ico");
    }

    #[cfg(target_os = "macos")]
    {
        cc::Build::new()
            .file("macos/url_handler.m")
            .flag("-fobjc-arc")
            .compile("url_handler_macos");

        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rerun-if-changed=macos/url_handler.m");
        println!("cargo:rerun-if-changed=packaging/macos/Info.plist");
        println!("cargo:rerun-if-changed=packaging/macos/icon.icns");
    }
}
