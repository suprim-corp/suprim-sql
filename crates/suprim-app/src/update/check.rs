//! Polls the update feed and decides whether a newer release is available.

use std::time::Duration;

use semver::Version;
use serde::Deserialize;

use super::{CURRENT_VERSION, DEFAULT_ENDPOINT};

/// Payload returned by the `/update/latest` endpoint on suprim-server.
/// Fields mirror the server DTO (snake_case on the wire).
///
/// `channel`, `release_notes`, and `release_url` are carried end-to-end so
/// future UI can show them in a "What's new" modal. Keep them even though
/// the initial banner doesn't render them — dropping them now means a lossy
/// re-parse the next time we want to surface the data.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct LatestRelease {
    pub version: String,
    pub channel: String,
    pub os: String,
    pub arch: String,
    pub download_url: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub release_notes: Option<String>,
    pub release_url: Option<String>,
}

#[derive(Debug)]
pub enum UpdateError {
    Http(reqwest::Error),
    Envelope(String),
    SemverParse(String),
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateError::Http(e) => write!(f, "update feed request failed: {e}"),
            UpdateError::Envelope(m) => write!(f, "update feed responded with error: {m}"),
            UpdateError::SemverParse(v) => write!(f, "cannot parse version '{v}' as semver"),
        }
    }
}

impl std::error::Error for UpdateError {}

impl From<reqwest::Error> for UpdateError {
    fn from(e: reqwest::Error) -> Self {
        UpdateError::Http(e)
    }
}

/// Server wrapper matching `dev.suprim.kit.web.response.BaseResponse`.
/// `code == 1` means success; anything else is an error carried in `message`.
#[derive(Debug, Deserialize)]
struct BaseResponse<T> {
    code: i32,
    message: Option<String>,
    data: Option<T>,
}

/// Query the update feed. Returns `Ok(Some(release))` if the server's latest
/// version is strictly greater than [`CURRENT_VERSION`]; `Ok(None)` means
/// "already up to date".
pub async fn check_for_update(os: &str, arch: &str) -> Result<Option<LatestRelease>, UpdateError> {
    let endpoint =
        std::env::var("SUPRIM_UPDATE_ENDPOINT").unwrap_or_else(|_| DEFAULT_ENDPOINT.to_owned());
    check_at(&endpoint, CURRENT_VERSION, os, arch).await
}

/// Test-friendly variant of [`check_for_update`]: takes the endpoint and
/// current version as explicit arguments so a unit test can point it at a
/// mock server without mutating process-wide environment variables.
pub(crate) async fn check_at(
    endpoint: &str,
    current_version: &str,
    os: &str,
    arch: &str,
) -> Result<Option<LatestRelease>, UpdateError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(concat!("SuprimSQL/", env!("CARGO_PKG_VERSION")))
        .build()?;

    let envelope: BaseResponse<LatestRelease> = client
        .get(endpoint)
        .query(&[("channel", "stable"), ("os", os), ("arch", arch)])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    if envelope.code != 1 {
        return Err(UpdateError::Envelope(
            envelope.message.unwrap_or_else(|| format!("code={}", envelope.code)),
        ));
    }

    let release = match envelope.data {
        Some(r) => r,
        None => return Ok(None),
    };

    let latest = Version::parse(&release.version)
        .map_err(|_| UpdateError::SemverParse(release.version.clone()))?;
    let current = Version::parse(current_version)
        .map_err(|_| UpdateError::SemverParse(current_version.to_owned()))?;

    if latest > current {
        Ok(Some(release))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn semver_comparison_uses_strict_greater_than() {
        let current = Version::parse("0.1.2").unwrap();
        assert!(Version::parse("0.1.3").unwrap() > current);
        assert!(Version::parse("0.2.0").unwrap() > current);
        assert!(Version::parse("1.0.0").unwrap() > current);
        assert!(Version::parse("0.1.2").unwrap() <= current);
        assert!(Version::parse("0.1.1").unwrap() <= current);
    }

    #[test]
    fn error_display_includes_context_for_each_variant() {
        // Each variant's Display impl must include the triggering detail so
        // log lines are useful in production.
        let envelope = UpdateError::Envelope("not found".to_owned());
        let msg = format!("{envelope}");
        assert!(msg.contains("not found"), "got: {msg}");

        let semver = UpdateError::SemverParse("banana".to_owned());
        let msg = format!("{semver}");
        assert!(msg.contains("banana"), "got: {msg}");
    }

    #[tokio::test]
    async fn http_variant_wraps_reqwest_errors() {
        // Minimal way to provoke a reqwest::Error: hit a closed port.
        let err = reqwest::get("http://127.0.0.1:1/").await.unwrap_err();
        let wrapped: UpdateError = err.into();
        assert!(matches!(wrapped, UpdateError::Http(_)));
        let msg = format!("{wrapped}");
        assert!(msg.contains("update feed request failed"), "got: {msg}");
    }

    #[test]
    fn error_implements_std_error_trait() {
        // Confirms the `impl std::error::Error for UpdateError` compiles —
        // someone tempted to drop it would break callers that do
        // `Box<dyn Error>` or `?`-propagate into anyhow/thiserror chains.
        fn assert_error<E: std::error::Error>() {}
        assert_error::<UpdateError>();
    }

    #[test]
    fn latest_release_deserializes_snake_case_from_suprim_server() {
        // Exact shape served by dev.suprim.server.update.UpdateController
        // (JacksonConfig SNAKE_CASE). If this drifts, the client breaks.
        let raw = r#"{
            "version": "1.2.3",
            "channel": "stable",
            "os": "macos",
            "arch": "universal",
            "download_url": "https://example.test/a.dmg",
            "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "size_bytes": 42,
            "release_notes": "notes",
            "release_url": "https://example.test/tag",
            "published_at": "2026-01-02T03:04:05Z"
        }"#;
        let r: LatestRelease = serde_json::from_str(raw).unwrap();
        assert_eq!(r.version, "1.2.3");
        assert_eq!(r.download_url, "https://example.test/a.dmg");
        assert_eq!(r.size_bytes, 42);
        assert_eq!(r.release_notes.as_deref(), Some("notes"));
    }

    #[test]
    fn latest_release_allows_null_optional_fields() {
        let raw = r#"{
            "version": "1.0.0",
            "channel": "stable",
            "os": "linux",
            "arch": "x86_64",
            "download_url": "https://example.test/x.tar.gz",
            "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
            "size_bytes": 1,
            "release_notes": null,
            "release_url": null
        }"#;
        let r: LatestRelease = serde_json::from_str(raw).unwrap();
        assert!(r.release_notes.is_none());
        assert!(r.release_url.is_none());
    }

    /// Helper: spin up a wiremock server that returns the given JSON body
    /// for `GET /update/latest`.
    async fn mock_server_returning(body: serde_json::Value) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/update/latest"))
            .and(query_param("channel", "stable"))
            .and(query_param("os", "macos"))
            .and(query_param("arch", "universal"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;
        server
    }

    fn valid_release_body(version: &str) -> serde_json::Value {
        json!({
            "code": 1,
            "message": "Success!",
            "data": {
                "version": version,
                "channel": "stable",
                "os": "macos",
                "arch": "universal",
                "download_url": "https://example.test/SuprimSQL.dmg",
                "sha256": "a".repeat(64),
                "size_bytes": 12345,
                "release_notes": null,
                "release_url": null,
            }
        })
    }

    #[tokio::test]
    async fn returns_some_when_server_version_is_newer() {
        let server = mock_server_returning(valid_release_body("0.2.0")).await;
        let url = format!("{}/update/latest", server.uri());

        let got = check_at(&url, "0.1.2", "macos", "universal").await.unwrap();
        let release = got.expect("expected Some for newer version");
        assert_eq!(release.version, "0.2.0");
        assert_eq!(release.sha256.len(), 64);
    }

    #[tokio::test]
    async fn returns_none_when_server_version_equals_current() {
        let server = mock_server_returning(valid_release_body("0.1.2")).await;
        let url = format!("{}/update/latest", server.uri());

        let got = check_at(&url, "0.1.2", "macos", "universal").await.unwrap();
        assert!(got.is_none(), "equal versions should yield None");
    }

    #[tokio::test]
    async fn returns_none_when_server_version_is_older() {
        let server = mock_server_returning(valid_release_body("0.1.1")).await;
        let url = format!("{}/update/latest", server.uri());

        let got = check_at(&url, "0.1.2", "macos", "universal").await.unwrap();
        assert!(got.is_none(), "older versions should yield None");
    }

    #[tokio::test]
    async fn propagates_envelope_errors() {
        let body = json!({
            "code": 404,
            "message": "No release found",
            "data": null,
        });
        let server = mock_server_returning(body).await;
        let url = format!("{}/update/latest", server.uri());

        let err = check_at(&url, "0.1.2", "macos", "universal")
            .await
            .expect_err("non-success code should be an error");
        match err {
            UpdateError::Envelope(msg) => assert!(msg.contains("No release found")),
            other => panic!("expected Envelope variant, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn rejects_non_semver_version_strings() {
        let server = mock_server_returning(valid_release_body("banana")).await;
        let url = format!("{}/update/latest", server.uri());

        let err = check_at(&url, "0.1.2", "macos", "universal")
            .await
            .expect_err("invalid semver must not parse");
        match err {
            UpdateError::SemverParse(v) => assert_eq!(v, "banana"),
            other => panic!("expected SemverParse variant, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn returns_none_when_data_is_null_with_success_code() {
        // Server responded "success" but dropped the payload — treat as
        // "nothing new" rather than crashing the UI.
        let body = json!({"code": 1, "message": "Success!", "data": null});
        let server = mock_server_returning(body).await;
        let url = format!("{}/update/latest", server.uri());

        let got = check_at(&url, "0.1.2", "macos", "universal").await.unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn network_errors_bubble_up_as_http_variant() {
        // Point at an unreachable address — short timeout so the test is fast.
        // 127.0.0.1:1 is effectively guaranteed to be closed.
        let err = check_at(
            "http://127.0.0.1:1/update/latest",
            "0.1.2",
            "macos",
            "universal",
        )
        .await
        .expect_err("connection refused must be an error");
        match err {
            UpdateError::Http(_) => {}
            other => panic!("expected Http variant, got {other:?}"),
        }
    }
}
