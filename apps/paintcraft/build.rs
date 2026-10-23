//! Name unbundled macOS runs independently of executable and document titles.
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=resources/macos/Info.plist.in");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let version = env::var("CARGO_PKG_VERSION").expect("Cargo supplies package version");
    let template = fs::read_to_string("resources/macos/Info.plist.in")
        .expect("read macOS application metadata template");
    let plist = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies build output"))
        .join("paintcraft-info.plist");
    fs::write(&plist, template.replace("@VERSION@", &version))
        .expect("write versioned macOS application metadata");
    // Apple CoreFoundation reads this section for an unbundled Mach-O executable.
    // Pass arguments separately so checkout/output paths may contain spaces or commas.
    for argument in ["-sectcreate", "__TEXT", "__info_plist"] {
        println!("cargo:rustc-link-arg-bin=paintcraft=-Xlinker");
        println!("cargo:rustc-link-arg-bin=paintcraft={argument}");
    }
    println!("cargo:rustc-link-arg-bin=paintcraft=-Xlinker");
    println!("cargo:rustc-link-arg-bin=paintcraft={}", plist.display());
}
