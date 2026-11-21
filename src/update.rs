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

/// `kstar update`, with `--check` and `--no-vscode`.
pub fn update(args: &[&str]) -> ExitCode {
    let mut check_only = false;
    let mut vscode = std::env::var("KSTAR_NO_VSCODE").map_or(true, |v| v != "1");
    for arg in args {
        match *arg {
            "--check" => check_only = true,
            "--no-vscode" => vscode = false,
            _ => {
                eprintln!("kstar: usage: kstar update [--check] [--no-vscode]");
                return ExitCode::from(2);
            }
        }
    }
    let current = env!("CARGO_PKG_VERSION");
    let latest = match latest_tag() {
        Ok(tag) => tag,
        Err(message) => {
            eprintln!("kstar: {message}");
            return ExitCode::FAILURE;
        }
    };
    let mut status = ExitCode::SUCCESS;
    if !is_newer(&latest, current) {
        println!("kstar {current} is up to date (the latest release is {latest})");
    } else {
        println!("a newer version is available: {latest} (this is {current})");
        if check_only {
            println!("run `kstar update` to see how to install it");
        } else {
            println!(
                "kstar does not replace itself while running; install {latest} the way you installed this one.\n\
                 With the Windows installer, download and run the new kstar-setup-x64.exe. Otherwise:\n\n    {}\n\n\
                 Both also update the VS Code extension.",
                install_command()
            );
        }
    }
    if vscode {
        if let Err(message) = match_extension(current, check_only) {
            eprintln!("kstar: {message}");
            status = ExitCode::FAILURE;
        }
    }
    status
}

// ---- the VS Code extension ----
//
// Each release ships the extension as kleene-star-<version>.vsix next to the
// programs. An extension from another release than the language server
// highlights and offers less than the server knows (an old one has no
// highlighting for newer keywords and modules), so `kstar update` keeps the
// two together.

/// The identifier VS Code gives the extension: publisher.name.
const EXTENSION_ID: &str = "kleene-star.kleene-star";

/// VS Code's `code` command: `KSTAR_CODE` if set, then `code` on the PATH,
/// then (on Windows) where VS Code's installers put it. `None` if VS Code
/// is not installed.
fn code_command() -> Option<std::path::PathBuf> {
    if let Some(path) = std::env::var_os("KSTAR_CODE").filter(|p| !p.is_empty()) {
        return Some(path.into());
    }
    let file = if cfg!(windows) { "code.cmd" } else { "code" };
    let mut folders: Vec<std::path::PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    if cfg!(windows) {
        for (base, rest) in [("LOCALAPPDATA", "Programs\\Microsoft VS Code\\bin"), ("ProgramFiles", "Microsoft VS Code\\bin")] {
            if let Some(base) = std::env::var_os(base) {
                folders.push(std::path::Path::new(&base).join(rest));
            }
        }
    }
    folders.into_iter().map(|f| f.join(file)).find(|p| p.is_file())
}

/// The version of the Kleene Star extension in the output of
/// `code --list-extensions --show-versions` (lines like `pub.name@1.2.3`),
/// or `None` if it is not installed.
pub fn extension_version(listing: &str) -> Option<String> {
    listing.lines().find_map(|line| {
        let (id, version) = line.trim().split_once('@')?;
        id.eq_ignore_ascii_case(EXTENSION_ID).then(|| version.trim().to_string())
    })
}

/// Makes the VS Code extension match this kstar, if VS Code is installed.
fn match_extension(current: &str, check_only: bool) -> Result<(), String> {
    match code_command() {
        Some(code) => match_extension_with(&code, current, check_only, download_vsix),
        None => Ok(()),
    }
}

/// `match_extension` with VS Code's `code` command and the download given,
/// so a test can stand in for both.
fn match_extension_with(
    code: &std::path::Path,
    current: &str,
    check_only: bool,
    download: impl Fn(&str, &std::path::Path) -> Result<std::path::PathBuf, String>,
) -> Result<(), String> {
    let listing = Command::new(code)
        .args(["--list-extensions", "--show-versions"])
        .output()
        .map_err(|e| format!("could not run {}: {e}", code.display()))?;
    let installed = extension_version(&String::from_utf8_lossy(&listing.stdout));
    if installed.as_deref() == Some(current) {
        println!("the VS Code extension is {current}, the same as kstar");
        return Ok(());
    }
    let have = installed.map_or("not installed".to_string(), |v| format!("{v}, not {current}"));
    if check_only {
        println!("the VS Code extension is {have}; run `kstar update` to install kleene-star-{current}.vsix");
        return Ok(());
    }
    println!("the VS Code extension is {have}; installing kleene-star-{current}.vsix");
    let work = std::env::temp_dir().join(format!("kstar-update-{}", std::process::id()));
    std::fs::create_dir_all(&work).map_err(|e| format!("could not make {}: {e}", work.display()))?;
    let result = download(current, &work).and_then(|vsix| {
        let status = Command::new(code)
            .arg("--install-extension")
            .arg(&vsix)
            .arg("--force")
            .status()
            .map_err(|e| format!("could not run {}: {e}", code.display()))?;
        if !status.success() {
            return Err(format!("VS Code could not install {}", vsix.display()));
        }
        println!("installed the VS Code extension {current}; reload VS Code windows to use it");
        Ok(())
    });
    let _ = std::fs::remove_dir_all(&work);
    result
}

/// Downloads kleene-star-<version>.vsix and its checksum from release
/// v<version> into `work` (through `gh`, or `curl` without it), checks the
/// checksum, and returns the file.
fn download_vsix(version: &str, work: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let repo = repository();
    let tag = format!("v{version}");
    let vsix = format!("kleene-star-{version}.vsix");
    let sum = format!("{vsix}.sha256");
    let by_gh = Command::new("gh")
        .args(["release", "download", &tag, "-R", &repo, "-p", &vsix, "-p", &sum, "-D"])
        .arg(work)
        .output()
        .is_ok_and(|o| o.status.success());
    if !by_gh {
        for file in [&vsix, &sum] {
            let status = Command::new("curl")
                .args(["-fsSL", "-o"])
                .arg(work.join(file))
                .arg(format!("https://github.com/{repo}/releases/download/{tag}/{file}"))
                .status()
                .map_err(|e| format!("could not run gh or curl: {e}"))?;
            if !status.success() {
                return Err(format!("could not download {file} from release {tag} of {repo}"));
            }
        }
    }
    let path = work.join(&vsix);
    let bytes = std::fs::read(&path).map_err(|e| format!("could not read {}: {e}", path.display()))?;
    let expected = std::fs::read_to_string(work.join(&sum)).map_err(|e| format!("could not read {sum}: {e}"))?;
    check_sum(&bytes, &expected, &vsix)?;
    Ok(path)
}

/// Checks `bytes` against a checksum file's first word.
pub fn check_sum(bytes: &[u8], file: &str, name: &str) -> Result<(), String> {
    let expected = file.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
    let actual = crate::sha256::hex(bytes);
    if expected != actual {
        return Err(format!("the checksum of {name} does not match (expected {expected}, got {actual}); nothing was installed"));
    }
    Ok(())
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
    fn finds_the_extension_in_the_listing() {
        let listing = "ms-python.python@2026.1.0\nkleene-star.kleene-star@0.2.3\nrust-lang.rust-analyzer@0.3.2\n";
        assert_eq!(extension_version(listing).as_deref(), Some("0.2.3"));
        assert_eq!(extension_version("Kleene-Star.Kleene-Star@0.5.1\r\n").as_deref(), Some("0.5.1"));
        assert_eq!(extension_version("ms-python.python@1.0.0\n"), None);
        assert_eq!(extension_version(""), None);
    }

    #[test]
    fn checks_downloads_against_their_checksum() {
        let file = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  kleene-star-0.5.1.vsix\n";
        assert_eq!(check_sum(b"abc", file, "x.vsix"), Ok(()));
        assert_eq!(check_sum(b"abc", &file.to_uppercase(), "x.vsix"), Ok(()));
        let wrong = check_sum(b"abd", file, "x.vsix").unwrap_err();
        assert!(wrong.contains("does not match") && wrong.contains("nothing was installed"), "{wrong}");
        assert!(check_sum(b"abc", "", "x.vsix").is_err());
    }

    /// A stand-in for VS Code's `code` command that lists `listing` and
    /// writes the arguments of any other call to `log`.
    fn fake_code(dir: &std::path::Path, listing: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let log = dir.join("log.txt");
        let script = if cfg!(windows) {
            let path = dir.join("code.cmd");
            let text = format!(
                "@echo off
if \"%1\"==\"--list-extensions\" (
 echo {listing}
) else (
 echo %* > \"{}\"
)
",
                log.display()
            );
            std::fs::write(&path, text).unwrap();
            path
        } else {
            let path = dir.join("code");
            let text = format!("#!/bin/sh
if [ \"$1\" = --list-extensions ]; then echo '{listing}'; else echo \"$@\" > '{}'; fi
", log.display());
            std::fs::write(&path, text).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
            path
        };
        (script, log)
    }

    #[test]
    fn installs_the_matching_extension_only_when_it_differs() {
        let dir = std::env::temp_dir().join(format!("kstar-update-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let vsix = dir.join("kleene-star-0.5.1.vsix");
        let download = |version: &str, _: &std::path::Path| {
            assert_eq!(version, "0.5.1");
            Ok(vsix.clone())
        };
        // An old extension is replaced.
        let (code, log) = fake_code(&dir, "kleene-star.kleene-star@0.2.3");
        assert_eq!(match_extension_with(&code, "0.5.1", false, download), Ok(()));
        let call = std::fs::read_to_string(&log).unwrap();
        assert!(call.contains("--install-extension") && call.contains("kleene-star-0.5.1.vsix") && call.contains("--force"), "{call}");
        // With --check, or when it already matches, nothing is installed.
        std::fs::remove_file(&log).unwrap();
        let never = |_: &str, _: &std::path::Path| -> Result<std::path::PathBuf, String> { panic!("downloaded") };
        assert_eq!(match_extension_with(&code, "0.5.1", true, never), Ok(()));
        let (code, log) = fake_code(&dir, "kleene-star.kleene-star@0.5.1");
        assert_eq!(match_extension_with(&code, "0.5.1", false, never), Ok(()));
        assert!(!log.exists());
        // A failed download installs nothing and says why.
        let (code, log) = fake_code(&dir, "other.ext@1.0.0");
        let failing = |_: &str, _: &std::path::Path| -> Result<std::path::PathBuf, String> { Err("no network".into()) };
        assert_eq!(match_extension_with(&code, "0.5.1", false, failing), Err("no network".into()));
        assert!(!log.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn version_has_four_lines() {
        let text = version();
        assert!(text.starts_with(&format!("kstar {}\ncommit ", env!("CARGO_PKG_VERSION"))));
        assert_eq!(text.lines().count(), 4);
        assert!(text.contains("\nbuilt 20"), "{text}");
    }
}
