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
    // Try terminal-notifier first (better sound and icon support)
    if notify_with_terminal_notifier(title, body) {
        return;
    }
    // Fallback to osascript
    notify_with_osascript(title, body);
}

fn notify_with_terminal_notifier(title: &str, body: &str) -> bool {
    // Check if running from .app bundle and get icon path
    let current_exe = std::env::current_exe().ok();
    let app_icon_url = current_exe.as_ref().and_then(|exe| {
        let exe_str = exe.to_str()?;
        if exe_str.contains(".app/Contents/MacOS") {
            // Use PNG icon in Resources for terminal-notifier
            let app_path = exe_str.split(".app/").next()?;
            let png_path = format!("{}.app/Contents/Resources/AppIcon.png", app_path);
            // Verify the file exists
            if std::path::Path::new(&png_path).exists() {
                Some(format!("file://{}", png_path))
            } else {
                None
            }
        } else {
            None
        }
    });

    let mut cmd = Command::new("terminal-notifier");
    cmd.args(["-title", title, "-message", body, "-sound", "Glass"]);

    // Add explicit icon URL when running from .app bundle
    if let Some(icon_url) = &app_icon_url {
        cmd.args(["-appIcon", icon_url]);
    } else {
        // Fallback to sender bundle ID for standalone binary
        cmd.args(["-sender", BUNDLE_ID]);
    }

    cmd.spawn().is_ok()
}

fn notify_with_osascript(title: &str, body: &str) {
    let title_escaped = title.replace('"', "'");
    let body_escaped = body.replace('"', "'");
    let script = format!(
        "display notification \"{}\" with title \"{}\" sound name \"Glass\"",
        body_escaped, title_escaped
    );
    let _ = Command::new("osascript").args(["-e", &script]).spawn();
}
