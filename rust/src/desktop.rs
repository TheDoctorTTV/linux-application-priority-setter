use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesktopEntry {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub executable: String,
    pub startup_class: String,
}

pub fn find(app_id: Option<&str>, executable: &Path, process_name: &str) -> Option<DesktopEntry> {
    let executable_name = executable
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(process_name)
        .to_lowercase();
    let process_name = process_name.to_lowercase();
    let app_id = app_id.map(normalize);

    entries()
        .iter()
        .filter_map(|entry| {
            let entry_id = normalize(&entry.id);
            let id_tail = entry_id.rsplit('.').next().unwrap_or(&entry_id);
            let score = if app_id.as_deref() == Some(entry_id.as_str()) {
                100
            } else if entry.executable.eq_ignore_ascii_case(&executable_name) {
                80
            } else if entry_id == executable_name || id_tail == executable_name {
                70
            } else if entry.startup_class.eq_ignore_ascii_case(&process_name) {
                60
            } else {
                0
            };
            (score > 0).then_some((score, entry))
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, entry)| entry.clone())
}

fn entries() -> &'static Vec<DesktopEntry> {
    static ENTRIES: OnceLock<Vec<DesktopEntry>> = OnceLock::new();
    ENTRIES.get_or_init(load_entries)
}

fn load_entries() -> Vec<DesktopEntry> {
    let mut entries = Vec::new();
    let mut seen = HashSet::new();

    for directory in application_directories() {
        let Ok(files) = fs::read_dir(directory) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("desktop") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|name| name.to_str()) else {
                continue;
            };
            if !seen.insert(id.to_lowercase()) {
                continue;
            }
            let Ok(contents) = fs::read_to_string(&path) else {
                continue;
            };
            if let Some(entry) = parse_entry(id, &contents) {
                entries.push(entry);
            }
        }
    }

    entries
}

fn application_directories() -> Vec<PathBuf> {
    let data_home = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    let data_dirs = env::var_os("XDG_DATA_DIRS")
        .map(|value| env::split_paths(&value).collect::<Vec<_>>())
        .unwrap_or_else(|| {
            vec![
                PathBuf::from("/usr/local/share"),
                PathBuf::from("/usr/share"),
            ]
        });

    let mut directories = data_home
        .into_iter()
        .chain(data_dirs)
        .map(|directory| directory.join("applications"))
        .collect::<Vec<_>>();

    if let Some(home) = env::var_os("HOME") {
        directories
            .push(PathBuf::from(home).join(".local/share/flatpak/exports/share/applications"));
    }
    directories.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));

    let mut seen = HashSet::new();
    directories.retain(|directory| seen.insert(directory.clone()));
    directories
}

fn parse_entry(id: &str, contents: &str) -> Option<DesktopEntry> {
    let mut in_desktop_entry = false;
    let mut name = String::new();
    let mut icon = String::new();
    let mut exec = String::new();
    let mut startup_class = String::new();
    let mut hidden = false;

    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_desktop_entry || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "Name" => name = value.trim().to_owned(),
            "Icon" => icon = value.trim().to_owned(),
            "Exec" => exec = value.trim().to_owned(),
            "StartupWMClass" => startup_class = value.trim().to_owned(),
            "Hidden" => hidden = value.trim().eq_ignore_ascii_case("true"),
            _ => {}
        }
    }

    if hidden || name.is_empty() {
        return None;
    }

    Some(DesktopEntry {
        id: id.to_owned(),
        name,
        icon,
        executable: executable_from_exec(&exec),
        startup_class,
    })
}

fn executable_from_exec(exec: &str) -> String {
    exec.split_whitespace()
        .map(|token| token.trim_matches(['\'', '"']))
        .skip_while(|token| *token == "env" || token.starts_with('-') || token.contains('='))
        .find(|token| !token.starts_with('%'))
        .and_then(|token| Path::new(token).file_name())
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_lowercase()
}

fn normalize(value: &str) -> String {
    value
        .trim_end_matches(".desktop")
        .trim_start_matches("app-")
        .trim_start_matches("flatpak-")
        .replace("\\x2d", "-")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_desktop_metadata() {
        let entry = parse_entry(
            "org.example.App",
            "[Desktop Entry]\nName=Example App\nExec=env FOO=1 /usr/bin/example-app %U\nIcon=example\nStartupWMClass=example-app\n",
        )
        .unwrap();

        assert_eq!(entry.name, "Example App");
        assert_eq!(entry.executable, "example-app");
        assert_eq!(entry.icon, "example");
    }

    #[test]
    fn ignores_hidden_entries() {
        assert!(parse_entry("hidden", "[Desktop Entry]\nName=Hidden\nHidden=true\n").is_none());
    }
}
