//! The ONNX Runtime WebGPU archive links Dawn as a shared library that `ort` copies beside the executable.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var_os("CARGO_FEATURE_ONNX_WEBGPU").is_none() {
        return;
    }
    match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("macos") => println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path"),
        Ok("linux") => println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN"),
        _ => {}
    }
}
