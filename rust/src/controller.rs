use crate::process::ApplicationSnapshot;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use std::collections::HashMap;

#[derive(Default)]
pub struct ProcessControllerRust {
    snapshot_json: QString,
    status_message: QString,
    application_count: i32,
    busy: bool,
    autostart_enabled: bool,
    snapshots: HashMap<String, ApplicationSnapshot>,
}

struct RefreshReport {
    snapshots: Vec<ApplicationSnapshot>,
    auto_changed: usize,
    auto_apps: usize,
    auto_denied: usize,
}

#[cxx_qt::bridge]
mod ffi {
    #[namespace = ""]
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, snapshot_json)]
        #[qproperty(QString, status_message)]
        #[qproperty(i32, application_count)]
        #[qproperty(bool, busy)]
        #[qproperty(bool, autostart_enabled)]
        type ProcessController = super::ProcessControllerRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut ProcessController>);

        #[qinvokable]
        fn apply_priority(self: Pin<&mut ProcessController>, key: &QString, nice: i32);

        #[qinvokable]
        fn save_priority(self: Pin<&mut ProcessController>, key: &QString, nice: i32);

        #[qinvokable]
        fn clear_saved_priority(self: Pin<&mut ProcessController>, key: &QString);

        #[qinvokable]
        fn set_autostart(self: Pin<&mut ProcessController>, enabled: bool);
    }

    impl cxx_qt::Initialize for ProcessController {}
    impl cxx_qt::Threading for ProcessController {}
}

impl cxx_qt::Initialize for ffi::ProcessController {
    fn initialize(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut()
            .set_autostart_enabled(crate::autostart::is_enabled());
    }
}

impl ffi::ProcessController {
    pub fn refresh(mut self: std::pin::Pin<&mut Self>) {
        if self.rust().busy {
            return;
        }

        self.as_mut().set_busy(true);
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = refresh_in_background();
            let _ = qt_thread.queue(move |mut controller| {
                controller.as_mut().finish_refresh(result);
            });
        });
    }

    fn finish_refresh(
        mut self: std::pin::Pin<&mut Self>,
        result: std::io::Result<RefreshReport>,
    ) {
        self.as_mut().set_busy(false);
        match result {
            Ok(report) => {
                let applications: Vec<_> =
                    report.snapshots.iter().map(|snapshot| &snapshot.application).collect();
                match serde_json::to_string(&applications) {
                    Ok(json) => {
                        let count = report.snapshots.len() as i32;
                        self.as_mut().rust_mut().get_mut().snapshots = report
                            .snapshots
                            .into_iter()
                            .map(|snapshot| (snapshot.application.key.clone(), snapshot))
                            .collect();
                        self.as_mut().set_snapshot_json(QString::from(&json));
                        self.as_mut().set_application_count(count);
                        let mut message = String::from("Applications updated");
                        if report.auto_changed > 0 {
                            message.push_str(&format!(
                                " · re-applied {} saved priorit{} ({} process(es))",
                                report.auto_apps,
                                if report.auto_apps == 1 { "y" } else { "ies" },
                                report.auto_changed
                            ));
                        }
                        if report.auto_denied > 0 {
                            message.push_str(&format!(
                                " · {} process(es) need authorization",
                                report.auto_denied
                            ));
                        }
                        self.as_mut().set_status_message(QString::from(&message));
                    }
                    Err(error) => self.as_mut().set_status_message(QString::from(&format!(
                        "Could not prepare the process list: {error}"
                    ))),
                }
            }
            Err(error) => self.as_mut().set_status_message(QString::from(&format!(
                "Could not read running applications: {error}"
            ))),
        }
    }

    pub fn apply_priority(mut self: std::pin::Pin<&mut Self>, key: &QString, nice: i32) {
        let key = key.to_string();
        let Some(snapshot) = self.rust().snapshots.get(&key) else {
            self.as_mut().set_status_message(QString::from(
                "That application is no longer in the current process list",
            ));
            return;
        };

        let name = snapshot.application.name.clone();
        let result = crate::priority::apply(&snapshot.processes, nice);
        let message = if result.denied > 0 {
            format!(
                "Changed {} process(es) for {name}; {} require authorization",
                result.changed, result.denied
            )
        } else if result.failed > 0 || result.stale > 0 {
            format!(
                "Changed {} process(es) for {name}; {} exited and {} failed",
                result.changed, result.stale, result.failed
            )
        } else {
            format!(
                "Priority updated for {name} ({} process(es))",
                result.changed
            )
        };
        self.as_mut().set_status_message(QString::from(&message));
    }

    pub fn save_priority(mut self: std::pin::Pin<&mut Self>, key: &QString, nice: i32) {
        let key = key.to_string();
        let Some(snapshot) = self.rust().snapshots.get(&key).cloned() else {
            self.as_mut().set_status_message(QString::from(
                "That application is no longer in the current process list",
            ));
            return;
        };

        let nice = crate::rules::clamp_nice(nice);
        let rule_key = snapshot.application.rule_key.clone();
        let name = snapshot.application.name.clone();
        let mut rules = crate::rules::load();
        rules.insert(rule_key.clone(), nice);
        if let Err(error) = crate::rules::save(&rules) {
            self.as_mut().set_status_message(QString::from(&format!(
                "Could not save the priority for {name}: {error}"
            )));
            return;
        }

        // Update the in-memory snapshots so the UI shows the saved value
        // without waiting for the next refresh.
        for stored in self.as_mut().rust_mut().get_mut().snapshots.values_mut() {
            if stored.application.rule_key == rule_key {
                stored.application.saved_nice = Some(nice);
            }
        }
        self.as_mut().refresh_snapshot_json();

        let result = crate::priority::apply(&snapshot.processes, nice);
        let message = if result.denied > 0 {
            format!(
                "Saved priority {nice} for {name}; {} process(es) need authorization and will apply later",
                result.denied
            )
        } else {
            format!(
                "Saved priority {nice} for {name} ({} process(es) updated)",
                result.changed
            )
        };
        self.as_mut().set_status_message(QString::from(&message));
    }

    pub fn clear_saved_priority(mut self: std::pin::Pin<&mut Self>, key: &QString) {
        let key = key.to_string();
        let Some(snapshot) = self.rust().snapshots.get(&key).cloned() else {
            self.as_mut().set_status_message(QString::from(
                "That application is no longer in the current process list",
            ));
            return;
        };

        let rule_key = snapshot.application.rule_key.clone();
        let name = snapshot.application.name.clone();
        let mut rules = crate::rules::load();
        if rules.remove(&rule_key).is_none() {
            self.as_mut().set_status_message(QString::from(&format!(
                "No saved priority for {name}"
            )));
            return;
        }
        if let Err(error) = crate::rules::save(&rules) {
            self.as_mut().set_status_message(QString::from(&format!(
                "Could not forget the priority for {name}: {error}"
            )));
            return;
        }

        for stored in self.as_mut().rust_mut().get_mut().snapshots.values_mut() {
            if stored.application.rule_key == rule_key {
                stored.application.saved_nice = None;
            }
        }
        self.as_mut().refresh_snapshot_json();
        self.as_mut().set_status_message(QString::from(&format!(
            "Forgot the saved priority for {name}"
        )));
    }

    pub fn set_autostart(mut self: std::pin::Pin<&mut Self>, enabled: bool) {
        match crate::autostart::set_enabled(enabled) {
            Ok(()) => {
                self.as_mut().set_autostart_enabled(enabled);
                self.as_mut().set_status_message(QString::from(if enabled {
                    "Application Priority Setter will start when you log in"
                } else {
                    "Application Priority Setter will no longer start at login"
                }));
            }
            Err(error) => self.as_mut().set_status_message(QString::from(&format!(
                "Could not change the startup setting: {error}"
            ))),
        }
    }

    fn refresh_snapshot_json(mut self: std::pin::Pin<&mut Self>) {
        let applications: Vec<_> = self
            .rust()
            .snapshots
            .values()
            .map(|snapshot| &snapshot.application)
            .collect();
        if let Ok(json) = serde_json::to_string(&applications) {
            self.as_mut().set_snapshot_json(QString::from(&json));
        }
    }
}

fn refresh_in_background() -> std::io::Result<RefreshReport> {
    let rules = crate::rules::load();
    let mut snapshots = crate::process::scan_current_user()?;
    crate::process::attach_saved_rules(&mut snapshots, &rules);

    let mut auto_changed = 0usize;
    let mut auto_apps = 0usize;
    let mut auto_denied = 0usize;
    let mut auto_failed = 0usize;

    for snapshot in snapshots.iter_mut() {
        let Some(desired) = snapshot.application.saved_nice else {
            continue;
        };
        let already_ok =
            !snapshot.application.mixed_priority && snapshot.application.nice == desired;
        if already_ok {
            continue;
        }
        let result = crate::priority::apply(&snapshot.processes, desired);
        if result.changed > 0 {
            auto_changed += result.changed;
            auto_apps += 1;
            // Optimistic UI update on full success so the list doesn't flicker
            // until the next refresh.
            if result.denied == 0 && result.failed == 0 && result.stale == 0 {
                snapshot.application.nice = desired;
                snapshot.application.mixed_priority = false;
            }
        }
        auto_denied += result.denied;
        auto_failed += result.failed + result.stale;
    }
    let _ = auto_failed;

    Ok(RefreshReport {
        snapshots,
        auto_changed,
        auto_apps,
        auto_denied,
    })
}
