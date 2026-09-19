use crate::process::{ProcessIdentity, process_start_time};

#[derive(Debug, Default)]
pub struct ApplyResult {
    pub changed: usize,
    pub stale: usize,
    pub denied: usize,
    pub failed: usize,
}

pub fn apply(identities: &[ProcessIdentity], nice: i32) -> ApplyResult {
    let mut result = ApplyResult::default();
    let nice = nice.clamp(-20, 19);

    for identity in identities {
        match process_start_time(identity.pid) {
            Ok(start_time) if start_time == identity.start_time => {}
            _ => {
                result.stale += 1;
                continue;
            }
        }

        // SAFETY: setpriority has no pointer arguments. The PID and bounded nice value are
        // validated before the call, and errors are read immediately from errno.
        let code = unsafe { libc::setpriority(libc::PRIO_PROCESS, identity.pid as u32, nice) };
        if code == 0 {
            result.changed += 1;
        } else {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::PermissionDenied {
                result.denied += 1;
            } else {
                result.failed += 1;
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn changes_a_child_process_priority() {
        let mut child = Command::new("sleep").arg("5").spawn().unwrap();
        let pid = child.id() as i32;
        let identity = ProcessIdentity {
            pid,
            start_time: process_start_time(pid).unwrap(),
        };

        let result = apply(&[identity], 5);
        // SAFETY: getpriority takes scalar arguments and the PID belongs to our child.
        let observed = unsafe { libc::getpriority(libc::PRIO_PROCESS, pid as u32) };
        child.kill().unwrap();
        child.wait().unwrap();

        assert_eq!(result.changed, 1);
        assert_eq!(result.denied + result.failed + result.stale, 0);
        assert_eq!(observed, 5);
    }
}
