//! How `kstar update` installs a release: entirely in this program.
//!
//! It downloads the release's archive for this platform over HTTPS, checks
//! it against both its `.sha256` file and the release's `SHA256SUMS`,
//! unpacks `kstar` and `kstar-lsp` into a new folder next to the `bin`
//! folder, and swaps them in. It never starts PowerShell, a shell, `curl`
//! or `gh`: a program that downloads a script and pipes it into a shell is
//! exactly what virus scanners look for, and Windows Defender has stopped
//! (and quarantined) kstar for doing that.
//!
//! A running program cannot be overwritten on Windows, but it can be
//! renamed, so each program is renamed to `<name>.old` and the new one moved
//! into its place. If anything fails the old programs are put back. The
//! `.old` files are removed the next time `kstar` starts.

use std::io::Read;
use std::path::{Path, PathBuf};

/// Where release files come from: `https://github.com/<repo>/releases/download`
/// unless `KSTAR_UPDATE_BASE_URL` names another place, which is for testing
/// only. Files are fetched from `<base>/<tag>/<file>`.
pub fn release_base(repo: &str) -> Result<String, String> {
    base_from(std::env::var("KSTAR_UPDATE_BASE_URL").ok(), repo)
}

/// `release_base` with the variable's value given.
fn base_from(value: Option<String>, repo: &str) -> Result<String, String> {
    match value {
        Some(base) if !base.is_empty() => {
            let base = base.trim_end_matches('/').to_string();
            let local = [
                "http://127.0.0.1:",
                "http://localhost:",
                "http://127.0.0.1/",
                "http://localhost/",
            ]
            .iter()
            .any(|p| base.starts_with(p));
            if base.starts_with("https://") || local {
                Ok(base)
            } else {
                Err(format!("KSTAR_UPDATE_BASE_URL must be an https:// address (or http:// on this machine, for testing), not {base}"))
            }
        }
        _ => Ok(format!("https://github.com/{repo}/releases/download")),
    }
}

/// The archive of this platform's programs in a release, as named by
/// `.github/scripts/package.sh`.
pub fn archive_name() -> String {
    let target = env!("KSTAR_TARGET");
    if cfg!(windows) {
        format!("kstar-{target}.zip")
    } else {
        format!("kstar-{target}.tar.gz")
    }
}

/// The programs a release installs, as file names in the `bin` folder.
pub fn program_files() -> [&'static str; 2] {
    if cfg!(windows) {
        ["kstar.exe", "kstar-lsp.exe"]
    } else {
        ["kstar", "kstar-lsp"]
    }
}

/// What a program is renamed to while it is replaced: `kstar.exe` ->
/// `kstar.old`.
pub fn old_name(file: &str) -> String {
    format!("{}.old", file.trim_end_matches(".exe"))
}

// ---- downloading ----

/// The most a download may be; the archives are a few megabytes.
const MOST: u64 = 256 * 1024 * 1024;

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(std::time::Duration::from_secs(30)))
        .timeout_global(Some(std::time::Duration::from_secs(600)))
        .user_agent(concat!("kstar/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

/// Fetches `url` with the given headers. Errors say which address failed
/// and why: the HTTP status, or why it could not be reached.
pub fn get(url: &str, headers: &[(&str, String)]) -> Result<Vec<u8>, Failure> {
    let mut request = agent().get(url);
    for (name, value) in headers {
        request = request.header(*name, value);
    }
    match request.call() {
        Ok(mut response) => response
            .body_mut()
            .with_config()
            .limit(MOST)
            .read_to_vec()
            .map_err(|e| Failure::Network(format!("the download of {url} broke off: {e}"))),
        Err(ureq::Error::StatusCode(status)) => Err(Failure::Status(status, url.to_string())),
        Err(e) => Err(Failure::Network(format!("could not reach {url}: {e}"))),
    }
}

/// Why a download failed.
#[derive(Debug)]
pub enum Failure {
    /// The server answered with this HTTP status.
    Status(u16, String),
    Network(String),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Failure::Status(status, url) => {
                write!(f, "the server answered HTTP {status} for {url}")
            }
            Failure::Network(message) => f.write_str(message),
        }
    }
}

/// GitHub's API, with `GITHUB_TOKEN` if it is set (needed for a private
/// repository).
pub fn github_api(path: &str) -> Result<Vec<u8>, Failure> {
    let mut headers = vec![("Accept", "application/vnd.github+json".to_string())];
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        if !token.is_empty() {
            headers.push(("Authorization", format!("Bearer {token}")));
        }
    }
    get(&format!("https://api.github.com/{path}"), &headers)
}

// ---- checking ----

/// Checks `bytes` against a checksum file's first word.
pub fn check_sum(bytes: &[u8], file: &str, name: &str) -> Result<(), String> {
    let expected = file
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let actual = crate::sha256::hex(bytes);
    if expected != actual {
        return Err(format!("the checksum of {name} does not match (expected {expected}, got {actual}); nothing was installed"));
    }
    Ok(())
}

/// Checks the archive `name` against its own `.sha256` file and against
/// its line in the release's `SHA256SUMS`: both must name its checksum.
pub fn verify(bytes: &[u8], name: &str, own: &str, sums: &str) -> Result<(), String> {
    check_sum(bytes, own, &format!("{name} (from {name}.sha256)"))?;
    let line = sums
        .lines()
        .find(|l| {
            l.split_whitespace()
                .nth(1)
                .map(|f| f.trim_start_matches('*'))
                == Some(name)
        })
        .ok_or_else(|| format!("SHA256SUMS has no line for {name}; nothing was installed"))?;
    check_sum(bytes, line, &format!("{name} (from SHA256SUMS)"))
}

// ---- unpacking ----

/// What `unpack` wrote into its folder: the programs, and the man pages and
/// info manual (in `man/`, from the Linux and macOS archives).
#[derive(Debug, Default)]
pub struct Unpacked {
    pub programs: Vec<&'static str>,
    pub manual: Vec<String>,
}

/// Unpacks the programs (and `man/`) of archive `name` into `into`, a new
/// folder. Only plain file names are written, so an entry like
/// `../../x` cannot write outside it.
pub fn unpack(bytes: &[u8], name: &str, into: &Path) -> Result<Unpacked, String> {
    let mut found = Unpacked::default();
    std::fs::create_dir_all(into.join("man"))
        .map_err(|e| io(&e, &format!("could not make {}", into.display())))?;
    let mut take = |path: &str, read: &mut dyn Read| -> Result<(), String> {
        let path = path.replace('\\', "/");
        let parts: Vec<&str> = path
            .split('/')
            .filter(|p| !p.is_empty() && *p != ".")
            .collect();
        let Some(&file) = parts.last() else {
            return Ok(());
        };
        if parts.contains(&"..") {
            return Ok(());
        }
        let dest = if let Some(program) = program_files().into_iter().find(|p| *p == file) {
            found.programs.push(program);
            into.join(program)
        } else if parts.len() == 2
            && parts[0] == "man"
            && (file.ends_with(".1") || file.ends_with(".info"))
        {
            found.manual.push(file.to_string());
            into.join("man").join(file)
        } else {
            return Ok(());
        };
        write_checked(&dest, read)
    };
    if name.ends_with(".zip") {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
            .map_err(|e| format!("{name} is not a readable zip file: {e}"))?;
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("{name} is damaged: {e}"))?;
            if entry.is_file() {
                let path = entry
                    .name()
                    .map_err(|e| format!("{name} is damaged: {e}"))?
                    .to_string();
                take(&path, &mut entry)?;
            }
        }
    } else {
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
        let entries = archive
            .entries()
            .map_err(|e| format!("{name} is not a readable .tar.gz file: {e}"))?;
        for entry in entries {
            let mut entry = entry.map_err(|e| format!("{name} is damaged: {e}"))?;
            if entry.header().entry_type().is_file() {
                let path = entry
                    .path()
                    .map_err(|e| format!("{name} is damaged: {e}"))?
                    .to_string_lossy()
                    .to_string();
                take(&path, &mut entry)?;
            }
        }
    }
    for program in program_files() {
        if !found.programs.contains(&program) {
            return Err(format!(
                "{name} has no {program} in it; nothing was installed"
            ));
        }
    }
    Ok(found)
}

/// Writes `read` to `dest` (runnable, on Unix), then opens it again to make
/// sure it is still there: a virus scanner may remove a program as soon as
/// it is written.
fn write_checked(dest: &Path, read: &mut dyn Read) -> Result<(), String> {
    let mut file = std::fs::File::create(dest)
        .map_err(|e| io(&e, &format!("could not write {}", dest.display())))?;
    let written = std::io::copy(read, &mut file)
        .map_err(|e| io(&e, &format!("could not write {}", dest.display())))?;
    drop(file);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if program_files()
            .iter()
            .any(|p| dest.file_name().is_some_and(|n| n == *p))
        {
            std::fs::set_permissions(dest, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| io(&e, &format!("could not make {} runnable", dest.display())))?;
        }
    }
    still_there(dest, Some(written))
}

/// Opens `path` to make sure it is there and whole (`len` bytes, if given).
fn still_there(path: &Path, len: Option<u64>) -> Result<(), String> {
    let opened = std::fs::File::open(path).and_then(|f| f.metadata());
    match opened {
        Ok(meta) if len.is_none_or(|len| meta.len() == len) => Ok(()),
        Ok(meta) => Err(format!(
            "{} was written with {} bytes but now has {}.{}",
            path.display(),
            len.unwrap_or(0),
            meta.len(),
            blocked_hint()
        )),
        Err(e) => Err(format!(
            "{} cannot be opened after it was written: {}.{}",
            path.display(),
            describe(&e),
            blocked_hint()
        )),
    }
}

/// What to add when a program disappears or cannot be opened right after it
/// was written: on Windows, most likely the virus scanner took it.
pub fn blocked_hint() -> &'static str {
    if cfg!(windows) {
        " Windows Security may have blocked it; see Windows Security > Protection history."
    } else {
        ""
    }
}

// ---- replacing ----

/// Moves the programs `files` from `staged` into `bin`, each current one
/// renamed to `<name>.old` first. If any step fails, every program already
/// moved is put back and the error says which file and why.
pub fn replace_programs(bin: &Path, staged: &Path, files: &[&str]) -> Result<(), String> {
    replace_programs_trying(bin, staged, files, 25)
}

/// `replace_programs`, trying each rename up to `attempts` times a fifth of
/// a second apart: on Windows a program that was just written or started
/// can be held for a moment by a virus scanner or the search indexer.
pub fn replace_programs_trying(
    bin: &Path,
    staged: &Path,
    files: &[&str],
    attempts: usize,
) -> Result<(), String> {
    let rename = |from: &Path, to: &Path| {
        retry(attempts, std::time::Duration::from_millis(200), || {
            std::fs::rename(from, to)
        })
    };
    let mut done: Vec<(&str, bool)> = Vec::new();
    for &file in files {
        let (path, old, new) = (bin.join(file), bin.join(old_name(file)), staged.join(file));
        let had = path.exists();
        if had {
            // Left over from an update whose program was still running.
            let _ = std::fs::remove_file(&old);
            if let Err(e) = rename(&path, &old) {
                put_back(bin, &done);
                return Err(io(
                    &e,
                    &format!("could not move {} aside to replace it", path.display()),
                ));
            }
        }
        if let Err(e) = rename(&new, &path) {
            put_back(bin, &[(file, had)]);
            put_back(bin, &done);
            let mut message = io(
                &e,
                &format!("could not move the new {file} into {}", bin.display()),
            );
            if e.kind() == std::io::ErrorKind::NotFound {
                message.push_str(blocked_hint());
            }
            return Err(message);
        }
        if let Err(message) = still_there(&path, None) {
            put_back(bin, &[(file, had)]);
            put_back(bin, &done);
            return Err(message);
        }
        done.push((file, had));
    }
    Ok(())
}

/// Undoes `replace_programs` for the files given, last first: removes each
/// new program and renames the old one back, if there was one.
fn put_back(bin: &Path, done: &[(&str, bool)]) {
    for &(file, had) in done.iter().rev() {
        let path = bin.join(file);
        if had {
            let _ = std::fs::remove_file(&path);
            let _ = retry(25, std::time::Duration::from_millis(200), || {
                std::fs::rename(bin.join(old_name(file)), &path)
            });
        }
    }
}

/// Removes the programs a previous `kstar update` renamed to `.old` in
/// `bin`. Quietly leaves them if they are still running.
pub fn remove_old_programs_in(bin: &Path) {
    for file in program_files() {
        let old = bin.join(old_name(file));
        if old.exists() {
            let _ = std::fs::remove_file(old);
        }
    }
}

/// Runs `f` up to `attempts` times, `pause` apart, until it succeeds;
/// returns its last error if it never does.
pub fn retry<T>(
    attempts: usize,
    pause: std::time::Duration,
    mut f: impl FnMut() -> std::io::Result<T>,
) -> std::io::Result<T> {
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

/// `doing` and why it failed, in words: permission denied, the file in use,
/// or what the system said.
pub fn io(e: &std::io::Error, doing: &str) -> String {
    format!("{doing}: {}", describe(e))
}

/// Why a file operation failed, in words.
pub fn describe(e: &std::io::Error) -> String {
    // ERROR_SHARING_VIOLATION and ERROR_LOCK_VIOLATION.
    if cfg!(windows) && matches!(e.raw_os_error(), Some(32 | 33)) {
        return format!("the file is in use by another program; close VS Code and any other kstar and try again ({e})");
    }
    match e.kind() {
        std::io::ErrorKind::PermissionDenied => format!("permission denied ({e})"),
        std::io::ErrorKind::NotFound => format!("the file is not there ({e})"),
        _ => e.to_string(),
    }
}

/// A new folder inside `parent` that only this user can use, with a name
/// no one can guess in advance. Creating it fails if the path exists, so a
/// folder someone else made is never used.
pub fn private_dir_in(parent: &Path, prefix: &str) -> Result<PathBuf, String> {
    use std::hash::{BuildHasher, Hasher};
    for _ in 0..8 {
        // RandomState is seeded from the operating system's randomness.
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u32(std::process::id());
        let path = parent.join(format!("{prefix}{:016x}", hasher.finish()));
        #[cfg_attr(not(unix), allow(unused_mut))]
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        match builder.create(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(io(&e, &format!("could not make {}", path.display()))),
        }
    }
    Err(format!(
        "could not make a private folder in {}",
        parent.display()
    ))
}

/// Copies the man pages and info manual in `staged/man` to
/// `home/share/man/man1` and `home/share/info`, if the install script put
/// them there before. Returns what it did, for the log.
pub fn refresh_manual(
    home: &Path,
    staged: &Path,
    files: &[String],
) -> Result<Option<String>, String> {
    let man1 = home.join("share").join("man").join("man1");
    if !man1.join("kstar.1").is_file() || files.is_empty() {
        return Ok(None);
    }
    let info = home.join("share").join("info");
    for file in files {
        let to = if file.ends_with(".info") {
            info.join(file)
        } else {
            man1.join(file)
        };
        if let Some(dir) = to.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| io(&e, &format!("could not make {}", dir.display())))?;
        }
        std::fs::copy(staged.join("man").join(file), &to)
            .map_err(|e| io(&e, &format!("could not write {}", to.display())))?;
    }
    Ok(Some(format!(
        "Updated {} man and info pages in {}",
        files.len(),
        home.join("share").display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("kstar-selfupdate-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn read(path: PathBuf) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    #[test]
    fn swaps_the_programs_and_keeps_the_old_ones_aside() {
        let dir = scratch("swap");
        let (bin, staged) = (dir.join("bin"), dir.join("new"));
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&staged).unwrap();
        for f in ["kstar.exe", "kstar-lsp.exe"] {
            std::fs::write(bin.join(f), format!("old {f}")).unwrap();
            std::fs::write(staged.join(f), format!("new {f}")).unwrap();
        }
        replace_programs(&bin, &staged, &["kstar.exe", "kstar-lsp.exe"]).unwrap();
        assert_eq!(read(bin.join("kstar.exe")), "new kstar.exe");
        assert_eq!(read(bin.join("kstar-lsp.exe")), "new kstar-lsp.exe");
        assert_eq!(read(bin.join("kstar.old")), "old kstar.exe");
        assert_eq!(read(bin.join("kstar-lsp.old")), "old kstar-lsp.exe");
    }

    #[test]
    fn puts_everything_back_when_a_program_is_missing() {
        let dir = scratch("rollback");
        let (bin, staged) = (dir.join("bin"), dir.join("new"));
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&staged).unwrap();
        for f in ["kstar", "kstar-lsp"] {
            std::fs::write(bin.join(f), format!("old {f}")).unwrap();
        }
        // Only the first new program is there (as if a scanner took the other).
        std::fs::write(staged.join("kstar"), "new kstar").unwrap();
        let err = replace_programs_trying(&bin, &staged, &["kstar", "kstar-lsp"], 1).unwrap_err();
        assert!(
            err.contains("could not move the new kstar-lsp") && err.contains("not there"),
            "{err}"
        );
        assert_eq!(read(bin.join("kstar")), "old kstar");
        assert_eq!(read(bin.join("kstar-lsp")), "old kstar-lsp");
        assert!(!bin.join("kstar.old").exists() && !bin.join("kstar-lsp.old").exists());
    }

    /// A program held open without sharing (as a scanner might) cannot be
    /// renamed: the error says it is in use, and nothing is changed.
    #[test]
    #[cfg(windows)]
    fn a_program_in_use_is_named_and_nothing_changes() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = scratch("in-use");
        let (bin, staged) = (dir.join("bin"), dir.join("new"));
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&staged).unwrap();
        for f in ["kstar.exe", "kstar-lsp.exe"] {
            std::fs::write(bin.join(f), format!("old {f}")).unwrap();
            std::fs::write(staged.join(f), format!("new {f}")).unwrap();
        }
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(bin.join("kstar-lsp.exe"))
            .unwrap();
        let err =
            replace_programs_trying(&bin, &staged, &["kstar.exe", "kstar-lsp.exe"], 2).unwrap_err();
        drop(held);
        assert!(
            err.contains("kstar-lsp.exe") && err.contains("in use by another program"),
            "{err}"
        );
        assert_eq!(read(bin.join("kstar.exe")), "old kstar.exe");
        assert_eq!(read(bin.join("kstar-lsp.exe")), "old kstar-lsp.exe");
    }

    #[test]
    fn old_programs_are_cleaned_up() {
        let dir = scratch("cleanup");
        for f in program_files() {
            std::fs::write(dir.join(old_name(f)), "old").unwrap();
            std::fs::write(dir.join(f), "current").unwrap();
        }
        remove_old_programs_in(&dir);
        for f in program_files() {
            assert!(!dir.join(old_name(f)).exists());
            assert!(dir.join(f).exists());
        }
    }

    #[test]
    fn checks_both_checksums() {
        let bytes = b"archive";
        let sum = crate::sha256::hex(bytes);
        let own = format!("{sum}  kstar-x.zip\n");
        let sums = format!("{}  other.zip\n{sum}  kstar-x.zip\n", "0".repeat(64));
        verify(bytes, "kstar-x.zip", &own, &sums).unwrap();
        let wrong = format!("{}  kstar-x.zip\n", "0".repeat(64));
        assert!(verify(bytes, "kstar-x.zip", &wrong, &sums)
            .unwrap_err()
            .contains("kstar-x.zip.sha256"));
        assert!(verify(bytes, "kstar-x.zip", &own, &wrong)
            .unwrap_err()
            .contains("from SHA256SUMS"));
        assert!(verify(bytes, "kstar-x.zip", &own, "")
            .unwrap_err()
            .contains("no line for kstar-x.zip"));
    }

    #[test]
    fn unpacks_only_the_programs_and_the_manual() {
        let dir = scratch("unpack");
        let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::fast(),
        ));
        let mut add = |path: &str, body: &[u8]| {
            let mut header = tar::Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, path, body).unwrap();
        };
        let [kstar, lsp] = program_files();
        add(&format!("./{kstar}"), b"k");
        add(&format!("./{lsp}"), b"l");
        add("./README.md", b"r");
        add("./man/kstar.1", b"m");
        add("./man/kstar.info", b"i");
        let bytes = tar.into_inner().unwrap().finish().unwrap();
        let out = dir.join("out");
        let found = unpack(&bytes, "kstar-x.tar.gz", &out).unwrap();
        assert_eq!(found.programs, vec![kstar, lsp]);
        assert_eq!(
            found.manual,
            vec!["kstar.1".to_string(), "kstar.info".to_string()]
        );
        assert_eq!(read(out.join(kstar)), "k");
        assert!(!out.join("README.md").exists());

        // An archive without kstar-lsp is refused.
        let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::fast(),
        ));
        let mut header = tar::Header::new_gnu();
        header.set_size(1);
        header.set_cksum();
        tar.append_data(&mut header, kstar, &b"k"[..]).unwrap();
        let bytes = tar.into_inner().unwrap().finish().unwrap();
        let err = unpack(&bytes, "kstar-x.tar.gz", &dir.join("out2")).unwrap_err();
        assert!(err.contains(&format!("has no {lsp}")), "{err}");
    }

    #[test]
    fn unpacks_a_zip() {
        use std::io::Write;
        let dir = scratch("unzip");
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for f in program_files() {
            zip.start_file(
                format!("folder/{f}"),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(f.as_bytes()).unwrap();
        }
        let bytes = zip.finish().unwrap().into_inner();
        let found = unpack(&bytes, "kstar-x.zip", &dir.join("out")).unwrap();
        assert_eq!(found.programs.len(), 2);
        assert_eq!(
            read(dir.join("out").join(program_files()[1])),
            program_files()[1]
        );
    }

    #[test]
    fn only_https_or_this_machine_for_the_test_address() {
        assert_eq!(
            base_from(None, "o/r").unwrap(),
            "https://github.com/o/r/releases/download"
        );
        assert_eq!(
            base_from(Some(String::new()), "o/r").unwrap(),
            "https://github.com/o/r/releases/download"
        );
        assert_eq!(
            base_from(Some("http://127.0.0.1:8080/".into()), "o/r").unwrap(),
            "http://127.0.0.1:8080"
        );
        assert_eq!(
            base_from(Some("https://mirror.example/r".into()), "o/r").unwrap(),
            "https://mirror.example/r"
        );
        assert!(base_from(Some("http://example.com".into()), "o/r")
            .unwrap_err()
            .contains("https://"));
        assert!(base_from(Some("http://localhost.evil.com".into()), "o/r").is_err());
    }

    #[test]
    fn retries_until_it_works() {
        let mut tries = 0;
        let result = retry(5, std::time::Duration::ZERO, || {
            tries += 1;
            if tries < 3 {
                Err(std::io::Error::other("busy"))
            } else {
                Ok(tries)
            }
        });
        assert_eq!(result.unwrap(), 3);
        let mut tries = 0;
        let result: std::io::Result<()> = retry(4, std::time::Duration::ZERO, || {
            tries += 1;
            Err(std::io::Error::other("busy"))
        });
        assert!(result.is_err());
        assert_eq!(tries, 4);
    }
}
