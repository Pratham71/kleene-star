//! Build script: records what `kstar version` reports (the commit, the build
//! date and the target), and on Windows puts the K* icon and version details
//! into `kstar.exe` and `kstar-lsp.exe`.

use std::process::Command;

fn main() {
    // The commit: from the release workflow's environment if it set one,
    // otherwise from git, otherwise unknown (building from a source archive).
    let commit = std::env::var("KSTAR_COMMIT").ok().filter(|c| !c.is_empty()).or_else(|| {
        let output = Command::new("git").args(["rev-parse", "--short=10", "HEAD"]).output().ok()?;
        output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
    });
    println!("cargo:rustc-env=KSTAR_COMMIT={}", commit.unwrap_or_else(|| "unknown".into()));
    println!("cargo:rustc-env=KSTAR_BUILD_DATE={}", build_date());
    println!("cargo:rustc-env=KSTAR_TARGET={}", std::env::var("TARGET").unwrap_or_default());
    println!("cargo:rerun-if-env-changed=KSTAR_COMMIT");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=assets/kstar.ico");

    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("assets/kstar.ico")
            .set("ProductName", "Kleene Star")
            .set("FileDescription", "Kleene Star: a language for Theory of Computation");
        // A missing resource compiler only costs the icon, not the build.
        if let Err(error) = resource.compile() {
            println!("cargo:warning=the Windows icon was not embedded: {error}");
        }
    }
}

/// Today's date (UTC) as YYYY-MM-DD, or the date of `SOURCE_DATE_EPOCH` for
/// reproducible builds.
fn build_date() -> String {
    let seconds = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0)
        });
    let (y, m, d) = civil_from_days(seconds.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// The calendar date of a day number counted from 1970-01-01 (Howard
/// Hinnant's algorithm), to avoid a dependency just for this.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}
