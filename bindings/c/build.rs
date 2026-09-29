//! Gives the shared library a location-independent name so programs that link
//! it by path (as CMake does) record `libsyntaxmate.so`, not the path, and
//! find it through their rpath or the system search path.
//!
//! The SONAME is unversioned: 0.x releases make no ABI promise between minor
//! versions, so a `.so.0` name would claim a compatibility it does not have.

use std::env;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let vendor = env::var("CARGO_CFG_TARGET_VENDOR").unwrap_or_default();
    if vendor == "apple" {
        // Without this, the install name is the absolute path in `target/`.
        println!("cargo:rustc-cdylib-link-arg=-Wl,-install_name,@rpath/libsyntaxmate.dylib");
    } else if matches!(
        os.as_str(),
        "linux" | "android" | "freebsd" | "netbsd" | "openbsd" | "dragonfly"
    ) {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,libsyntaxmate.so");
    }
}
