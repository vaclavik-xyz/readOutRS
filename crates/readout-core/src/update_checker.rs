const GITHUB_RELEASES_URL: &str =
    "https://api.github.com/repos/vaclavik-xyz/readOutRS/releases/latest";

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Check GitHub for a newer release. Returns the new version string if available.
/// Designed to be called from `spawn_blocking` — blocks on HTTP.
pub fn check_for_update() -> Option<String> {
    let agent = ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(10)))
            .build(),
    );
    let body: serde_json::Value = agent
        .get(GITHUB_RELEASES_URL)
        .header("User-Agent", "readOutRS")
        .call()
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;
    let tag = body["tag_name"].as_str()?;
    let remote = tag.strip_prefix('v').unwrap_or(tag);
    if is_newer(remote, CURRENT_VERSION) {
        Some(remote.to_string())
    } else {
        None
    }
}

/// Detect if running from a Homebrew installation (cached).
pub fn is_homebrew() -> bool {
    use std::sync::OnceLock;
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .map(|s| s.contains("/Cellar/") || s.contains("/homebrew/"))
            .unwrap_or(false)
    })
}

fn is_newer(remote: &str, current: &str) -> bool {
    let parse = |s: &str| -> Option<Vec<u32>> {
        // Strip pre-release suffix (everything after first '-')
        let clean = s.split('-').next().unwrap_or(s);
        clean.split('.').map(|p| p.parse().ok()).collect()
    };
    match (parse(remote), parse(current)) {
        (Some(r), Some(c)) => r > c,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
        assert!(is_newer("0.1.1", "0.1.0"));
    }
}
