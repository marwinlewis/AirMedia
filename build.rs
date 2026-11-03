fn main() {
    #[cfg(windows)]
    {
        // Only embed Windows application manifest and PE version resources for release builds
        // to keep test harnesses lightweight and avoid Windows Defender cloud-lookup delays during testing.
        let is_release = std::env::var("PROFILE").map(|p| p == "release").unwrap_or(false);
        if is_release {
            let mut res = winres::WindowsResource::new();
            res.set_icon("assets/windows/icon.ico");
            res.set("FileDescription", env!("CARGO_PKG_DESCRIPTION"));
            res.set("ProductName", "AirMedia");
            res.set("OriginalFilename", "airmedia.exe");
            res.set("LegalCopyright", "Copyright (C) 2026 AirMedia Contributors");
            res.set("CompanyName", env!("CARGO_PKG_AUTHORS"));
            res.set("InternalName", env!("CARGO_PKG_NAME"));
            res.set_manifest_file("assets/windows/airmedia.manifest");
            if let Err(e) = res.compile() {
                eprintln!("cargo:warning=Failed to compile Windows resources: {e}");
            }
        }
    }
}
