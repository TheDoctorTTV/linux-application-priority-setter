use crate::desktop;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Application {
    pub key: String,
    pub name: String,
    pub icon: String,
    pub executable: String,
    pub main_pid: i32,
    pub process_count: usize,
    pub pids: Vec<i32>,
    pub nice: i32,
    pub mixed_priority: bool,
}

#[derive(Clone, Debug)]
pub struct ProcessIdentity {
    pub pid: i32,
    pub start_time: u64,
}

#[derive(Clone, Debug)]
struct Process {
    identity: ProcessIdentity,
    parent_pid: i32,
    uid: u32,
    name: String,
    executable: PathBuf,
    nice: i32,
    application_scope: Option<ApplicationScope>,
}

#[derive(Clone, Debug)]
struct ApplicationScope {
    key: String,
    app_id: String,
}

#[derive(Clone, Debug)]
struct ProcessStat {
    name: String,
    parent_pid: i32,
    nice: i32,
    start_time: u64,
}

#[derive(Clone, Debug)]
struct ProcessGroup {
    app_id: Option<String>,
    root_pid: i32,
    processes: Vec<Process>,
}

#[derive(Clone, Debug)]
pub struct ApplicationSnapshot {
    pub application: Application,
    pub processes: Vec<ProcessIdentity>,
}

pub fn scan_current_user() -> io::Result<Vec<ApplicationSnapshot>> {
    let uid = effective_uid()?;
    let this_pid = std::process::id() as i32;
    let mut processes = HashMap::new();

    for entry in fs::read_dir("/proc")? {
        let Ok(entry) = entry else { continue };
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse().ok())
        else {
            continue;
        };
        let Ok(process) = read_process(pid) else {
            continue;
        };
        if process.uid == uid && pid != this_pid {
            processes.insert(pid, process);
        }
    }

    let mut groups: BTreeMap<String, ProcessGroup> = BTreeMap::new();
    for process in processes.values() {
        let (key, app_id, root_pid) = match &process.application_scope {
            Some(scope) => (
                format!("scope:{}", scope.key),
                Some(scope.app_id.clone()),
                0,
            ),
            None => {
                let root_pid = find_tree_root(process.identity.pid, &processes);
                let start_time = processes
                    .get(&root_pid)
                    .map(|root| root.identity.start_time)
                    .unwrap_or(process.identity.start_time);
                (format!("tree:{root_pid}:{start_time}"), None, root_pid)
            }
        };

        groups
            .entry(key)
            .or_insert_with(|| ProcessGroup {
                app_id,
                root_pid,
                processes: Vec::new(),
            })
            .processes
            .push(process.clone());
    }

    let mut applications = groups
        .into_iter()
        .filter_map(|(key, group)| build_application(key, group))
        .collect::<Vec<_>>();
    applications.sort_by(|a, b| {
        a.application
            .name
            .to_lowercase()
            .cmp(&b.application.name.to_lowercase())
            .then_with(|| a.application.main_pid.cmp(&b.application.main_pid))
    });
    Ok(applications)
}

pub fn process_start_time(pid: i32) -> io::Result<u64> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat"))?;
    parse_stat(&stat).map(|stat| stat.start_time)
}

fn build_application(key: String, mut group: ProcessGroup) -> Option<ApplicationSnapshot> {
    group.processes.sort_by_key(|process| process.identity.pid);
    let pid_set = group
        .processes
        .iter()
        .map(|process| process.identity.pid)
        .collect::<HashSet<_>>();
    let root = group
        .processes
        .iter()
        .find(|process| group.root_pid > 0 && process.identity.pid == group.root_pid)
        .or_else(|| {
            group
                .processes
                .iter()
                .find(|process| !pid_set.contains(&process.parent_pid))
        })
        .or_else(|| group.processes.first())?;
    let desktop_entry = desktop::find(group.app_id.as_deref(), &root.executable, &root.name);
    let representative = desktop_entry
        .as_ref()
        .and_then(|entry| {
            group.processes.iter().find(|process| {
                process
                    .executable
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.eq_ignore_ascii_case(&entry.executable))
            })
        })
        .unwrap_or(root);
    let nice = representative.nice;
    let mixed_priority = group.processes.iter().any(|process| process.nice != nice);
    let name = desktop_entry
        .as_ref()
        .map(|entry| entry.name.clone())
        .unwrap_or_else(|| friendly_name(&representative.name, &representative.executable));
    let icon = desktop_entry
        .as_ref()
        .map(|entry| entry.icon.clone())
        .unwrap_or_default();
    let identities = group
        .processes
        .iter()
        .map(|process| process.identity.clone())
        .collect();
    let pids = group
        .processes
        .iter()
        .map(|process| process.identity.pid)
        .collect();

    Some(ApplicationSnapshot {
        application: Application {
            key,
            name,
            icon,
            executable: representative.executable.display().to_string(),
            main_pid: representative.identity.pid,
            process_count: group.processes.len(),
            pids,
            nice,
            mixed_priority,
        },
        processes: identities,
    })
}

fn find_tree_root(pid: i32, processes: &HashMap<i32, Process>) -> i32 {
    let mut current_pid = pid;
    let mut visited = HashSet::new();

    while visited.insert(current_pid) {
        let Some(current) = processes.get(&current_pid) else {
            break;
        };
        let Some(parent) = processes.get(&current.parent_pid) else {
            break;
        };
        if is_launch_boundary(parent) {
            break;
        }
        current_pid = parent.identity.pid;
    }

    current_pid
}

fn is_launch_boundary(process: &Process) -> bool {
    let executable = process
        .executable
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&process.name)
        .to_lowercase();
    matches!(
        executable.as_str(),
        "systemd"
            | "init"
            | "bash"
            | "dash"
            | "fish"
            | "sh"
            | "zsh"
            | "sudo"
            | "doas"
            | "pkexec"
            | "sshd"
            | "konsole"
            | "konsole-bin"
            | "kitty"
            | "alacritty"
            | "wezterm"
            | "gnome-terminal-server"
            | "kgx"
            | "ptyxis"
            | "codex"
            | "hyprland"
            | "uwsm"
            | "plasmashell"
            | "dbus-broker"
            | "xdg-desktop-portal"
            | "xdg-desktop-portal-kde"
            | "xdg-desktop-portal-gtk"
    )
}

fn read_process(pid: i32) -> io::Result<Process> {
    let base = Path::new("/proc").join(pid.to_string());
    let status = fs::read_to_string(base.join("status"))?;
    let uid = status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing process UID"))?;
    let stat = parse_stat(&fs::read_to_string(base.join("stat"))?)?;
    let executable = fs::read_link(base.join("exe")).unwrap_or_default();
    let application_scope = fs::read_to_string(base.join("cgroup"))
        .ok()
        .and_then(|cgroup| parse_application_scope(&cgroup));

    Ok(Process {
        identity: ProcessIdentity {
            pid,
            start_time: stat.start_time,
        },
        parent_pid: stat.parent_pid,
        uid,
        name: stat.name,
        executable,
        nice: stat.nice,
        application_scope,
    })
}

fn parse_stat(stat: &str) -> io::Result<ProcessStat> {
    let open = stat.find('(').ok_or_else(invalid_stat)?;
    let close = stat.rfind(')').ok_or_else(invalid_stat)?;
    let fields = stat[close + 1..].split_whitespace().collect::<Vec<_>>();
    Ok(ProcessStat {
        name: stat[open + 1..close].to_owned(),
        parent_pid: parse_stat_field(&fields, 1)?,
        nice: parse_stat_field(&fields, 16)?,
        start_time: parse_stat_field(&fields, 19)?,
    })
}

fn parse_stat_field<T: std::str::FromStr>(fields: &[&str], index: usize) -> io::Result<T> {
    fields
        .get(index)
        .and_then(|value| value.parse().ok())
        .ok_or_else(invalid_stat)
}

fn parse_application_scope(cgroup: &str) -> Option<ApplicationScope> {
    let scope = cgroup
        .lines()
        .filter_map(|line| line.split_once("::").map(|(_, path)| path))
        .flat_map(|path| path.split('/'))
        .find(|component| component.starts_with("app-") && component.ends_with(".scope"))?;
    let body = scope.trim_start_matches("app-").trim_end_matches(".scope");
    let app_id = body
        .rsplit_once('-')
        .filter(|(_, suffix)| suffix.chars().all(|character| character.is_ascii_digit()))
        .map(|(prefix, _)| prefix)
        .unwrap_or(body)
        .trim_start_matches("flatpak-")
        .replace("\\x2d", "-");

    Some(ApplicationScope {
        key: scope.to_owned(),
        app_id,
    })
}

fn invalid_stat() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid /proc stat data")
}

fn effective_uid() -> io::Result<u32> {
    let status = fs::read_to_string("/proc/self/status")?;
    status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .and_then(|rest| rest.split_whitespace().nth(1))
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing effective UID"))
}

fn friendly_name(process_name: &str, executable: &Path) -> String {
    let candidate = executable
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(process_name)
        .trim_end_matches(".bin")
        .replace(['-', '_'], " ");

    candidate
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_names_with_spaces_and_parentheses() {
        let stat = "42 (name with ) paren) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 -5 17 18 12345";
        let stat = parse_stat(stat).unwrap();
        assert_eq!(stat.name, "name with ) paren");
        assert_eq!(stat.parent_pid, 1);
        assert_eq!(stat.nice, -5);
        assert_eq!(stat.start_time, 12345);
    }

    #[test]
    fn parses_systemd_application_scopes() {
        let scope = parse_application_scope(
            "0::/user.slice/user-1000.slice/user@1000.service/app.slice/app-org.chromium.Chromium-2225278.scope\n",
        )
        .unwrap();

        assert_eq!(scope.key, "app-org.chromium.Chromium-2225278.scope");
        assert_eq!(scope.app_id, "org.chromium.Chromium");
    }

    #[test]
    fn parses_flatpak_application_scopes() {
        let scope = parse_application_scope(
            "0::/user.slice/app.slice/app-flatpak-com.discordapp.Discord-123.scope\n",
        )
        .unwrap();

        assert_eq!(scope.app_id, "com.discordapp.Discord");
    }

    #[test]
    fn formats_executable_names() {
        assert_eq!(
            friendly_name("ignored", Path::new("/usr/bin/my-cool_app")),
            "My Cool App"
        );
    }

    #[test]
    fn groups_descendants_under_the_first_process_after_a_launch_boundary() {
        let mut processes = HashMap::new();
        for process in [
            test_process(10, 1, "bash"),
            test_process(20, 10, "example-app"),
            test_process(21, 20, "example-helper"),
            test_process(22, 21, "example-renderer"),
        ] {
            processes.insert(process.identity.pid, process);
        }

        assert_eq!(find_tree_root(20, &processes), 20);
        assert_eq!(find_tree_root(21, &processes), 20);
        assert_eq!(find_tree_root(22, &processes), 20);
    }

    #[test]
    fn scans_the_current_users_processes() {
        let applications = scan_current_user().unwrap();
        let this_pid = std::process::id() as i32;

        assert!(!applications.is_empty());
        assert!(applications.iter().all(|application| {
            application
                .processes
                .iter()
                .all(|process| process.pid != this_pid)
        }));
    }

    fn test_process(pid: i32, parent_pid: i32, name: &str) -> Process {
        Process {
            identity: ProcessIdentity {
                pid,
                start_time: pid as u64,
            },
            parent_pid,
            uid: 1000,
            name: name.to_owned(),
            executable: PathBuf::from(format!("/usr/bin/{name}")),
            nice: 0,
            application_scope: None,
        }
    }
}
