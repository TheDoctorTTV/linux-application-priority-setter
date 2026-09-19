//! Manages the XDG autostart entry ("start with the system").
//!
//! Enabled = a copy of the application's `.desktop` file exists at
//! `$XDG_CONFIG_HOME/autostart/io.github.applicationprioritysetter.desktop`
//! (usually `~/.config/autostart/...`). Disabling removes it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const DESKTOP_FILE_NAME: &str = "io.github.applicationprioritysetter.desktop";
const AUTOSTART_DIR_NAME: &str = "autostart";

pub fn autostart_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"));
    base.join(AUTOSTART_DIR_NAME)
}

pub fn autostart_path() -> PathBuf {
    autostart_dir().join(DESKTOP_FILE_NAME)
}

/// Ground truth: the entry exists and is not explicitly hidden.
pub fn is_enabled() -> bool {
    is_enabled_at(&autostart_path())
}

fn is_enabled_at(path: &Path) -> bool {
    let Ok(contents) = fs::read_to_string(path) else {
        return false;
    };
    !contents.lines().any(|line| {
        line.trim().eq_ignore_ascii_case("Hidden=true")
            || line.trim().eq_ignore_ascii_case("X-GNOME-Autostart-enabled=false")
    })
}

pub fn set_enabled(enabled: bool) -> io::Result<()> {
    set_enabled_at(&autostart_path(), enabled)
}

fn set_enabled_at(path: &Path, enabled: bool) -> io::Result<()> {
    if !enabled {
        return match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        };
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let contents = entry_contents();
    // Atomic write so a crash can't leave half a file.
    let tmp_path = path.with_extension("desktop.tmp");
    fs::write(&tmp_path, contents)?;
    fs::rename(&tmp_path, path)?;
    Ok(())
}

/// Prefers a byte-for-byte copy of the installed desktop entry so Name, Icon
/// and Categories stay in sync with the packaged app. Falls back to a minimal
/// generated entry (e.g. when running uninstalled from the build directory).
fn entry_contents() -> String {
    if let Some(contents) = find_installed_entry() {
        return ensure_autostart_flags(&contents);
    }
    let exec = std::env::current_exe()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "application-priority-setter".to_owned());
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Application Priority Setter\n\
         Comment=View running applications and change their CPU priority\n\
         Exec={exec}\n\
         Icon=utilities-system-monitor\n\
         Terminal=false\n\
         Categories=System;Utility;\n\
         X-GNOME-Autostart-enabled=true\n"
    )
}

fn find_installed_entry() -> Option<String> {
    for dir in system_application_dirs() {
        let candidate = dir.join(DESKTOP_FILE_NAME);
        if let Ok(contents) = fs::read_to_string(&candidate)
            && contents.contains("[Desktop Entry]")
        {
            return Some(contents);
        }
    }
    None
}

fn system_application_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        dirs.push(home.join(".local/share/applications"));
        dirs.push(home.join(".local/share/flatpak/exports/share/applications"));
    }
    if let Some(data_dirs) = std::env::var_os("XDG_DATA_DIRS") {
        dirs.extend(std::env::split_paths(&data_dirs).map(|dir| dir.join("applications")));
    } else {
        dirs.push(PathBuf::from("/usr/local/share/applications"));
        dirs.push(PathBuf::from("/usr/share/applications"));
    }
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    dirs
}

/// Marks a copied entry as autostart-suitable without touching anything else.
fn ensure_autostart_flags(contents: &str) -> String {
    let mut out = String::with_capacity(contents.len() + 64);
    let mut has_gnome_flag = false;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("Hidden=true") {
            out.push_str("Hidden=false\n");
        } else {
            has_gnome_flag |= trimmed.eq_ignore_ascii_case("X-GNOME-Autostart-enabled=true");
            out.push_str(line);
            out.push('\n');
        }
    }
    if !has_gnome_flag {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("X-GNOME-Autostart-enabled=true\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "aps-autostart-test-{}-{}.d",
            std::process::id(),
            nanos
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn temp_entry(dir: &Path) -> PathBuf {
        dir.join("test-app.desktop")
    }

    #[test]
    fn enable_disable_round_trip() {
        let dir = temp_dir();
        let path = temp_entry(&dir);

        assert!(!is_enabled_at(&path));
        // Disabling when absent is a no-op success.
        set_enabled_at(&path, false).unwrap();

        set_enabled_at(&path, true).unwrap();
        assert!(is_enabled_at(&path));
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("[Desktop Entry]"));
        assert!(contents.contains("X-GNOME-Autostart-enabled=true"));

        set_enabled_at(&path, false).unwrap();
        assert!(!is_enabled_at(&path));
        assert!(!path.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn hidden_entry_counts_as_disabled() {
        let dir = temp_dir();
        let path = temp_entry(&dir);
        fs::write(&path, "[Desktop Entry]\nHidden=true\n").unwrap();
        assert!(!is_enabled_at(&path));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn copied_entry_is_unhidden_and_flagged() {
        let fixed = ensure_autostart_flags("[Desktop Entry]\nHidden=true\nExec=foo\n");
        assert!(fixed.contains("Hidden=false"));
        assert!(!fixed.contains("Hidden=true"));
        assert!(fixed.contains("X-GNOME-Autostart-enabled=true"));
        // No duplicated flag when already present.
        let fixed = ensure_autostart_flags("[Desktop Entry]\nX-GNOME-Autostart-enabled=true\n");
        assert_eq!(fixed.matches("X-GNOME-Autostart-enabled=true").count(), 1);
    }
}
