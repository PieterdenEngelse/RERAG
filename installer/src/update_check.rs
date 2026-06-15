//! Phase F — auto-update check.
//!
//! Hits api.github.com/repos/.../releases/latest on Welcome-screen mount
//! and surfaces a non-blocking banner if the published tag is newer than
//! our embedded CARGO_PKG_VERSION. Result is cached in
//! ~/.cache/ag-installer/update-check.json for 4 hours so we don't burn
//! through GitHub's 60-req/hr unauthenticated rate limit when the user
//! re-launches the installer.
//!
//! Failure modes (network down, GitHub rate-limited, malformed JSON,
//! no releases yet) all return `None` — the banner stays hidden, the
//! installer carries on. Update checks are best-effort UX, not a
//! correctness path.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

const CACHE_TTL_SECS: u64 = 4 * 60 * 60;
const RELEASES_URL: &str =
    "https://api.github.com/repos/PieterdenEngelse/RARAG/releases/latest";

/// What the Welcome screen needs to render the "update available" banner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    /// The GitHub release page URL — opens in browser on banner click.
    pub url: String,
}

/// Check for a newer release. Returns Some(info) when one is published and
/// strictly newer than the running installer's version; None in every
/// other case (no newer release, network error, parse error, rate-limited,
/// repo private without auth, …).
pub async fn check() -> Option<UpdateInfo> {
    if let Some((tag, url)) = read_cache() {
        return judge(tag, url);
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent(concat!("ag-installer/", env!("CARGO_PKG_VERSION")))
        .build()
        .ok()?;
    let resp = client.get(RELEASES_URL).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body: serde_json::Value = resp.json().await.ok()?;
    let tag = body.get("tag_name")?.as_str()?.to_string();
    let url = body.get("html_url")?.as_str()?.to_string();
    write_cache(&tag, &url);
    judge(tag, url)
}

fn judge(latest: String, url: String) -> Option<UpdateInfo> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    if version_newer(&latest, &current) {
        Some(UpdateInfo { current, latest, url })
    } else {
        None
    }
}

// --- semver compare ---------------------------------------------------------

fn version_newer(tag: &str, current: &str) -> bool {
    // Tags are "v0.4.0"; CARGO_PKG_VERSION is "0.4.0".
    let tag_clean = tag.trim_start_matches('v');
    match (parse_semver(tag_clean), parse_semver(current)) {
        (Some(t), Some(c)) => t > c,
        _ => false,
    }
}

fn parse_semver(s: &str) -> Option<(u32, u32, u32)> {
    // Pre-release suffixes (-rc1, -alpha, …) get stripped — Phase 0
    // decision was "no pre-release tags", but if someone tags one
    // we treat it as the major.minor.patch part only.
    let core = s.split('-').next()?;
    let mut parts = core.split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = parts.next()?.parse().ok()?;
    let patch: u32 = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

// --- cache ------------------------------------------------------------------

fn cache_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let dir = PathBuf::from(home).join(".cache/ag-installer");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("update-check.json"))
}

fn read_cache() -> Option<(String, String)> {
    let path = cache_path()?;
    let meta = std::fs::metadata(&path).ok()?;
    let modified = meta.modified().ok()?;
    let age = SystemTime::now().duration_since(modified).ok()?;
    if age.as_secs() > CACHE_TTL_SECS {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    Some((
        json.get("tag")?.as_str()?.to_string(),
        json.get("url")?.as_str()?.to_string(),
    ))
}

fn write_cache(tag: &str, url: &str) {
    if let Some(path) = cache_path() {
        let json = serde_json::json!({ "tag": tag, "url": url });
        let _ = std::fs::write(path, json.to_string());
    }
}
