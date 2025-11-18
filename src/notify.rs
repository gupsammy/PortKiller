use std::collections::HashSet;
use std::process::Command;
use std::time::Instant;

use crate::model::{AppState, ProcessInfo};

pub fn maybe_notify_changes(state: &AppState, prev: &[ProcessInfo]) {
    if !state.config.notifications_enabled {
        return;
    }
    if let Some(until) = state.snooze_until
        && Instant::now() < until
    {
        return;
    }
    let prev_ports: HashSet<u16> = prev.iter().map(|p| p.port).collect();
    let curr_ports: HashSet<u16> = state.processes.iter().map(|p| p.port).collect();
    let added: Vec<u16> = curr_ports.difference(&prev_ports).copied().collect();
    let removed: Vec<u16> = prev_ports.difference(&curr_ports).copied().collect();
    if !added.is_empty() {
        notify(&format!("Ports now listening: {:?}", added));
    }
    if !removed.is_empty() {
        notify(&format!("Ports freed: {:?}", removed));
    }
}

fn notify(message: &str) {
    let msg = message.replace('"', "'");
    let script = format!("display notification \"{}\" with title \"Macport\"", msg);
    let _ = Command::new("osascript").args(["-e", &script]).spawn();
}
