//! `kstar version` and `kstar update`.
//!
//! `kstar update --check` asks GitHub for the latest release and compares it
//! with this build. `kstar update` shows the same, with the first paragraph
//! of the release's notes, and asks before it installs anything. On yes it
//! runs the release's install script into the folder this `kstar` is in
//! (every way of installing puts the programs in a `bin` folder), which also
//! updates the VS Code extension.
//!
//! A running program cannot be overwritten on Windows, and on Linux not
//! while it runs. It can be renamed, though, so the programs are first moved
//! aside as `kstar.old` (and `kstar-lsp.old`), put back if the install
//! fails, and the old files are removed the next time `kstar` starts.

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

pub use kstar::tui::update::{is_newer, Release};
use kstar::tui::update::{Channel, Outcome, Progress, StepState, UpdateScreen};

/// The latest release, asked through the `gh` command (which handles
/// signing in, needed if the repository is private) or, failing that,
/// `curl` with `GITHUB_TOKEN` if it is set. `KSTAR_LATEST` (a tag) stands in
/// for GitHub, for testing.
fn latest_release() -> Result<Release, String> {
    if let Ok(tag) = std::env::var("KSTAR_LATEST") {
        return Ok(Release { tag, notes: std::env::var("KSTAR_LATEST_NOTES").unwrap_or_default() });
    }
    let repo = repository();
    let gh = Command::new("gh").args(["api", &format!("repos/{repo}/releases/latest")]).output();
    if let Ok(output) = &gh {
        if output.status.success() {
            return parse_release(&String::from_utf8_lossy(&output.stdout));
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
    parse_release(&String::from_utf8_lossy(&output.stdout))
}

/// A release from GitHub's JSON for it.
fn parse_release(json: &str) -> Result<Release, String> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("GitHub's answer is not JSON: {e}"))?;
    let tag = value["tag_name"].as_str().ok_or("GitHub's answer has no tag_name")?.to_string();
    let notes = value["body"].as_str().unwrap_or("").to_string();
    Ok(Release { tag, notes })
}

/// The latest pre-release (beta), if there is one, from GitHub's list of
/// the newest releases, asked like `latest_release`. `KSTAR_LATEST_BETA`
/// (a tag, or empty for none) stands in for GitHub, for testing, as does
/// `KSTAR_LATEST` alone (no beta then). Any trouble asking means no beta:
/// the stable release still works.
fn latest_beta() -> Option<Release> {
    if let Ok(tag) = std::env::var("KSTAR_LATEST_BETA") {
        let notes = std::env::var("KSTAR_LATEST_BETA_NOTES").unwrap_or_default();
        return (!tag.is_empty()).then_some(Release { tag, notes });
    }
    if std::env::var_os("KSTAR_LATEST").is_some() {
        return None;
    }
    let path = format!("repos/{}/releases?per_page=30", repository());
    let json = match Command::new("gh").args(["api", &path]).output() {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).to_string(),
        _ => {
            let mut curl = Command::new("curl");
            curl.args(["-fsSL", "-H", "Accept: application/vnd.github+json"]);
            if let Ok(token) = std::env::var("GITHUB_TOKEN") {
                curl.args(["-H", &format!("Authorization: Bearer {token}")]);
            }
            let out = curl.arg(format!("https://api.github.com/{path}")).output().ok()?;
            if !out.status.success() {
                return None;
            }
            String::from_utf8_lossy(&out.stdout).to_string()
        }
    };
    newest_beta(&json)
}

/// The newest pre-release in GitHub's JSON list of releases.
fn newest_beta(json: &str) -> Option<Release> {
    let list: serde_json::Value = serde_json::from_str(json).ok()?;
    list.as_array()?
        .iter()
        .filter(|r| r["prerelease"].as_bool() == Some(true) && r["draft"].as_bool() != Some(true))
        .filter_map(|r| Some(Release { tag: r["tag_name"].as_str()?.to_string(), notes: r["body"].as_str().unwrap_or("").to_string() }))
        .reduce(|a, b| if is_newer(&b.tag, &a.tag) { b } else { a })
}

/// What a release is about, for the question: the first paragraph of its
/// notes that is not a heading, on one line, shortened past 300 characters.
pub fn headline(notes: &str) -> String {
    let notes = notes.replace("\r\n", "\n");
    let paragraph = notes
        .split("\n\n")
        .map(str::trim)
        .find(|p| !p.is_empty() && !p.starts_with('#'))
        .unwrap_or("");
    let line = paragraph.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() > 300 {
        let cut: String = line.chars().take(297).collect();
        format!("{}...", cut.trim_end())
    } else {
        line
    }
}

/// Whether an answer to "[Y/n]" is a yes: empty (the default), `y` or `yes`.
pub fn is_yes(answer: &str) -> bool {
    matches!(answer.trim().to_ascii_lowercase().as_str(), "" | "y" | "yes")
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

/// `kstar update`, with `--check`, `--yes`, `--no-vscode`, `--beta`,
/// `--no-tui` and `--theme`. On a terminal, when there is something newer,
/// it shows the update screen (`kstar::tui::update`); otherwise it asks a
/// plain question.
pub fn update(args: &[&str]) -> ExitCode {
    let mut check_only = false;
    let mut yes = false;
    let mut beta = false;
    let mut no_tui = false;
    let mut theme = None;
    let mut vscode = std::env::var("KSTAR_NO_VSCODE").map_or(true, |v| v != "1");
    let mut args = args.iter();
    while let Some(&arg) = args.next() {
        match arg {
            "--check" => check_only = true,
            "-y" | "--yes" => yes = true,
            "--no-vscode" => vscode = false,
            "--beta" => beta = true,
            "--no-tui" => no_tui = true,
            "--theme" => match args.next() {
                Some(&name) => theme = Some(name),
                None => return usage("`--theme` needs auto, light, dark or high-contrast"),
            },
            _ if arg.starts_with("--theme=") => theme = Some(&arg["--theme=".len()..]),
            other => return usage(&format!("unknown option `{other}`")),
        }
    }
    let theme = match kstar::theme::resolve(theme) {
        Ok(theme) => theme,
        Err(message) => return usage(&message),
    };
    let current = env!("CARGO_PKG_VERSION");
    let stable = match latest_release() {
        Ok(release) => release,
        Err(message) => {
            eprintln!("kstar: {message}");
            return ExitCode::FAILURE;
        }
    };
    // The screen offers both channels, so it needs the beta too.
    let tui = !check_only && !yes && kstar::tui::wanted(no_tui);
    let newest_beta = if beta || tui { latest_beta() } else { None };
    let channel = if beta { Channel::Beta } else { Channel::Stable };
    let newer = [Some(&stable), newest_beta.as_ref()].into_iter().flatten().any(|r| is_newer(&r.tag, current));
    if tui && newer {
        return update_screen(current, stable, newest_beta, channel, vscode, theme);
    }
    let release = kstar::tui::update::target(channel, Some(&stable), newest_beta.as_ref()).cloned().unwrap_or(stable);
    let latest = &release.tag;
    let which = if beta { "latest release, betas included," } else { "latest release" };
    if !is_newer(latest, current) {
        println!("kstar {current} is up to date (the {which} is {latest})");
        // The extension may still be from another release.
        if vscode {
            if let Err(message) = match_extension(current, check_only) {
                eprintln!("kstar: {message}");
                return ExitCode::FAILURE;
            }
        }
        return ExitCode::SUCCESS;
    }
    println!("installed: kstar {current}");
    println!("latest:    kstar {}", latest.trim_start_matches('v'));
    let summary = headline(&release.notes);
    if !summary.is_empty() {
        println!("\n{summary}\n");
    }
    if check_only {
        println!("run `kstar update{}` to install it", if beta { " --beta" } else { "" });
        if vscode {
            if let Err(message) = match_extension(current, true) {
                eprintln!("kstar: {message}");
                return ExitCode::FAILURE;
            }
        }
        return ExitCode::SUCCESS;
    }
    if !yes {
        use std::io::{BufRead, IsTerminal, Write};
        if !std::io::stdin().is_terminal() {
            eprintln!("kstar: not updating without asking: standard input is not a terminal. Run `kstar update --yes` to update without the question.");
            return ExitCode::FAILURE;
        }
        print!("Update to {latest}? [Y/n] ");
        let _ = std::io::stdout().flush();
        let mut answer = String::new();
        if std::io::stdin().lock().read_line(&mut answer).is_err() || !is_yes(&answer) {
            println!("Nothing was changed.");
            return ExitCode::SUCCESS;
        }
    }
    // Without the screen, the install script's output is shown as it comes.
    let mut show = |progress: Progress| {
        if let Progress::Log(line) = progress {
            println!("{line}");
        }
    };
    match install(latest, vscode, &mut show) {
        Ok(()) => {
            println!("Updated kstar to {latest}.");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("kstar: {message}");
            ExitCode::FAILURE
        }
    }
}

fn usage(message: &str) -> ExitCode {
    eprintln!(
        "kstar: {message}\n\nUsage:\n  kstar update [--check] [--beta] [--yes] [--no-vscode] [--no-tui] [--theme <name>]\n\nRun `kstar help update` for its options and examples."
    );
    ExitCode::from(2)
}

/// Installs in a thread of its own, sending its progress to the screen.
struct InBackground {
    vscode: bool,
}

impl kstar::tui::update::Installer for InBackground {
    fn start(&mut self, tag: &str) -> std::sync::mpsc::Receiver<Progress> {
        let (send, receive) = std::sync::mpsc::channel();
        let (tag, vscode) = (tag.to_string(), self.vscode);
        std::thread::spawn(move || {
            let result = install(&tag, vscode, &mut |p| {
                let _ = send.send(p);
            });
            let _ = send.send(Progress::Finished(result.map(|()| format!("Updated kstar to {tag}."))));
        });
        receive
    }
}

/// The update screen; says afterwards, in the terminal, what happened.
fn update_screen(current: &str, stable: Release, beta: Option<Release>, channel: Channel, vscode: bool, theme: kstar::theme::Choice) -> ExitCode {
    let mut screen = UpdateScreen::new(current, Some(stable), beta, channel, Box::new(InBackground { vscode }));
    if let Err(e) = kstar::tui::run(&mut screen, &kstar::tui::Palette::of(theme)) {
        eprintln!("kstar: the update screen failed ({e}); run `kstar update --no-tui` instead");
        return ExitCode::FAILURE;
    }
    match screen.outcome {
        Outcome::Cancelled => {
            println!("Nothing was changed.");
            ExitCode::SUCCESS
        }
        Outcome::Updated(message) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Outcome::Failed(message) => {
            eprintln!("kstar: {message}");
            ExitCode::FAILURE
        }
    }
}

// ---- installing ----

/// The programs this release installs, as file names in the `bin` folder.
fn program_files() -> [&'static str; 2] {
    if cfg!(windows) { ["kstar.exe", "kstar-lsp.exe"] } else { ["kstar", "kstar-lsp"] }
}

/// What a program is renamed to while it is replaced: `kstar.exe` ->
/// `kstar.old`.
fn old_name(file: &str) -> String {
    format!("{}.old", file.trim_end_matches(".exe"))
}

/// Removes the programs a previous `kstar update` set aside. Called when
/// `kstar` starts; quietly does nothing if they are still running or gone.
pub fn remove_old_programs() {
    if let Some(bin) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_path_buf())) {
        for file in program_files() {
            let old = bin.join(old_name(file));
            if old.exists() {
                let _ = std::fs::remove_file(old);
            }
        }
    }
}

/// Installs release `tag` into the folder above this program's `bin`
/// folder, with the release's install script (`KSTAR_INSTALL_SCRIPT`, a
/// local copy of it, for testing).
///
/// It reports each of `kstar::tui::update::STEPS` and every line the
/// script prints to `report`.
fn install(tag: &str, vscode: bool, report: &mut dyn FnMut(Progress)) -> Result<(), String> {
    report(Progress::Step(0, StepState::Running));
    let exe = std::env::current_exe().map_err(|e| format!("cannot tell where kstar is: {e}"))?;
    let bin = exe.parent().ok_or("kstar is not in a folder")?.to_path_buf();
    let Some(home) = bin.parent().filter(|_| bin.file_name().is_some_and(|n| n.eq_ignore_ascii_case("bin"))) else {
        report(Progress::Step(0, StepState::Failed));
        return Err(format!(
            "kstar is in {}, not in a `bin` folder, so it was not installed by the installer, the install \
             script or cargo. Install {tag} the way you installed this one:\n\n    {}",
            bin.display(),
            install_command()
        ));
    };
    let moved = match set_aside(&bin) {
        Ok(moved) => moved,
        Err(message) => {
            report(Progress::Step(0, StepState::Failed));
            return Err(message);
        }
    };
    report(Progress::Step(0, StepState::Done));
    report(Progress::Step(1, StepState::Running));
    let mut command = match std::env::var_os("KSTAR_INSTALL_SCRIPT") {
        Some(script) if cfg!(windows) => {
            let mut c = Command::new("powershell");
            c.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"]).arg(script);
            c
        }
        Some(script) => {
            let mut c = Command::new("sh");
            c.arg(script);
            c
        }
        None if cfg!(windows) => {
            let mut c = Command::new("powershell");
            c.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command"])
                .arg(format!("irm https://raw.githubusercontent.com/{}/main/install.ps1 | iex", repository()));
            c
        }
        None => {
            let mut c = Command::new("sh");
            c.arg("-c").arg(format!("curl -fsSL https://raw.githubusercontent.com/{}/main/install.sh | sh", repository()));
            c
        }
    };
    command.env("KSTAR_VERSION", tag).env("KSTAR_INSTALL_DIR", home).env("KSTAR_NO_PATH", "1");
    if !vscode {
        command.env("KSTAR_NO_VSCODE", "1");
    }
    let ran = run_reporting(&mut command, report);
    report(Progress::Step(1, if ran { StepState::Done } else { StepState::Failed }));
    let ok = ran && program_files().iter().all(|f| bin.join(f).is_file());
    if ran {
        report(Progress::Step(2, if ok { StepState::Done } else { StepState::Failed }));
    }
    if !ok {
        put_back(&bin, &moved);
        return Err(format!("the install script could not install {tag}; kstar was left as it was"));
    }
    // The script installs the extension when VS Code is there.
    let extension = vscode && code_command().is_some();
    report(Progress::Step(3, if extension { StepState::Done } else { StepState::Skipped }));
    for old in &moved {
        // On Windows the running kstar cannot be deleted; the next start does it.
        let _ = std::fs::remove_file(bin.join(old_name(old)));
    }
    Ok(())
}

/// Runs `command`, sending every line it prints (on either stream) to
/// `report` as it comes. True if it ran and succeeded.
fn run_reporting(command: &mut Command, report: &mut dyn FnMut(Progress)) -> bool {
    use std::io::BufRead;
    command.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let Ok(mut child) = command.spawn() else { return false };
    let (send, receive) = std::sync::mpsc::channel::<String>();
    let mut readers = Vec::new();
    let streams: [Option<Box<dyn std::io::Read + Send>>; 2] =
        [child.stdout.take().map(|s| Box::new(s) as _), child.stderr.take().map(|s| Box::new(s) as _)];
    for stream in streams.into_iter().flatten() {
        let send = send.clone();
        readers.push(std::thread::spawn(move || {
            for line in std::io::BufReader::new(stream).lines().map_while(Result::ok) {
                let _ = send.send(line.trim_end_matches('\r').to_string());
            }
        }));
    }
    drop(send);
    for line in receive {
        report(Progress::Log(line));
    }
    for reader in readers {
        let _ = reader.join();
    }
    child.wait().is_ok_and(|status| status.success())
}

/// Renames the programs in `bin` out of the way; returns the ones moved.
fn set_aside(bin: &std::path::Path) -> Result<Vec<&'static str>, String> {
    let mut moved = Vec::new();
    for file in program_files() {
        let (path, old) = (bin.join(file), bin.join(old_name(file)));
        if !path.exists() {
            continue;
        }
        let _ = std::fs::remove_file(&old);
        if let Err(e) = patiently(|| std::fs::rename(&path, &old)) {
            put_back(bin, &moved);
            return Err(format!("cannot move {} aside to replace it: {e}", path.display()));
        }
        moved.push(file);
    }
    Ok(moved)
}

/// Runs `f` until it succeeds, up to 25 times a fifth of a second apart.
/// On Windows a program that was just written or started can be held for a
/// moment by a virus scanner or the search indexer, and renaming it fails
/// with "used by another process" until they let go.
fn patiently<T>(f: impl FnMut() -> std::io::Result<T>) -> std::io::Result<T> {
    retry(25, std::time::Duration::from_millis(200), f)
}

/// Runs `f` up to `attempts` times, `pause` apart, until it succeeds;
/// returns its last error if it never does.
fn retry<T>(attempts: usize, pause: std::time::Duration, mut f: impl FnMut() -> std::io::Result<T>) -> std::io::Result<T> {
    let mut last = f();
    for _ in 1..attempts {
        if last.is_ok() {
            break;
        }
        std::thread::sleep(pause);
        last = f();
    }
    last
}

/// Undoes `set_aside`.
fn put_back(bin: &std::path::Path, moved: &[&str]) {
    for file in moved {
        let _ = std::fs::remove_file(bin.join(file));
        let _ = patiently(|| std::fs::rename(bin.join(old_name(file)), bin.join(file)));
    }
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
    let work = private_dir()?;
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

/// A new folder for the download that only this user can use, with a name
/// no one can guess in advance. Creating it fails if the path exists, so a
/// folder someone else made in the shared temporary folder is never used:
/// otherwise they could swap the `.vsix` between the checksum and the
/// install.
fn private_dir() -> Result<std::path::PathBuf, String> {
    use std::hash::{BuildHasher, Hasher};
    for _ in 0..8 {
        // RandomState is seeded from the operating system's randomness.
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u32(std::process::id());
        let path = std::env::temp_dir().join(format!("kstar-update-{:016x}", hasher.finish()));
        #[cfg_attr(not(unix), allow(unused_mut))]
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        match builder.create(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("could not make {}: {e}", path.display())),
        }
    }
    Err("could not make a private folder for the download".into())
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
    fn finds_the_newest_beta() {
        let json = r#"[
            {"tag_name": "v0.8.0", "prerelease": false, "body": "stable"},
            {"tag_name": "v0.8.1-beta", "prerelease": true, "body": "newer beta"},
            {"tag_name": "v0.7.5-beta", "prerelease": true, "body": "older beta"},
            {"tag_name": "v0.9.0-beta", "prerelease": true, "draft": true, "body": "a draft"}
        ]"#;
        assert_eq!(newest_beta(json), Some(Release { tag: "v0.8.1-beta".into(), notes: "newer beta".into() }));
        assert_eq!(newest_beta("[]"), None);
        assert_eq!(newest_beta("not json"), None);
    }

    #[test]
    fn reports_every_line_a_command_prints() {
        let mut command = if cfg!(windows) {
            let mut c = Command::new("cmd");
            c.args(["/C", "echo one&& echo two 1>&2"]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", "echo one; echo two >&2"]);
            c
        };
        let mut lines = Vec::new();
        assert!(run_reporting(&mut command, &mut |p| {
            if let Progress::Log(line) = p {
                lines.push(line.trim().to_string());
            }
        }));
        lines.sort();
        assert_eq!(lines, vec!["one", "two"]);
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
    fn download_folders_are_new_and_different() {
        let a = private_dir().unwrap();
        let b = private_dir().unwrap();
        assert_ne!(a, b);
        assert!(a.is_dir() && std::fs::read_dir(&a).unwrap().next().is_none());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&a).unwrap().permissions().mode() & 0o777, 0o700);
        }
        let _ = (std::fs::remove_dir(&a), std::fs::remove_dir(&b));
    }

    #[test]
    fn reads_a_release_and_its_headline() {
        let json = r###"{"tag_name": "v0.5.2", "body": "## What changed\r\n\r\nA fix for `kstar export`:\r\nit writes files.\r\n\r\n### Fixed\r\n- more"}"###;
        let release = parse_release(json).unwrap();
        assert_eq!(release.tag, "v0.5.2");
        assert_eq!(headline(&release.notes), "A fix for `kstar export`: it writes files.");
        assert_eq!(headline(""), "");
        assert!(headline(&"word ".repeat(100)).ends_with("..."));
        assert!(headline(&"word ".repeat(100)).chars().count() <= 300);
        assert!(parse_release("{}").is_err() && parse_release("not json").is_err());
    }

    #[test]
    fn yes_is_the_default_answer() {
        for answer in ["", "\n", "y", "Y\r\n", "yes", " YES "] {
            assert!(is_yes(answer), "{answer:?}");
        }
        for answer in ["n", "no", "N\n", "nope", "q"] {
            assert!(!is_yes(answer), "{answer:?}");
        }
    }

    #[test]
    fn retries_until_it_works() {
        let pause = std::time::Duration::from_millis(1);
        let mut calls = 0;
        let busy = || std::io::Error::other("used by another process");
        let result = retry(5, pause, || {
            calls += 1;
            if calls < 3 { Err(busy()) } else { Ok(calls) }
        });
        assert_eq!(result.unwrap(), 3);
        let mut calls = 0;
        assert!(retry(4, pause, || -> std::io::Result<()> {
            calls += 1;
            Err(busy())
        })
        .is_err());
        assert_eq!(calls, 4);
    }

    #[test]
    fn programs_are_set_aside_and_put_back() {
        let bin = std::env::temp_dir().join(format!("kstar-update-aside-{}", std::process::id())).join("bin");
        let _ = std::fs::remove_dir_all(&bin);
        std::fs::create_dir_all(&bin).unwrap();
        let [kstar, lsp] = program_files();
        std::fs::write(bin.join(kstar), "old kstar").unwrap();
        std::fs::write(bin.join(lsp), "old lsp").unwrap();
        let moved = set_aside(&bin).unwrap();
        assert_eq!(moved, vec![kstar, lsp]);
        assert!(!bin.join(kstar).exists() && bin.join(old_name(kstar)).exists());
        // A failed install leaves the old programs where they were.
        std::fs::write(bin.join(kstar), "half written").unwrap();
        put_back(&bin, &moved);
        assert_eq!(std::fs::read_to_string(bin.join(kstar)).unwrap(), "old kstar");
        assert_eq!(std::fs::read_to_string(bin.join(lsp)).unwrap(), "old lsp");
        assert!(!bin.join(old_name(kstar)).exists());
        let _ = std::fs::remove_dir_all(bin.parent().unwrap());
    }

    #[test]
    fn version_has_four_lines() {
        let text = version();
        assert!(text.starts_with(&format!("kstar {}\ncommit ", env!("CARGO_PKG_VERSION"))));
        assert_eq!(text.lines().count(), 4);
        assert!(text.contains("\nbuilt 20"), "{text}");
    }
}
