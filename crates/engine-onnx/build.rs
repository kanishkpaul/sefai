//! ONNX Runtime's CoreML objects call `__isPlatformVersionAtLeast`.
//! Rust's standard library defines it, but release LTO drops that unreferenced definition,
//! so macOS runtime builds also link the toolchain's compiler runtime archive.

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var_os("CARGO_FEATURE_RUNTIME").is_none()
        || std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos")
    {
        return;
    }
    let output = Command::new("clang")
        .arg("--print-runtime-dir")
        .output()
        .expect("onnx macos builds require clang to locate its compiler runtime");
    assert!(output.status.success(), "clang --print-runtime-dir failed");
    let directory = String::from_utf8(output.stdout).expect("utf-8 runtime directory");
    println!("cargo:rustc-link-search=native={}", directory.trim());
    println!("cargo:rustc-link-lib=static=clang_rt.osx");
}
