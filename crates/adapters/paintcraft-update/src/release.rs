//! The GitHub "latest release" document: parsing it, and choosing the one asset to install.

use semver::Version;
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::http::Endpoint;
use crate::platform::Platform;
use crate::version;

/// Largest asset we ever download.
pub const MAX_ASSET_BYTES: u64 = 300 * 1024 * 1024;
/// A minisign signature file is about 300 bytes.
pub const MAX_SIGNATURE_BYTES: u64 = 4 * 1024;
/// Largest release document we parse.
pub const MAX_API_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Deserialize)]
pub struct ReleaseDoc {
    pub tag_name: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    pub html_url: String,
    #[serde(default)]
    pub assets: Vec<AssetDoc>,
}

#[derive(Debug, Deserialize)]
pub struct AssetDoc {
    pub name: String,
    pub size: u64,
    pub browser_download_url: String,
    #[serde(default)]
    pub state: Option<String>,
}

/// One downloadable file of a release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    /// Size the release lists; the download must match it exactly.
    pub size: u64,
    pub url: String,
}

/// The chosen installer and its detached signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub asset: Asset,
    pub signature: Asset,
}

/// Where a release's download must be: `<web>/<repo>/releases/download/<tag>/<name>`.
fn expected_url(endpoint: &Endpoint, repository: &str, tag: &str, name: &str) -> String {
    format!(
        "{}/{repository}/releases/download/{tag}/{name}",
        endpoint.web_base
    )
}

pub fn parse(json: &[u8]) -> Result<ReleaseDoc> {
    serde_json::from_slice(json).map_err(|e| Error::BadRelease(e.to_string()))
}

#[derive(Debug)]
pub struct Selection {
    pub version: Version,
    pub release_url: String,
    /// `None` when the platform only gets a link (manual install).
    pub package: Option<Package>,
}

/// Applies every acceptance rule to a release: not a draft, tag is `v<semver>`, strictly newer
/// (and not a prerelease for a stable build), then the exact asset name for this platform.
/// `Ok(None)` means "nothing to offer".
pub fn select(
    doc: &ReleaseDoc,
    current: &Version,
    platform: &Platform,
    endpoint: &Endpoint,
    repository: &str,
) -> Result<Option<Selection>> {
    if doc.draft {
        return Ok(None);
    }
    let offered = version::from_tag(&doc.tag_name)?;
    if doc.prerelease != !offered.pre.is_empty() {
        return Err(Error::BadRelease(
            "prerelease flag disagrees with the tag".into(),
        ));
    }
    if !version::is_offerable(current, &offered) {
        return Ok(None);
    }
    let release_url = release_page(doc, endpoint, repository);
    let package = match platform.asset_name(&offered) {
        None => None,
        Some(name) => {
            let Some(asset) = find(doc, endpoint, repository, &name)? else {
                return Err(Error::AssetMissing(name));
            };
            if asset.size == 0 || asset.size > MAX_ASSET_BYTES {
                return Err(Error::BadRelease(format!(
                    "asset {name} has size {}",
                    asset.size
                )));
            }
            let sig_name = format!("{name}.minisig");
            let signature = find(doc, endpoint, repository, &sig_name)?
                .ok_or(Error::SignatureMissing(sig_name))?;
            if signature.size == 0 || signature.size > MAX_SIGNATURE_BYTES {
                return Err(Error::BadRelease(
                    "signature file has an implausible size".into(),
                ));
            }
            Some(Package { asset, signature })
        }
    };
    Ok(Some(Selection {
        version: offered,
        release_url,
        package,
    }))
}

fn find(
    doc: &ReleaseDoc,
    endpoint: &Endpoint,
    repository: &str,
    name: &str,
) -> Result<Option<Asset>> {
    let mut hits = doc.assets.iter().filter(|a| a.name == name);
    let Some(first) = hits.next() else {
        return Ok(None);
    };
    if hits.next().is_some() {
        return Err(Error::BadRelease(format!("asset {name} is listed twice")));
    }
    if first.state.as_deref().is_some_and(|s| s != "uploaded") {
        return Ok(None);
    }
    // The URL is rebuilt from the tag and the name, never taken from the document: a release
    // that points an asset somewhere else is refused instead of followed.
    let url = expected_url(endpoint, repository, &doc.tag_name, name);
    if first.browser_download_url != url {
        return Err(Error::BadRelease(format!(
            "asset {name} points outside this repository's release"
        )));
    }
    Ok(Some(Asset {
        name: name.to_owned(),
        size: first.size,
        url,
    }))
}

fn release_page(doc: &ReleaseDoc, endpoint: &Endpoint, repository: &str) -> String {
    let prefix = format!("{}/{repository}/releases/", endpoint.web_base);
    if doc.html_url.starts_with(&prefix) && doc.html_url.chars().all(|c| c.is_ascii_graphic()) {
        doc.html_url.clone()
    } else {
        format!("{prefix}tag/{}", doc.tag_name)
    }
}
