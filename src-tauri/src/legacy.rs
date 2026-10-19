//! Before its rename, Beanbag was called BigScreen. The first start after the rename
//! moves the old data folder and the old "Start with Windows" entry, so nothing is lost
//! and nothing starts twice.

use std::{fs, path::Path};

const OLD_FOLDER: &str = "com.bigscreen.launcher";

/// Moves everything from the old data folder that the new one does not have yet.
/// Returns true when anything moved.
pub fn move_data(data: &Path) -> bool {
    let old = data.with_file_name(OLD_FOLDER);
    let Ok(entries) = fs::read_dir(&old) else { return false };
    let _ = fs::create_dir_all(data);
    let mut moved = false;
    for entry in entries.flatten() {
        let target = data.join(entry.file_name());
        if !target.exists() && fs::rename(entry.path(), &target).is_ok() {
            moved = true;
        }
    }
    // Only an empty folder goes; anything left over stays for a person to look at.
    let _ = fs::remove_dir(&old);
    moved
}

/// Removes the old startup entry. True when there was one, so the new one takes its place.
#[cfg(windows)]
pub fn take_startup_entry() -> bool {
    use windows_sys::Win32::System::Registry::{RegDeleteKeyValueW, HKEY_CURRENT_USER};
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (key, value) = (wide("Software\\Microsoft\\Windows\\CurrentVersion\\Run"), wide("BigScreen"));
    unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr()) == 0 }
}

#[cfg(not(windows))]
pub fn take_startup_entry() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_the_old_folder_without_overwriting_anything_new() {
        let root = std::env::temp_dir().join(format!("beanbag-legacy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let (old, new) = (root.join(OLD_FOLDER), root.join("com.beanbag.launcher"));
        fs::create_dir_all(old.join("art/g")).unwrap();
        fs::write(old.join("library.json"), "old").unwrap();
        fs::write(old.join("art/g/cover.jpg"), "x").unwrap();
        fs::create_dir_all(old.join("EBWebView")).unwrap();
        fs::create_dir_all(new.join("EBWebView")).unwrap();

        assert!(move_data(&new));
        assert_eq!(fs::read_to_string(new.join("library.json")).unwrap(), "old");
        assert!(new.join("art/g/cover.jpg").exists());
        assert!(old.join("EBWebView").exists(), "a folder the new app already has stays behind");
        assert!(!move_data(&new), "a second start moves nothing");

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn no_old_folder_means_nothing_to_do() {
        let root = std::env::temp_dir().join(format!("beanbag-fresh-{}", std::process::id()));
        assert!(!move_data(&root.join("com.beanbag.launcher")));
        let _ = fs::remove_dir_all(&root);
    }
}
