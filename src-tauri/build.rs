fn main() {
    // SDL3 for Proton's controller bus (scripts/build-sdl3.sh): carried inside the launcher when it was built first
    println!("cargo:rustc-check-cfg=cfg(bundled_sdl3)");
    println!("cargo:rerun-if-changed=../build/sdl3/libSDL3.so.0");
    let sdl3 = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../build/sdl3/libSDL3.so.0");
    if sdl3.is_file() {
        println!("cargo:rustc-cfg=bundled_sdl3");
        println!("cargo:rustc-env=XC2L_SDL3={}", sdl3.canonicalize().unwrap().display());
    }
    tauri_build::build()
}
