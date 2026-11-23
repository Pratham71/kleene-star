//! The man pages and the info manual in `man/` are made from the help
//! (see `src/manual.rs`). This checks that they are up to date; to write
//! them again after changing the help:
//!
//!     UPDATE_MAN=1 cargo test --test manual

use std::path::Path;

#[test]
fn man_pages_and_info_are_up_to_date() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("man");
    let mut files = kstar::manual::man_pages();
    files.push(("kstar.info".into(), kstar::manual::info_manual()));
    if std::env::var_os("UPDATE_MAN").is_some() {
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (name, text) in &files {
            std::fs::write(dir.join(name), text).unwrap();
        }
    }
    let mut stale = Vec::new();
    for (name, text) in &files {
        let on_disk = std::fs::read_to_string(dir.join(name)).unwrap_or_default().replace("\r\n", "\n");
        if on_disk != *text {
            stale.push(name.as_str());
        }
    }
    assert!(stale.is_empty(), "out of date in man/: {stale:?}; run UPDATE_MAN=1 cargo test --test manual");
    // Nothing else is in the folder.
    let mut on_disk: Vec<String> =
        std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    on_disk.sort();
    let mut expected: Vec<String> = files.iter().map(|(n, _)| n.clone()).collect();
    expected.sort();
    assert_eq!(on_disk, expected);
}
