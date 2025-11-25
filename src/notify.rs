use std::collections::HashSet;
use std::process::Command;

use crate::model::{AppState, ProcessInfo};

const BUNDLE_ID: &str = "com.samarthgupta.portkiller";

pub fn maybe_notify_changes(state: &AppState, prev: &[ProcessInfo]) {
    if !state.config.notifications.enabled {
        return;
    }

    let prev_ports: HashSet<u16> = prev.iter().map(|p| p.port).collect();
    let curr_ports: HashSet<u16> = state.processes.iter().map(|p| p.port).collect();

    // Notify for added ports
    let added: Vec<u16> = curr_ports.difference(&prev_ports).copied().collect();
    for port in added {
        if let Some(process) = state.processes.iter().find(|p| p.port == port) {
            let (title, body) = format_notification(port, process, state, true);
            notify(&title, &body);
        }
    }

    // Notify for removed ports
    let removed: Vec<u16> = prev_ports.difference(&curr_ports).copied().collect();
    for port in removed {
        if let Some(process) = prev.iter().find(|p| p.port == port) {
            let (title, body) = format_notification(port, process, state, false);
            notify(&title, &body);
        }
    }
}

fn format_notification(
    port: u16,
    process: &ProcessInfo,
    state: &AppState,
    is_start: bool,
) -> (String, String) {
    let title = if is_start {
        format!("Port {} Started", port)
    } else {
        format!("Port {} Stopped", port)
    };

    let command = truncate_command(&process.command, 40);

    let body = if let Some(project) = state.project_cache.get(&process.pid) {
        format!("{} ({}) • {}", command, process.pid, project.name)
    } else {
        format!("{} ({})", command, process.pid)
    };

    (title, body)
}

fn truncate_command(command: &str, max_len: usize) -> String {
    if command.len() <= max_len {
        command.to_string()
    } else {
        format!("{}...", &command[..max_len.saturating_sub(3)])
    }
}

fn notify(title: &str, body: &str) {
    // Use terminal-notifier only - osascript fallback removed due to command injection risk
    // (malicious process names could contain AppleScript syntax)
    notify_with_terminal_notifier(title, body);
}

fn notify_with_terminal_notifier(title: &str, body: &str) {
    // Find terminal-notifier - check common homebrew paths first (needed for .app bundles
    // where PATH doesn't include homebrew), then fall back to PATH lookup
    let terminal_notifier = [
        "/opt/homebrew/bin/terminal-notifier", // Apple Silicon
        "/usr/local/bin/terminal-notifier",    // Intel Mac
        "terminal-notifier",                   // PATH lookup (works for standalone binary)
    ]
    .iter()
    .find(|path| {
        if path.starts_with('/') {
            std::path::Path::new(path).exists()
        } else {
            // For PATH lookup, try spawning with --help to check if it exists
            Command::new(path).arg("-help").output().is_ok()
        }
    });

    let Some(cmd_path) = terminal_notifier else {
        return;
    };

    let _ = Command::new(cmd_path)
        .args([
            "-title", title, "-message", body, "-sender", BUNDLE_ID, "-sound", "Glass",
        ])
        .spawn();
}
