fn main() {
    // The D3D12 Agility SDK exports (src/agility.rs): D3D12 finds them on
    // the executable. Only the MSVC linker takes /EXPORT on a binary.
    //
    // Only when asked for at build time (`VICTORY_AGILITY=1`): with the
    // exports and no `D3D12\D3D12Core.dll` beside the exe, device creation
    // fails (0x887E0003) on every system whose runtime is older than the
    // exported version, Windows 11 included. Without them the system's own
    // runtime is used, as before. Windows 10 needs them (and the folder).
    println!("cargo:rerun-if-env-changed=VICTORY_AGILITY");
    let wanted = std::env::var("VICTORY_AGILITY").is_ok_and(|v| v == "1");
    if wanted && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/EXPORT:D3D12SDKVersion,DATA");
        println!("cargo:rustc-link-arg-bins=/EXPORT:D3D12SDKPath,DATA");
    }
}
