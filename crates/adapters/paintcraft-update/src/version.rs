//! Version ordering and the rules for which release may be offered.

use semver::Version;

use crate::error::{Error, Result};

/// Parses `1.2.3` or `1.2.3-rc.1`. A leading `v` is NOT accepted here (tags are stripped by the
/// caller) so an asset name and a tag are compared as the same canonical text.
pub fn parse(text: &str) -> Option<Version> {
    let version = Version::parse(text).ok()?;
    // Round trip: rejects forms semver tolerates but the packager never writes.
    (version.to_string() == text).then_some(version)
}

/// `v1.2.3` to `1.2.3`.
pub fn from_tag(tag: &str) -> Result<Version> {
    tag.strip_prefix('v')
        .and_then(parse)
        .ok_or_else(|| Error::BadRelease(format!("tag {tag:?} is not v<semver>")))
}

/// Whether `offered` may replace `current`: strictly newer by semver precedence, and a
/// prerelease is only offered to a prerelease build. Build metadata never decides.
pub fn is_offerable(current: &Version, offered: &Version) -> bool {
    if !offered.pre.is_empty() && current.pre.is_empty() {
        return false;
    }
    offered.cmp_precedence(current).is_gt()
}
