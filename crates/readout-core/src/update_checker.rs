use std::path::{Path, PathBuf};

const GITHUB_RELEASES_URL: &str =
    "https://api.github.com/repos/vaclavik-xyz/readOutRS/releases/latest";

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallationMethod {
    DirectDownload,
    HomebrewFormula,
    HomebrewCask,
}

impl InstallationMethod {
    pub fn update_command(self) -> Option<&'static str> {
        match self {
            Self::DirectDownload => None,
            Self::HomebrewFormula => Some("brew upgrade readout"),
            Self::HomebrewCask => Some("brew upgrade --cask readout"),
        }
    }
}

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

/// Detect the current installation method (cached).
pub fn installation_method() -> InstallationMethod {
    use std::sync::OnceLock;
    static CACHED: OnceLock<InstallationMethod> = OnceLock::new();
    *CACHED.get_or_init(|| {
        let exe_path = std::env::current_exe().ok();
        let prefixes = homebrew_prefixes();
        classify_installation(
            exe_path.as_deref(),
            has_homebrew_formula_install(&prefixes),
            has_homebrew_cask_install(&prefixes),
            &prefixes,
        )
    })
}

/// Return the command users should run to update this installation, if known.
pub fn update_command() -> Option<&'static str> {
    installation_method().update_command()
}

/// Detect if running from a Homebrew installation (cached).
pub fn is_homebrew() -> bool {
    matches!(
        installation_method(),
        InstallationMethod::HomebrewFormula | InstallationMethod::HomebrewCask
    )
}

fn classify_installation(
    exe_path: Option<&Path>,
    formula_installed: bool,
    cask_installed: bool,
    homebrew_prefixes: &[PathBuf],
) -> InstallationMethod {
    let Some(exe_path) = exe_path else {
        return InstallationMethod::DirectDownload;
    };

    if is_homebrew_formula_path(exe_path)
        || (formula_installed && is_homebrew_bin_path(exe_path, homebrew_prefixes))
    {
        return InstallationMethod::HomebrewFormula;
    }

    if cask_installed && is_app_bundle_executable(exe_path) {
        return InstallationMethod::HomebrewCask;
    }

    InstallationMethod::DirectDownload
}

fn has_homebrew_formula_install(prefixes: &[PathBuf]) -> bool {
    prefixes
        .iter()
        .any(|prefix| prefix.join("Cellar").join("readout").exists())
}

fn has_homebrew_cask_install(prefixes: &[PathBuf]) -> bool {
    prefixes
        .iter()
        .any(|prefix| prefix.join("Caskroom").join("readout").exists())
}

fn homebrew_prefixes() -> Vec<PathBuf> {
    let mut prefixes = Vec::new();
    if let Some(prefix) = std::env::var_os("HOMEBREW_PREFIX")
        && !prefix.is_empty()
    {
        prefixes.push(PathBuf::from(prefix));
    }
    for prefix in ["/opt/homebrew", "/usr/local", "/home/linuxbrew/.linuxbrew"] {
        let path = PathBuf::from(prefix);
        if !prefixes.iter().any(|p| p == &path) {
            prefixes.push(path);
        }
    }
    prefixes
}

fn is_homebrew_formula_path(path: &Path) -> bool {
    has_component_sequence(path, &["Cellar", "readout"])
}

fn is_app_bundle_executable(path: &Path) -> bool {
    let components = path_components(path);
    components.windows(3).any(|window| {
        window[0].ends_with(".app") && window[1] == "Contents" && window[2] == "MacOS"
    })
}

fn is_homebrew_bin_path(path: &Path, prefixes: &[PathBuf]) -> bool {
    prefixes
        .iter()
        .any(|prefix| path.starts_with(prefix.join("bin")))
}

fn has_component_sequence(path: &Path, sequence: &[&str]) -> bool {
    path_components(path).windows(sequence.len()).any(|window| {
        window
            .iter()
            .map(String::as_str)
            .eq(sequence.iter().copied())
    })
}

fn path_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| component.as_os_str().to_str().map(str::to_owned))
        .collect()
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
    use std::path::{Path, PathBuf};

    fn test_homebrew_prefixes() -> Vec<PathBuf> {
        vec![
            PathBuf::from("/opt/homebrew"),
            PathBuf::from("/usr/local"),
            PathBuf::from("/home/linuxbrew/.linuxbrew"),
        ]
    }

    #[test]
    fn test_is_newer() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
        assert!(is_newer("0.1.1", "0.1.0"));
    }

    #[test]
    fn classifies_formula_from_cellar_path() {
        let method = classify_installation(
            Some(Path::new(
                "/opt/homebrew/Cellar/readout/0.1.2/bin/readout-tui",
            )),
            false,
            false,
            &test_homebrew_prefixes(),
        );

        assert_eq!(method, InstallationMethod::HomebrewFormula);
        assert_eq!(method.update_command(), Some("brew upgrade readout"));
    }

    #[test]
    fn classifies_formula_from_homebrew_bin_when_formula_exists() {
        let method = classify_installation(
            Some(Path::new("/opt/homebrew/bin/readout-gui")),
            true,
            false,
            &test_homebrew_prefixes(),
        );

        assert_eq!(method, InstallationMethod::HomebrewFormula);
    }

    #[test]
    fn classifies_formula_from_custom_homebrew_bin_when_formula_exists() {
        let prefixes = [PathBuf::from("/Users/dev/homebrew")];
        let method = classify_installation(
            Some(Path::new("/Users/dev/homebrew/bin/readout-gui")),
            true,
            false,
            &prefixes,
        );

        assert_eq!(method, InstallationMethod::HomebrewFormula);
    }

    #[test]
    fn classifies_cask_app_bundle_when_cask_exists() {
        let method = classify_installation(
            Some(Path::new(
                "/Applications/readOut.app/Contents/MacOS/readout-gui",
            )),
            false,
            true,
            &test_homebrew_prefixes(),
        );

        assert_eq!(method, InstallationMethod::HomebrewCask);
        assert_eq!(method.update_command(), Some("brew upgrade --cask readout"));
    }

    #[test]
    fn does_not_classify_manual_app_bundle_as_homebrew() {
        let method = classify_installation(
            Some(Path::new(
                "/Applications/readOut.app/Contents/MacOS/readout-gui",
            )),
            false,
            false,
            &test_homebrew_prefixes(),
        );

        assert_eq!(method, InstallationMethod::DirectDownload);
        assert_eq!(method.update_command(), None);
    }
}
