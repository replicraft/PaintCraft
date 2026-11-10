//! Shared test helpers: the TEST-ONLY signer, fixtures and a tiny local HTTP server. Nothing here
//! touches the network or anything outside private temp directories.
#![allow(dead_code)]

pub mod server;
pub mod zips;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use blake2::{Blake2b512, Digest};
use ed25519_dalek::{Signer, SigningKey};

pub const KEY_ID: [u8; 8] = *b"RLYTEST1";

pub fn signing_key() -> SigningKey {
    let seed = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/TEST-ONLY-update-key.seed"
    ))
    .expect("seed fixture");
    let bytes: [u8; 32] = hex::decode(seed.trim())
        .expect("hex")
        .try_into()
        .expect("32 bytes");
    SigningKey::from_bytes(&bytes)
}

/// A minisign signature file (prehashed, as the minisign tool writes), made with `key`.
pub fn sign_with(
    key: &SigningKey,
    key_id: [u8; 8],
    payload: &[u8],
    trusted_comment: &str,
) -> String {
    let mut hasher = Blake2b512::new();
    hasher.update(payload);
    let digest = hasher.finalize();
    let signature = key.sign(&digest).to_bytes();
    let mut blob = b"ED".to_vec();
    blob.extend_from_slice(&key_id);
    blob.extend_from_slice(&signature);
    let mut global = signature.to_vec();
    global.extend_from_slice(trusted_comment.as_bytes());
    let global_signature = key.sign(&global).to_bytes();
    format!(
        "untrusted comment: signature from test key\n{}\ntrusted comment: {trusted_comment}\n{}\n",
        STANDARD.encode(blob),
        STANDARD.encode(global_signature)
    )
}

pub fn sign(payload: &[u8], trusted_comment: &str) -> String {
    sign_with(&signing_key(), KEY_ID, payload, trusted_comment)
}

/// The signature the release pipeline would publish for `asset` of `version`.
pub fn sign_asset(payload: &[u8], version: &str, asset: &str) -> String {
    sign(payload, &format!("paintcraft-update {version} {asset}"))
}

use std::path::{Path, PathBuf};

use paintcraft_update::{Arch, Endpoint, InstallKind, Os, Platform, Updater};

pub const REPO: &str = "replicraft/PaintCraft";

/// An endpoint on the test server; only 127.0.0.1 is allowed.
pub fn endpoint(base: &str) -> Endpoint {
    Endpoint {
        api_base: base.to_owned(),
        web_base: base.to_owned(),
        hosts: vec!["127.0.0.1".to_owned()],
        https_only: false,
    }
}

pub fn linux_platform(appimage: &Path) -> Platform {
    Platform {
        os: Os::Linux,
        arch: Arch::X64,
        install: InstallKind::LinuxAppImage {
            path: appimage.to_owned(),
        },
    }
}

pub fn updater(current: &str, platform: Platform, base: &str) -> Updater {
    Updater::with_endpoint(current, platform, endpoint(base)).expect("updater")
}

/// The JSON GitHub returns for a release whose assets are `(name, size)`.
pub fn release_json(base: &str, tag: &str, prerelease: bool, assets: &[(&str, usize)]) -> String {
    let list: Vec<String> = assets
		.iter()
		.map(|(name, size)| {
			format!(
				r#"{{"name":"{name}","size":{size},"state":"uploaded","browser_download_url":"{base}/{REPO}/releases/download/{tag}/{name}"}}"#
			)
		})
		.collect();
    format!(
        r#"{{"tag_name":"{tag}","draft":false,"prerelease":{prerelease},"html_url":"{base}/{REPO}/releases/tag/{tag}","assets":[{}]}}"#,
        list.join(",")
    )
}

/// Publishes a signed AppImage release on `server` and returns the payload.
pub fn publish_appimage(server: &server::Server, version: &str, payload: &[u8]) -> String {
    let name = format!("paintcraft-{version}-linux-x64.AppImage");
    let signature = sign_asset(payload, version, &name);
    let tag = format!("v{version}");
    let json = release_json(
        &server.base,
        &tag,
        version.contains('-'),
        &[
            (&name, payload.len()),
            (&format!("{name}.minisig"), signature.len()),
        ],
    );
    server.route(
        &format!("/repos/{REPO}/releases/latest"),
        server::Reply::ok(json),
    );
    server.route(
        &format!("/{REPO}/releases/download/{tag}/{name}"),
        server::Reply::ok(payload),
    );
    server.route(
        &format!("/{REPO}/releases/download/{tag}/{name}.minisig"),
        server::Reply::ok(signature),
    );
    name
}

pub fn tempdir() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

pub fn write(path: impl AsRef<Path>, bytes: &[u8]) -> PathBuf {
    std::fs::write(path.as_ref(), bytes).expect("write");
    path.as_ref().to_owned()
}
