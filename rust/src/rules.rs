use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::PathBuf;

pub type SavedRules = BTreeMap<String, i32>;

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct RulesFile {
    #[serde(default = "default_version")]
    version: u32,
    #[serde(default)]
    rules: SavedRules,
}

fn default_version() -> u32 {
    1
}

pub fn clamp_nice(nice: i32) -> i32 {
    nice.clamp(-20, 19)
}

pub fn config_file_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from) {
        return dir
            .join("application-priority-setter")
            .join("rules.json");
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        return home
            .join(".config")
            .join("application-priority-setter")
            .join("rules.json");
    }
    PathBuf::from("rules.json")
}

pub fn load() -> SavedRules {
    load_from_path(&config_file_path())
}

pub fn load_from_path(path: &std::path::Path) -> SavedRules {
    let Ok(contents) = fs::read_to_string(path) else {
        return SavedRules::new();
    };
    // Preferred format: {"version":1,"rules":{...}}
    if let Ok(file) = serde_json::from_str::<RulesFile>(&contents) {
        // Distinguish an actually-empty file ("{}" parses as empty RulesFile)
        // from a legacy flat map. If the raw JSON has a "rules" key, use it.
        if contents.contains("\"rules\"") {
            return sanitize(file.rules);
        }
    }
    // Legacy / tolerant format: {"rule-key": nice, ...}
    if let Ok(flat) = serde_json::from_str::<SavedRules>(&contents) {
        return sanitize(flat);
    }
    SavedRules::new()
}

pub fn save(rules: &SavedRules) -> io::Result<()> {
    save_to_path(rules, &config_file_path())
}

pub fn save_to_path(rules: &SavedRules, path: &std::path::Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = RulesFile {
        version: 1,
        rules: sanitize(rules.clone()),
    };
    let contents = serde_json::to_string_pretty(&file)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    // Atomic write: temp file + rename so a crash can't leave half a file.
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, contents)?;
    fs::rename(&tmp_path, path)?;
    Ok(())
}

fn sanitize(rules: SavedRules) -> SavedRules {
    rules
        .into_iter()
        .filter_map(|(key, nice)| {
            let key = key.trim().to_owned();
            if key.is_empty() {
                return None;
            }
            Some((key, clamp_nice(nice)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "aps-rules-test-{}-{}.json",
            std::process::id(),
            nanos
        ))
    }

    #[test]
    fn round_trips_rules() {
        let path = temp_path();
        let mut rules = SavedRules::new();
        rules.insert("desktop:org.example.App".to_owned(), 10);
        rules.insert("exe:/usr/bin/example".to_owned(), -5);
        save_to_path(&rules, &path).unwrap();
        assert_eq!(load_from_path(&path), rules);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn missing_file_loads_empty() {
        let path = temp_path();
        assert!(load_from_path(&path).is_empty());
    }

    #[test]
    fn clamps_and_drops_bad_keys() {
        let path = temp_path();
        let mut rules = SavedRules::new();
        rules.insert("  ".to_owned(), 0);
        rules.insert("desktop:ok".to_owned(), 99);
        rules.insert("desktop:low".to_owned(), -99);
        save_to_path(&rules, &path).unwrap();
        let loaded = load_from_path(&path);
        assert_eq!(loaded.get("desktop:ok"), Some(&19));
        assert_eq!(loaded.get("desktop:low"), Some(&-20));
        assert!(!loaded.contains_key("  "));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn reads_legacy_flat_format() {
        let path = temp_path();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, r#"{"desktop:org.example.App": 5}"#).unwrap();
        assert_eq!(
            load_from_path(&path).get("desktop:org.example.App"),
            Some(&5)
        );
        let _ = fs::remove_file(&path);
    }
}
