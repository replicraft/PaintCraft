//! Host allowlist, redirects, size caps and the no-override guarantee.

use paintcraft_update::http::{GITHUB_HOSTS, check_url};
use paintcraft_update::{Arch, Endpoint, Error, InstallKind, ManualReason, Os, Platform, Updater};

mod support;
use support::server::{Reply, Server};
use support::{REPO, release_json};

fn manual_platform() -> Platform {
    Platform {
        os: Os::Linux,
        arch: Arch::X64,
        install: InstallKind::Manual(ManualReason::Packaged),
    }
}

#[test]
fn production_endpoint_is_the_github_constant() {
    if paintcraft_update::verify::PUBLIC_KEY.trim().is_empty() {
        assert!(matches!(
            Updater::new("0.1.0", manual_platform()),
            Err(Error::SigningKeyUnconfigured)
        ));
        return;
    }
    let updater = Updater::new("0.1.0", manual_platform()).unwrap();
    let endpoint = updater.endpoint();
    assert_eq!(endpoint, &Endpoint::github());
    assert_eq!(endpoint.api_base, "https://api.github.com");
    assert_eq!(endpoint.web_base, "https://github.com");
    assert!(endpoint.https_only);
    assert_eq!(
        endpoint.hosts,
        [
            "api.github.com",
            "github.com",
            "release-assets.githubusercontent.com",
            "objects.githubusercontent.com"
        ]
    );
    assert_eq!(GITHUB_HOSTS.len(), 4);
    assert_eq!(paintcraft_update::REPOSITORY, "replicraft/PaintCraft");
    assert_eq!(
        env!("CARGO_PKG_REPOSITORY"),
        format!("https://github.com/{}", paintcraft_update::REPOSITORY)
    );
}

/// The library reads the environment only to detect how it was installed. A new `env::var` anywhere
/// else would be a possible override and must be a conscious decision.
#[test]
fn no_source_file_reads_the_environment_except_detection() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    for entry in walk(&src) {
        let text = std::fs::read_to_string(&entry).unwrap();
        let name = entry.file_name().unwrap().to_string_lossy().into_owned();
        let reads = text.matches("env::var").count() + text.matches("option_env!").count();
        // platform.rs: $APPIMAGE and the Program Files folders; helper.rs: %SystemRoot% to find
        // powershell.exe. The helper's PAINTCRAFT_UPDATE_* variables are only ever set on the child.
        let allowed = match name.as_str() {
            "platform.rs" => 2,
            "helper.rs" => 1,
            "install_lock.rs" => 3,
            _ => 0,
        };
        let sets_own = text.matches("env::set_var").count();
        if reads > allowed || sets_own > 0 {
            offenders.push(name);
        }
    }
    assert!(offenders.is_empty(), "{offenders:?}");
}

fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out
}

#[test]
fn only_github_hosts_over_https_pass() {
    let e = Endpoint::github();
    for url in [
        "https://api.github.com/repos/x/y/releases/latest",
        "https://github.com/replicraft/PaintCraft/releases/download/v1.0.0/a.zip",
        "https://release-assets.githubusercontent.com/github-production-release-asset/1/2?sp=r&sig=abc",
        "https://objects.githubusercontent.com/x",
        "https://API.GITHUB.COM/x",
        "https://api.github.com:443/x",
    ] {
        assert!(check_url(&e, url, true).is_ok(), "{url}");
    }
    for url in [
        "https://evil.example/x",
        "https://github.com.evil.example/x",
        "https://evilgithub.com/x",
        "https://githubusercontent.com/x",
        "https://raw.githubusercontent.com/x",
        "https://sub.release-assets.githubusercontent.com/x",
        "https://api.github.com@evil.example/x",
        "https://evil.example@api.github.com/x",
        "https://api.github.com:8443/x",
        "http://api.github.com/x",
        "ftp://github.com/x",
        "file:///etc/passwd",
        "//github.com/x",
        "/relative",
        "https://127.0.0.1/x",
        "https://localhost/x",
    ] {
        assert!(check_url(&e, url, true).is_err(), "{url}");
    }
}

#[test]
fn leaving_https_is_refused_even_when_the_policy_allows_http() {
    let e = support::endpoint("http://127.0.0.1:1");
    assert!(check_url(&e, "http://127.0.0.1:1/x", false).is_ok());
    assert!(matches!(
        check_url(&e, "http://127.0.0.1:1/x", true),
        Err(Error::InsecureUrl)
    ));
}

fn latest_path() -> String {
    format!("/repos/{REPO}/releases/latest")
}

fn updater(server: &Server) -> Updater {
    support::updater("0.1.0", manual_platform(), &server.base)
}

#[test]
fn check_sends_one_request_with_the_exact_user_agent_and_no_cookies() {
    let server = Server::start();
    server.route(
        &latest_path(),
        Reply::ok(release_json(&server.base, "v0.1.1", false, &[])),
    );
    let available = updater(&server).check().unwrap().unwrap();
    assert_eq!(available.version.to_string(), "0.1.1");
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, latest_path());
    assert_eq!(requests[0].headers["user-agent"], "PaintCraft/0.1.0");
    for forbidden in ["cookie", "authorization", "referer"] {
        assert!(!requests[0].headers.contains_key(forbidden), "{forbidden}");
    }
    assert_eq!(requests[0].headers["accept"], "application/vnd.github+json");
}

#[test]
fn a_redirect_to_a_forbidden_host_is_not_followed() {
    let server = Server::start();
    let port = server.base.rsplit(':').next().unwrap();
    server.route(
        &latest_path(),
        Reply::redirect(&format!("http://localhost:{port}/steal")),
    );
    let result = updater(&server).check();
    assert!(matches!(result, Err(Error::HostNotAllowed(host)) if host == "localhost"));
    assert_eq!(
        server.requests().len(),
        1,
        "the forbidden hop must not be requested"
    );
}

#[test]
fn redirects_to_allowed_hosts_are_followed_up_to_a_limit() {
    let server = Server::start();
    server.route(
        &latest_path(),
        Reply::redirect(&format!("{}/hop1", server.base)),
    );
    server.route("/hop1", Reply::redirect(&format!("{}/final", server.base)));
    server.route(
        "/final",
        Reply::ok(release_json(&server.base, "v0.1.1", false, &[])),
    );
    assert!(updater(&server).check().unwrap().is_some());
    server.route("/loop", Reply::redirect(&format!("{}/loop", server.base)));
    server.route(
        &latest_path(),
        Reply::redirect(&format!("{}/loop", server.base)),
    );
    assert!(matches!(
        updater(&server).check(),
        Err(Error::TooManyRedirects)
    ));
}

#[test]
fn a_redirect_without_a_location_or_with_a_relative_one_fails() {
    let server = Server::start();
    server.route(
        &latest_path(),
        Reply {
            status: 302,
            headers: vec![],
            body: vec![],
            announce: None,
        },
    );
    assert!(updater(&server).check().is_err());
    server.route(&latest_path(), Reply::redirect("/relative"));
    assert!(updater(&server).check().is_err());
}

#[test]
fn the_production_policy_never_talks_plain_http() {
    let server = Server::start();
    server.route(
        &latest_path(),
        Reply::ok(release_json(&server.base, "v0.1.1", false, &[])),
    );
    let mut endpoint = support::endpoint(&server.base);
    endpoint.https_only = true;
    let updater = Updater::with_endpoint("0.1.0", manual_platform(), endpoint).unwrap();
    assert!(matches!(updater.check(), Err(Error::InsecureUrl)));
    assert!(server.requests().is_empty());
}

#[test]
fn the_api_document_is_capped_at_one_mebibyte() {
    let server = Server::start();
    let big = vec![b' '; paintcraft_update::release::MAX_API_BYTES as usize + 1];
    server.route(&latest_path(), Reply::ok(big));
    assert!(matches!(
        updater(&server).check(),
        Err(Error::ResponseTooLarge { .. })
    ));
    // A server that announces less than it sends is cut off at the cap as well.
    let mut lying = Reply::ok(vec![
        b' ';
        paintcraft_update::release::MAX_API_BYTES as usize + 10
    ]);
    lying.announce = Some(10);
    server.route(&latest_path(), lying);
    assert!(updater(&server).check().is_err());
}

#[test]
fn server_errors_and_garbage_are_errors() {
    let server = Server::start();
    assert!(matches!(
        updater(&server).check(),
        Err(Error::HttpStatus(404))
    ));
    server.route(&latest_path(), Reply::status(503));
    assert!(matches!(
        updater(&server).check(),
        Err(Error::HttpStatus(503))
    ));
    server.route(&latest_path(), Reply::ok("<html>"));
    assert!(matches!(
        updater(&server).check(),
        Err(Error::BadRelease(_))
    ));
}
