//! Snapshot the maintainer-owned public key without ever creating or replacing it.
use std::{env, fs, path::PathBuf};
fn main() {
    let manifest =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo supplies manifest dir"));
    let key = manifest.join("../../../packaging/paintcraft-update-key.pub");
    println!("cargo:rerun-if-changed={}", key.display());
    let bytes = match fs::read(&key) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => panic!("cannot read update public key {}: {error}", key.display()),
    };
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"));
    fs::write(out.join("update-key.pub"), bytes).expect("write public-key build snapshot");
}
