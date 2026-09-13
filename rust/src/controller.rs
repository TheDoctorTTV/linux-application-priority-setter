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
    snapshots: HashMap<String, ApplicationSnapshot>,
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
        type ProcessController = super::ProcessControllerRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut ProcessController>);

        #[qinvokable]
        fn apply_priority(self: Pin<&mut ProcessController>, key: &QString, nice: i32);
    }

    impl cxx_qt::Initialize for ProcessController {}
    impl cxx_qt::Threading for ProcessController {}
}

impl cxx_qt::Initialize for ffi::ProcessController {
    fn initialize(self: std::pin::Pin<&mut Self>) {}
}

impl ffi::ProcessController {
    pub fn refresh(mut self: std::pin::Pin<&mut Self>) {
        if self.rust().busy {
            return;
        }

        self.as_mut().set_busy(true);
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = crate::process::scan_current_user();
            let _ = qt_thread.queue(move |mut controller| {
                controller.as_mut().finish_refresh(result);
            });
        });
    }

    fn finish_refresh(
        mut self: std::pin::Pin<&mut Self>,
        result: std::io::Result<Vec<ApplicationSnapshot>>,
    ) {
        self.as_mut().set_busy(false);
        match result {
            Ok(snapshots) => {
                let applications: Vec<_> = snapshots
                    .iter()
                    .map(|snapshot| &snapshot.application)
                    .collect();
                match serde_json::to_string(&applications) {
                    Ok(json) => {
                        let count = snapshots.len() as i32;
                        self.as_mut().rust_mut().get_mut().snapshots = snapshots
                            .into_iter()
                            .map(|snapshot| (snapshot.application.key.clone(), snapshot))
                            .collect();
                        self.as_mut().set_snapshot_json(QString::from(&json));
                        self.as_mut().set_application_count(count);
                        self.as_mut()
                            .set_status_message(QString::from("Applications updated"));
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
}
