//! The one HTTP client. Every request, and every redirect hop, is checked against the host
//! allowlist before any byte is sent; bodies are read through a hard cap.

use std::io::Read;
use std::ops::ControlFlow;
use std::time::Duration;

use ureq::http::Uri;

use crate::error::{Error, Result};

const MAX_REDIRECTS: usize = 5;
const CHUNK: usize = 64 * 1024;

/// Hosts that may be contacted in release builds: the GitHub API, the web host that answers
/// `releases/download/...`, and the hosts GitHub redirects release assets to (the current
/// `release-assets.githubusercontent.com`; `objects.githubusercontent.com` served them before 2025
/// and still appears for older releases).
pub const GITHUB_HOSTS: [&str; 4] = [
    "api.github.com",
    "github.com",
    "release-assets.githubusercontent.com",
    "objects.githubusercontent.com",
];

/// Where the updater talks to. [`Endpoint::github`] is the only production value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// Base of the REST API (`https://api.github.com`).
    pub api_base: String,
    /// Base of the web host that serves release downloads (`https://github.com`).
    pub web_base: String,
    pub hosts: Vec<String>,
    /// Refuse anything but HTTPS on port 443. Always true outside tests.
    pub https_only: bool,
}

impl Endpoint {
    pub fn github() -> Self {
        Self {
            api_base: "https://api.github.com".into(),
            web_base: "https://github.com".into(),
            hosts: GITHUB_HOSTS.iter().map(|h| (*h).to_owned()).collect(),
            https_only: true,
        }
    }
}

/// Checks one URL (a first request or a redirect target). `previous_https` is whether the hop
/// that led here was HTTPS: leaving HTTPS is refused even when the policy would allow HTTP.
pub fn check_url(endpoint: &Endpoint, url: &str, previous_https: bool) -> Result<Uri> {
    let uri: Uri = url
        .parse()
        .map_err(|_| Error::HostNotAllowed(String::new()))?;
    let https = match uri.scheme_str() {
        Some("https") => true,
        Some("http") => false,
        _ => return Err(Error::InsecureUrl),
    };
    if (endpoint.https_only || previous_https) && !https {
        return Err(Error::InsecureUrl);
    }
    let authority = uri.authority().ok_or(Error::InsecureUrl)?;
    let host = authority.host().to_ascii_lowercase();
    if authority.as_str().contains('@') || !endpoint.hosts.contains(&host) {
        return Err(Error::HostNotAllowed(host));
    }
    if endpoint.https_only && authority.port_u16().is_some_and(|port| port != 443) {
        return Err(Error::HostNotAllowed(host));
    }
    Ok(uri)
}

/// Receives each chunk and the announced length; `Break` stops the transfer.
pub type Sink<'a> = dyn FnMut(&[u8], Option<u64>) -> ControlFlow<()> + 'a;

#[derive(Debug)]
pub struct Client {
    agent: ureq::Agent,
    endpoint: Endpoint,
}

impl Client {
    pub fn new(endpoint: Endpoint, user_agent: &str) -> Self {
        let config = ureq::Agent::config_builder()
            // Redirects are followed here so each hop is checked.
            .max_redirects(0)
            .max_redirects_will_error(false)
            .https_only(endpoint.https_only)
            .http_status_as_error(false)
            .user_agent(user_agent)
            .accept_encoding("identity")
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_recv_response(Some(Duration::from_secs(30)))
            .timeout_per_read(Some(Duration::from_secs(30)))
            .timeout_global(Some(Duration::from_secs(45 * 60)))
            .build();
        Self {
            agent: config.new_agent(),
            endpoint,
        }
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    /// GETs `url` and hands the body to `sink` in chunks. The sink sees `(chunk, content_length)` and
    /// may stop the transfer. Returns the number of body bytes. More than `cap` bytes is an
    /// error, whatever the server announces.
    pub fn get(&self, url: &str, accept: &str, cap: u64, sink: &mut Sink<'_>) -> Result<u64> {
        let mut current = url.to_owned();
        // The first hop has no predecessor to downgrade from; the endpoint policy covers it.
        let mut https = false;
        for _ in 0..=MAX_REDIRECTS {
            let uri = check_url(&self.endpoint, &current, https)?;
            https = uri.scheme_str() == Some("https");
            let mut response = self
                .agent
                .get(uri.to_string())
                .header("Accept", accept)
                .header("X-GitHub-Api-Version", "2022-11-28")
                .call()
                .map_err(|e| Error::Network(redact(&e.to_string())))?;
            let status = response.status().as_u16();
            if matches!(status, 301 | 302 | 303 | 307 | 308) {
                current = response
                    .headers()
                    .get("location")
                    .and_then(|v| v.to_str().ok())
                    .ok_or_else(|| Error::Network("redirect without a location".into()))?
                    .to_owned();
                continue;
            }
            if status != 200 {
                return Err(Error::HttpStatus(status));
            }
            let announced = response.body().content_length();
            if announced.is_some_and(|n| n > cap) {
                return Err(Error::ResponseTooLarge { limit: cap });
            }
            let mut reader = response.body_mut().as_reader();
            let mut buffer = vec![0u8; CHUNK];
            let mut total = 0u64;
            loop {
                let n = reader
                    .read(&mut buffer)
                    .map_err(|e| Error::Network(redact(&e.to_string())))?;
                if n == 0 {
                    return Ok(total);
                }
                total += n as u64;
                if total > cap {
                    return Err(Error::ResponseTooLarge { limit: cap });
                }
                if sink(&buffer[..n], announced).is_break() {
                    return Err(Error::Cancelled);
                }
            }
        }
        Err(Error::TooManyRedirects)
    }

    /// Small body into memory under `cap`.
    pub fn get_bytes(&self, url: &str, accept: &str, cap: u64) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        self.get(url, accept, cap, &mut |chunk, _| {
            out.extend_from_slice(chunk);
            ControlFlow::Continue(())
        })?;
        Ok(out)
    }
}

/// Error text from the HTTP stack can echo a URL; drop everything from a `?` to the next space so a
/// signed query string never reaches a log or the UI.
fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut skipping = false;
    for c in text.chars() {
        if c == '?' {
            skipping = true;
        } else if skipping && c.is_whitespace() {
            skipping = false;
        }
        if !skipping {
            out.push(c);
        }
    }
    out
}
