//! `kstar version` and `kstar update`.
//!
//! `kstar update --check` asks GitHub for the latest release and compares it
//! with this build. Replacing the running program in place is not done:
//! on Windows a running `.exe` cannot be overwritten, and getting the
//! rename-and-replace dance right for every way Kleene Star can be installed
//! (installer, script, `cargo install`) is more risk than it is worth. So
//! `kstar update` says what is new and prints the command that installs it.

use std::process::{Command, ExitCode};

/// The repository releases come from; `KSTAR_REPO` overrides it.
const REPOSITORY: &str = "Pratham71/kleene-star";

fn repository() -> String {
    std::env::var("KSTAR_REPO").ok().filter(|r| !r.is_empty()).unwrap_or_else(|| REPOSITORY.to_string())
}

/// Version, commit, build date and target, one per line.
pub fn version() -> String {
    format!(
        "kstar {}\ncommit {}\nbuilt {}\ntarget {}",
        env!("CARGO_PKG_VERSION"),
        env!("KSTAR_COMMIT"),
        env!("KSTAR_BUILD_DATE"),
        env!("KSTAR_TARGET")
    )
}

/// A version as numbers, `0.2.0` -> [0, 2, 0], ignoring a leading `v` and
/// anything after a `-` (a pre-release sorts before its release).
fn parse(version: &str) -> Option<(Vec<u64>, bool)> {
    let version = version.trim().trim_start_matches('v');
    let (core, pre) = match version.split_once('-') {
        Some((core, _)) => (core, true),
        None => (version, false),
    };
    let numbers = core.split('.').map(|n| n.parse().ok()).collect::<Option<Vec<u64>>>()?;
    Some((numbers, pre))
}

/// True if `candidate` is a later version than `current`.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse(candidate), parse(current)) {
        (Some((a, a_pre)), Some((b, b_pre))) => a > b || (a == b && b_pre && !a_pre),
        _ => false,
    }
}

/// The tag of the latest release, asked through the `gh` command (which
/// handles signing in, needed if the repository is private) or, failing
/// that, `curl` with `GITHUB_TOKEN` if it is set.
fn latest_tag() -> Result<String, String> {
    let repo = repository();
    let gh = Command::new("gh").args(["api", &format!("repos/{repo}/releases/latest"), "--jq", ".tag_name"]).output();
    if let Ok(output) = &gh {
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }
        if String::from_utf8_lossy(&output.stderr).contains("404") {
            return Err(format!("{repo} has no releases yet, or this GitHub account cannot see it"));
        }
    }
    let mut curl = Command::new("curl");
    curl.args(["-fsSL", "-H", "Accept: application/vnd.github+json"]);
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        curl.args(["-H", &format!("Authorization: Bearer {token}")]);
    }
    let output = curl
        .arg(format!("https://api.github.com/repos/{repo}/releases/latest"))
        .output()
        .map_err(|e| format!("could not run gh or curl: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "could not ask GitHub about the latest release of {repo}. If the repository is private, \
             sign in with `gh auth login` (or set GITHUB_TOKEN) and try again"
        ));
    }
    let body = String::from_utf8_lossy(&output.stdout);
    let after = body.split("\"tag_name\"").nth(1).ok_or("GitHub's answer has no tag_name")?;
    let tag = after.split('"').nth(1).ok_or("GitHub's answer has no tag_name")?;
    Ok(tag.to_string())
}

/// The command that installs the latest release on this system: the same
/// one-liner as in the README, which needs only PowerShell or curl.
fn install_command() -> String {
    let repo = repository();
    if cfg!(windows) {
        format!("powershell -c \"irm https://raw.githubusercontent.com/{repo}/main/install.ps1 | iex\"")
    } else {
        format!("curl -fsSL https://raw.githubusercontent.com/{repo}/main/install.sh | sh")
    }
}

/// `kstar update` and `kstar update --check`.
pub fn update(args: &[&str]) -> ExitCode {
    let check_only = match args {
        [] => false,
        ["--check"] => true,
        _ => {
            eprintln!("kstar: usage: kstar update [--check]");
            return ExitCode::from(2);
        }
    };
    let current = env!("CARGO_PKG_VERSION");
    let latest = match latest_tag() {
        Ok(tag) => tag,
        Err(message) => {
            eprintln!("kstar: {message}");
            return ExitCode::FAILURE;
        }
    };
    if !is_newer(&latest, current) {
        println!("kstar {current} is up to date (the latest release is {latest})");
        return ExitCode::SUCCESS;
    }
    println!("a newer version is available: {latest} (this is {current})");
    if check_only {
        println!("run `kstar update` to see how to install it");
    } else {
        println!(
            "kstar does not replace itself while running; install {latest} the way you installed this one.\n\
             With the Windows installer, download and run the new kstar-setup-x64.exe. Otherwise:\n\n    {}",
            install_command()
        );
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(is_newer("v0.3.0", "0.2.0"));
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(is_newer("v0.2.0", "0.2.0-rc.1"));
        assert!(!is_newer("v0.2.0", "0.2.0"));
        assert!(!is_newer("v0.2.0-rc.1", "0.2.0"));
        assert!(!is_newer("v0.1.9", "0.2.0"));
        assert!(!is_newer("nonsense", "0.2.0"));
    }

    #[test]
    fn version_has_four_lines() {
        let text = version();
        assert!(text.starts_with(&format!("kstar {}\ncommit ", env!("CARGO_PKG_VERSION"))));
        assert_eq!(text.lines().count(), 4);
        assert!(text.contains("\nbuilt 20"), "{text}");
    }
}
