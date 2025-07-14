use std::env;

fn main() {
    println!("cargo:rerun-if-changed=assets/dark-icon.png");
    println!("cargo:rerun-if-changed=build.rs");
    
    // Set up platform-specific build configurations
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    
    match target_os.as_str() {
        "macos" => setup_macos(),
        "windows" => setup_windows(),
        "linux" => setup_linux(),
        _ => println!("cargo:warning=Unknown target OS: {}", target_os),
    }
}

fn setup_macos() {
    println!("cargo:rustc-env=MACOS_ICON=assets/dark-icon.png");
}

fn setup_windows() {
    // Windows icon will be embedded via winres crate
    let mut res = winres::WindowsResource::new();
    res.set_icon("assets/icon.ico");
    res.set_version_info(winres::VersionInfo::PRODUCTVERSION, 0x00010000);
    res.set_version_info(winres::VersionInfo::FILEVERSION, 0x00010000);
    res.set("CompanyName", "Genxster1998");
    res.set("FileDescription", "PixelDrain Client");
    res.set("FileVersion", "1.0.0.0");
    res.set("InternalName", "pixeldrain");
    res.set("LegalCopyright", "Copyright (c) 2025 Genxster1998");
    res.set("OriginalFilename", "pixeldrain.exe");
    res.set("ProductName", "PixelDrain");
    res.set("ProductVersion", "1.0.0.0");
    
    if let Err(e) = res.compile() {
        eprintln!("Error: {}", e);
    }
    
    println!("cargo:rustc-env=WINDOWS_ICON=assets/icon.ico");
}

fn setup_linux() {
    println!("cargo:rustc-env=LINUX_ICON=assets/dark-icon.png");
} 