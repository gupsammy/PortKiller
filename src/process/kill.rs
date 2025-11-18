use std::thread;
use std::time::Duration;

use nix::errno::Errno;
use nix::sys::signal::{Signal, kill};
use nix::unistd::{Pid, getpgid};

use crate::model::KillOutcome;

const SIGTERM_GRACE: Duration = Duration::from_secs(2);
const SIGKILL_GRACE: Duration = Duration::from_secs(1);
const POLL_STEP: Duration = Duration::from_millis(200);

pub fn terminate_pid(pid_raw: i32) -> KillOutcome {
    let pid = Pid::from_raw(pid_raw);

    match kill(pid, None) {
        Err(Errno::ESRCH) => return KillOutcome::AlreadyExited,
        Err(err) => return KillOutcome::Failed(err),
        Ok(()) => {}
    }

    let mut last_perm_denied = false;
    let mut tried_group = false;

    if let Ok(pgid) = getpgid(Some(pid)) {
        let raw = pgid.as_raw();
        if raw > 0 {
            tried_group = true;
            let gpid = Pid::from_raw(-raw);
            match kill(gpid, Signal::SIGTERM) {
                Ok(()) => {}
                Err(Errno::ESRCH) => {}
                Err(Errno::EPERM) => last_perm_denied = true,
                Err(err) => return KillOutcome::Failed(err),
            }
            match wait_for_exit(pid, SIGTERM_GRACE) {
                Ok(true) => return KillOutcome::Success,
                Ok(false) => {}
                Err(err) => return KillOutcome::Failed(err),
            }
            match kill(gpid, Signal::SIGKILL) {
                Ok(()) => {}
                Err(Errno::ESRCH) => {}
                Err(Errno::EPERM) => last_perm_denied = true,
                Err(err) => return KillOutcome::Failed(err),
            }
            match wait_for_exit(pid, SIGKILL_GRACE) {
                Ok(true) => return KillOutcome::Success,
                Ok(false) => {}
                Err(err) => return KillOutcome::Failed(err),
            }
        }
    }

    match kill(pid, Signal::SIGTERM) {
        Ok(()) => {}
        Err(Errno::ESRCH) => return KillOutcome::AlreadyExited,
        Err(Errno::EPERM) => last_perm_denied = true,
        Err(err) => return KillOutcome::Failed(err),
    }
    match wait_for_exit(pid, SIGTERM_GRACE) {
        Ok(true) => return KillOutcome::Success,
        Ok(false) => {}
        Err(err) => return KillOutcome::Failed(err),
    }
    match kill(pid, Signal::SIGKILL) {
        Ok(()) => {}
        Err(Errno::ESRCH) => return KillOutcome::Success,
        Err(Errno::EPERM) => last_perm_denied = true,
        Err(err) => return KillOutcome::Failed(err),
    }
    match wait_for_exit(pid, SIGKILL_GRACE) {
        Ok(true) => KillOutcome::Success,
        Ok(false) => {
            if tried_group && last_perm_denied {
                KillOutcome::PermissionDenied
            } else {
                KillOutcome::TimedOut
            }
        }
        Err(err) => KillOutcome::Failed(err),
    }
}

fn wait_for_exit(pid: Pid, timeout: Duration) -> Result<bool, Errno> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match kill(pid, None) {
            Err(Errno::ESRCH) => return Ok(true),
            Err(err) => return Err(err),
            Ok(()) => {}
        }

        if std::time::Instant::now() >= deadline {
            return Ok(false);
        }
        thread::sleep(POLL_STEP);
    }
}
