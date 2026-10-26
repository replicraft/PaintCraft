//! Optional pinned C decoder build. Default packaged helpers are unchanged.
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};
const HASH: &str = "9edb11a2108b375cc58370354e705feebc93430bb780130363815c1e1ac0c250";
fn run(command: &mut Command) {
    let status = command
        .status()
        .unwrap_or_else(|e| panic!("Cannot launch {command:?}: {e}"));
    assert!(status.success(), "Native codec build failed: {command:?}");
}
fn changed(path: &Path, contents: &str) {
    if fs::read_to_string(path).ok().as_deref() != Some(contents) {
        fs::write(path, contents).unwrap();
    }
}
fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_AVIF_CODEC");
    println!("cargo:rerun-if-env-changed=CC");
    if env::var_os("CARGO_FEATURE_AVIF_CODEC").is_none() {
        return;
    }
    assert_eq!(
        env::var("HOST").unwrap(),
        env::var("TARGET").unwrap(),
        "AVIF native cross-compilation is not qualified yet"
    );
    let archive = PathBuf::from("vendor/dav1d/dav1d-1.5.4.tar.gz");
    println!("cargo:rerun-if-changed={}", archive.display());
    println!("cargo:rerun-if-changed=native/dav1d_mem.h");
    println!("cargo:rerun-if-changed=native/av1_bridge.c");
    let source = fs::read(&archive).unwrap();
    assert_eq!(
        Sha256::digest(&source)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        HASH,
        "Pinned dav1d source checksum mismatch"
    );
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let root = out.join("dav1d-54706fc6bc0cdecab7e9593974a4039cc038fca7");
    if !root.exists() {
        // Only fixed-hash regular files/directories are extracted. No symlink,
        // absolute or parent-relative archive member can escape OUT_DIR.
        let python = if cfg!(windows) { "python" } else { "python3" };
        run(Command::new(python).arg("-c").arg("import pathlib,sys,tarfile\nwith tarfile.open(sys.argv[1]) as t:\n m=t.getmembers()\n for e in m:\n  p=pathlib.PurePosixPath(e.name)\n  assert not p.is_absolute() and '..' not in p.parts and (e.isfile() or e.isdir())\n t.extractall(sys.argv[2],members=m,filter='data')").arg(&archive).arg(&out));
    }
    let header = root.join("src/mem.h");
    let pristine = out.join("dav1d-1.5.4-pristine-mem.h");
    if !pristine.exists() {
        fs::copy(&header, &pristine).unwrap();
    }
    let pristine_bytes = fs::read(&pristine).unwrap();
    assert_eq!(
        Sha256::digest(&pristine_bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        "f4b9be308b6a310af5bd28330589343a5fcf29848140d5bae38a6f71d7366174",
        "Pinned pristine allocation header mismatch"
    );
    let upstream = String::from_utf8(pristine_bytes).unwrap();
    let upstream = upstream.replace(
        "#if TRACK_HEAP_ALLOCATIONS\n    enum AllocationType type;\n#endif",
        "    enum AllocationType type;",
    );
    let start = upstream
        .find("#else\n#define dav1d_mem_pool_init")
        .expect("Pinned allocation branch");
    let end = upstream[start..]
        .find("#endif /* TRACK_HEAP_ALLOCATIONS */")
        .unwrap()
        + start;
    let replacement = fs::read_to_string("native/dav1d_mem.h").unwrap();
    changed(
        &header,
        &format!("{}{}{}", &upstream[..start], replacement, &upstream[end..]),
    );
    let mem_c = root.join("src/mem.c");
    let pristine_c = out.join("dav1d-1.5.4-pristine-mem.c");
    if !pristine_c.exists() {
        fs::copy(&mem_c, &pristine_c).unwrap();
    }
    let bytes_c = fs::read(&pristine_c).unwrap();
    assert_eq!(
        Sha256::digest(&bytes_c)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        "6b2824cea459be669aeaa9279cd855161e70388bfb084980649c9ef7a824f800",
        "Pinned pristine allocation source mismatch"
    );
    let source_c = String::from_utf8(bytes_c).unwrap().replace(
        "#if TRACK_HEAP_ALLOCATIONS\n            pool->type = type;\n#endif",
        "            pool->type = type;",
    );
    changed(&mem_c, &source_c);
    let ref_h = root.join("src/ref.h");
    let pristine_ref = out.join("dav1d-1.5.4-pristine-ref.h");
    if !pristine_ref.exists() {
        fs::copy(&ref_h, &pristine_ref).unwrap();
    }
    let ref_bytes = fs::read(&pristine_ref).unwrap();
    assert_eq!(
        Sha256::digest(&ref_bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        "c03e37fba41510c3c24a5b0158f5597378e05493ba7068e1d8d9c5eca1d36f18",
        "Pinned pristine reference header mismatch"
    );
    let ref_source = String::from_utf8(ref_bytes).unwrap().replace("#if !TRACK_HEAP_ALLOCATIONS\n#define dav1d_ref_create(type, size) dav1d_ref_create(size)\n#endif", "/* PaintCraft preserves allocation type arguments without statistics. */");
    changed(&ref_h, &ref_source);
    let bridge = fs::read_to_string("native/av1_bridge.c").unwrap();
    changed(&root.join("src/paintcraft_bridge.c"), &bridge);
    let meson_path = root.join("src/meson.build");
    let mut meson = fs::read_to_string(&meson_path).unwrap();
    if !meson.contains("'paintcraft_bridge.c'") {
        meson = meson.replacen(
            "libdav1d_sources = files(",
            "libdav1d_sources = files(\n    'paintcraft_bridge.c',",
            1,
        );
        changed(&meson_path, &meson);
    }
    let build = out.join("dav1d-build");
    let compiler = env::var("CC").unwrap_or_else(|_| {
        if cfg!(windows) {
            "cl".into()
        } else {
            "cc".into()
        }
    });
    let compiler_marker = build.join(".paintcraft-compiler");
    if build.exists() && fs::read_to_string(&compiler_marker).ok().as_deref() != Some(&compiler) {
        fs::remove_dir_all(&build).unwrap();
    }
    let mut setup = Command::new("meson");
    setup.env("CC", &compiler).arg("setup");
    if build.exists() {
        setup.arg("--reconfigure");
    }
    setup.arg(&build).arg(&root).args([
        "--backend=ninja",
        "--wrap-mode=nodownload",
        "--buildtype=release",
        "-Ddefault_library=static",
        "-Denable_asm=false",
        "-Denable_tools=false",
        "-Denable_tests=false",
        "-Denable_examples=false",
        "-Dbitdepths=8",
    ]);
    run(&mut setup);
    changed(&compiler_marker, &compiler);
    run(Command::new("ninja")
        .arg("-C")
        .arg(&build)
        .args(["-j", "4"]));
    println!(
        "cargo:rustc-link-search=native={}",
        build.join("src").display()
    );
    println!("cargo:rustc-link-lib=static=dav1d");
    if cfg!(unix) {
        println!("cargo:rustc-link-lib=pthread");
    }
}
