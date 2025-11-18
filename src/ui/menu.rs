use std::collections::BTreeMap;

use anyhow::Result;
use tray_icon::menu::{Menu, MenuId, MenuItem, PredefinedMenuItem};

use crate::model::{AppState, FeedbackSeverity, KillFeedback, ProcessInfo};

const MAX_TOOLTIP_ENTRIES: usize = 5;
const MENU_ID_KILL_ALL: &str = "kill_all";
const MENU_ID_QUIT: &str = "quit";
const MENU_ID_EDIT_CONFIG: &str = "edit_config";
const MENU_ID_PROCESS_PREFIX: &str = "process_";
const MENU_ID_DOCKER_STOP_PREFIX: &str = "docker_stop_";
const MENU_ID_BREW_STOP_PREFIX: &str = "brew_stop_";
const MENU_ID_EMPTY: &str = "empty";
const MENU_ID_SNOOZE_30M: &str = "snooze_30m";

pub fn build_menu_with_context(state: &AppState) -> Result<Menu> {
    let menu = Menu::new();
    let processes = &state.processes;
    if processes.is_empty() {
        let item = MenuItem::with_id(MENU_ID_EMPTY, "No dev ports listening", false, None);
        menu.append(&item)?;
    } else {
        let mut by_project: BTreeMap<String, Vec<&ProcessInfo>> = BTreeMap::new();
        for p in processes {
            let key = state
                .project_cache
                .get(&p.pid)
                .map(|pi| pi.name.clone())
                .unwrap_or_else(|| "(unknown project)".to_string());
            by_project.entry(key).or_default().push(p);
        }

        let mut total = 0usize;
        for (project, items) in by_project {
            let header = MenuItem::with_id(
                format!("header_{}", project),
                format!("— {} —", project),
                false,
                None,
            );
            menu.append(&header)?;
            for process in items {
                total += 1;

                if let Some(dc) = state.docker_port_map.get(&process.port) {
                    let dlabel = format!("Stop container {} (port {})", dc.name, process.port);
                    let did = format!("{}{}", MENU_ID_DOCKER_STOP_PREFIX, dc.name);
                    let ditem = MenuItem::with_id(&did, &dlabel, true, None);
                    menu.append(&ditem)?;
                } else if let Some(service) = crate::integrations::brew::get_brew_managed_service(
                    &process.command,
                    process.port,
                    &state.brew_services_map,
                ) {
                    let blabel = format!("Stop via brew {}", service);
                    let bid = format!("{}{}", MENU_ID_BREW_STOP_PREFIX, service);
                    let bitem = MenuItem::with_id(&bid, &blabel, true, None);
                    menu.append(&bitem)?;
                } else {
                    let label = format!(
                        "Kill {} (PID {}, port {})",
                        process.command, process.pid, process.port
                    );
                    let item = MenuItem::with_id(
                        MenuId::new(process_menu_id(process.pid, process.port)),
                        label,
                        true,
                        None,
                    );
                    menu.append(&item)?;
                }
            }
            menu.append(&PredefinedMenuItem::separator())?;
        }

        let kill_all_label = format!("Kill all ({})", total);
        let kill_all_item = MenuItem::with_id(MENU_ID_KILL_ALL, kill_all_label, true, None);
        menu.append(&kill_all_item)?;
    }

    menu.append(&PredefinedMenuItem::separator())?;
    let snooze_item = MenuItem::with_id(MENU_ID_SNOOZE_30M, "Snooze notifications 30m", true, None);
    menu.append(&snooze_item)?;
    let edit_config_item =
        MenuItem::with_id(MENU_ID_EDIT_CONFIG, "Edit Configuration...", true, None);
    menu.append(&edit_config_item)?;
    let quit_item = MenuItem::with_id(MENU_ID_QUIT, "Quit", true, None);
    menu.append(&quit_item)?;
    Ok(menu)
}

pub fn process_menu_id(pid: i32, port: u16) -> String {
    format!("{}{}_{}", MENU_ID_PROCESS_PREFIX, pid, port)
}

pub fn parse_menu_action(id: &MenuId) -> Option<crate::model::MenuAction> {
    let raw = id.as_ref();
    if raw == MENU_ID_KILL_ALL {
        Some(crate::model::MenuAction::KillAll)
    } else if raw == MENU_ID_QUIT {
        Some(crate::model::MenuAction::Quit)
    } else if raw == MENU_ID_EDIT_CONFIG {
        Some(crate::model::MenuAction::EditConfig)
    } else if raw == MENU_ID_SNOOZE_30M {
        Some(crate::model::MenuAction::Snooze30m)
    } else if let Some(rest) = raw.strip_prefix(MENU_ID_DOCKER_STOP_PREFIX) {
        Some(crate::model::MenuAction::DockerStop {
            container: sanitize_identifier(rest),
        })
    } else if let Some(rest) = raw.strip_prefix(MENU_ID_BREW_STOP_PREFIX) {
        Some(crate::model::MenuAction::BrewStop {
            service: sanitize_identifier(rest),
        })
    } else if let Some(remainder) = raw.strip_prefix(MENU_ID_PROCESS_PREFIX) {
        let mut parts = remainder.split('_');
        let pid = parts.next()?.parse::<i32>().ok()?;
        let _port = parts.next()?.parse::<u16>().ok()?;
        Some(crate::model::MenuAction::KillPid { pid })
    } else {
        None
    }
}

pub fn build_tooltip(processes: &[ProcessInfo], feedback: Option<&KillFeedback>) -> String {
    let mut lines = Vec::new();
    if processes.is_empty() {
        lines.push("No dev port listeners detected.".to_string());
    } else {
        lines.push(format!("Active listeners: {}", processes.len()));
        for process in processes.iter().take(MAX_TOOLTIP_ENTRIES) {
            lines.push(format!(
                "Port {} → {} (PID {})",
                process.port, process.command, process.pid
            ));
        }
        if processes.len() > MAX_TOOLTIP_ENTRIES {
            lines.push(format!(
                "…and {} more",
                processes.len() - MAX_TOOLTIP_ENTRIES
            ));
        }
    }

    if let Some(feedback) = feedback {
        let prefix = match feedback.severity {
            FeedbackSeverity::Info => "",
            FeedbackSeverity::Warning => "⚠️ ",
            FeedbackSeverity::Error => "⛔ ",
        };
        lines.push(format!("Last action: {}{}", prefix, feedback.message));
    }

    lines.join("\n")
}

fn sanitize_identifier(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .collect()
}

pub fn format_command_label(command: &str, ports: &[u16]) -> String {
    let mut label = if command.is_empty() {
        "Unknown".to_string()
    } else {
        command.to_string()
    };
    if !ports.is_empty() {
        label.push_str(" (port");
        if ports.len() > 1 {
            label.push('s');
        }
        label.push(' ');
        for (idx, port) in ports.iter().enumerate() {
            if idx > 0 {
                label.push_str(", ");
            }
            label.push_str(&port.to_string());
        }
        label.push(')');
    }
    label
}

pub fn collect_targets_for_all(processes: &[ProcessInfo]) -> Vec<crate::model::KillTarget> {
    let mut map: BTreeMap<i32, (String, Vec<u16>)> = BTreeMap::new();

    for process in processes {
        let entry = map
            .entry(process.pid)
            .or_insert_with(|| (process.command.clone(), Vec::new()));
        if !entry.1.contains(&process.port) {
            entry.1.push(process.port);
        }
        if entry.0.is_empty() {
            entry.0 = process.command.clone();
        }
    }

    map.into_iter()
        .filter_map(|(pid, (command, mut ports))| {
            if ports.is_empty() {
                return None;
            }
            ports.sort();
            let label = format_command_label(&command, &ports);
            Some(crate::model::KillTarget { pid, label })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::MenuAction;

    #[test]
    fn parse_simple_actions() {
        assert!(matches!(
            parse_menu_action(&MenuId::new("kill_all")),
            Some(MenuAction::KillAll)
        ));
        assert!(matches!(
            parse_menu_action(&MenuId::new("quit")),
            Some(MenuAction::Quit)
        ));
        assert!(matches!(
            parse_menu_action(&MenuId::new("edit_config")),
            Some(MenuAction::EditConfig)
        ));
        assert!(matches!(
            parse_menu_action(&MenuId::new("snooze_30m")),
            Some(MenuAction::Snooze30m)
        ));
    }

    #[test]
    fn parse_targeted_actions() {
        assert!(matches!(
            parse_menu_action(&MenuId::new("docker_stop_mycontainer")),
            Some(MenuAction::DockerStop { container }) if container == "mycontainer"
        ));
        assert!(matches!(
            parse_menu_action(&MenuId::new("brew_stop_postgresql")),
            Some(MenuAction::BrewStop { service }) if service == "postgresql"
        ));
        assert!(matches!(
            parse_menu_action(&MenuId::new("process_1234_3000")),
            Some(MenuAction::KillPid { pid }) if pid == 1234
        ));
    }

    #[test]
    fn label_formats_ports() {
        assert_eq!(format_command_label("node", &[3000]), "node (port 3000)");
        assert_eq!(
            format_command_label("python", &[8000, 8001]),
            "python (ports 8000, 8001)"
        );
        assert_eq!(format_command_label("", &[]), "Unknown");
    }

    #[test]
    fn collect_targets_groups_by_pid() {
        let p1 = ProcessInfo {
            port: 3000,
            pid: 111,
            command: "node".into(),
        };
        let p2 = ProcessInfo {
            port: 3001,
            pid: 111,
            command: "node".into(),
        };
        let p3 = ProcessInfo {
            port: 5173,
            pid: 222,
            command: "vite".into(),
        };
        let targets = collect_targets_for_all(&[p1, p2, p3]);
        assert_eq!(targets.len(), 2);
        assert!(
            targets
                .iter()
                .any(|t| t.pid == 111 && t.label.contains("3000") && t.label.contains("3001"))
        );
        assert!(
            targets
                .iter()
                .any(|t| t.pid == 222 && t.label.contains("5173"))
        );
    }
}
