//! Minisign verification against the compiled-in public key.

use minisign_verify::{PublicKey, Signature, StreamVerifier};
use semver::Version;

use crate::error::{Error, Result};

/// The release public key, as the text of a minisign `.pub` file. This is the ONLY key the
/// updater trusts; nothing at run time can change it.
pub const PUBLIC_KEY: &str = include_str!(concat!(env!("OUT_DIR"), "/update-key.pub"));

/// What the trusted comment of the signature of `asset` of `version` must say. It binds the
/// signature to one version and one file name, so a validly signed file cannot be replayed as
/// another version or another platform's asset.
pub fn trusted_comment(version: &Version, asset_name: &str) -> String {
    format!("paintcraft-update {version} {asset_name}")
}

pub fn public_key() -> Result<PublicKey> {
    if PUBLIC_KEY.trim().is_empty() {
        return Err(Error::SigningKeyUnconfigured);
    }
    decode_key(PUBLIC_KEY)
}

pub(crate) fn decode_key(text: &str) -> Result<PublicKey> {
    PublicKey::decode(text).map_err(|e| Error::SignatureInvalid(format!("bad public key: {e}")))
}

/// The TEST-ONLY key. Only compiled for tests; production code cannot reach it.
#[cfg(any(test, feature = "test-util"))]
pub const TEST_ONLY_PUBLIC_KEY: &str = include_str!("../tests/fixtures/TEST-ONLY-update-key.pub");

/// Decodes a `.minisig` and checks its trusted comment text (the cryptographic check of that
/// comment happens in [`finish`]).
pub fn parse_signature(raw: &[u8], version: &Version, asset_name: &str) -> Result<Signature> {
    let text = std::str::from_utf8(raw).map_err(|_| Error::SignatureInvalid("not text".into()))?;
    let signature = Signature::decode(text).map_err(|e| Error::SignatureInvalid(e.to_string()))?;
    let expected = trusted_comment(version, asset_name);
    if signature.trusted_comment() != expected {
        return Err(Error::TrustedCommentMismatch {
            expected,
            found: signature.trusted_comment().chars().take(200).collect(),
        });
    }
    Ok(signature)
}

/// Starts hashing for a streamed payload. Legacy (non-prehashed) signatures are refused.
pub fn start<'a>(key: &'a PublicKey, signature: &'a Signature) -> Result<StreamVerifier<'a>> {
    key.verify_stream(signature)
        .map_err(|e| Error::SignatureInvalid(e.to_string()))
}

/// Completes verification: signature over the payload hash and over the trusted comment.
pub fn finish(mut verifier: StreamVerifier<'_>) -> Result<()> {
    verifier
        .finalize()
        .map_err(|e| Error::SignatureInvalid(e.to_string()))
}

/// Verifies a whole in-memory payload (used by tests and small files).
pub fn verify_bytes(
    key: &PublicKey,
    payload: &[u8],
    raw_signature: &[u8],
    version: &Version,
    asset_name: &str,
) -> Result<()> {
    let signature = parse_signature(raw_signature, version, asset_name)?;
    let mut verifier = start(key, &signature)?;
    verifier.update(payload);
    finish(verifier)
}
