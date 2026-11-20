//! Embeds the delivery version. The CI sets `MACROHUD_VERSION` (0.1.<run number>); a local build
//! says it is a development build (0.1.0-dev). On Windows, also embeds the version information that
//! Explorer shows (file properties), and the icon if the HUD provides one (`src/hud/icone.ico`).

fn main() {
    println!("cargo:rerun-if-env-changed=MACROHUD_VERSION");
    let version = match std::env::var("MACROHUD_VERSION") {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => format!("{}-dev", std::env::var("CARGO_PKG_VERSION").unwrap_or_default()),
    };
    println!("cargo:rustc-env=MACROHUD_VERSION={version}");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        windows_resources(&version);
    }
}

/// Version information (and icon) of `macrohud.exe`, compiled by `rc.exe`.
#[cfg(windows)]
fn windows_resources(version: &str) {
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("set by Cargo"));
    let hud_dir = manifest.join("src").join("hud");
    println!("cargo:rerun-if-changed={}", hud_dir.display());
    let icon = hud_dir.join("icone.ico");

    // 0.1.12 or 0.1.0-dev -> 0.1.12.0 or 0.1.0.0
    let mut numbers = version.split(['.', '-']).map(|n| n.parse::<u64>().unwrap_or(0)).chain(std::iter::repeat(0));
    let packed = (0..4).fold(0u64, |acc, _| (acc << 16) | (numbers.next().unwrap_or(0) & 0xFFFF));

    let mut res = winresource::WindowsResource::new();
    if icon.is_file() {
        res.set_icon(&icon.to_string_lossy());
    }
    res.set_language(0x040C) // French (France)
        .set_version_info(winresource::VersionInfo::FILEVERSION, packed)
        .set_version_info(winresource::VersionInfo::PRODUCTVERSION, packed)
        .set("FileVersion", version)
        .set("ProductVersion", version)
        .set("ProductName", "MacroHUD")
        .set("FileDescription", "MacroHUD, macros clavier et souris")
        .set("CompanyName", "MacroHUD")
        .set("OriginalFilename", "macrohud.exe")
        .set("InternalName", "macrohud");
    res.compile().expect("Windows resources (version) compiled by rc.exe");
}

#[cfg(not(windows))]
fn windows_resources(_version: &str) {
    println!("cargo:warning=build for Windows from another system: macrohud.exe gets no version resource");
}
