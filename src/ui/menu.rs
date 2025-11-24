use std::collections::BTreeMap;

use anyhow::Result;
use tray_icon::menu::{Menu, MenuId, MenuItem, PredefinedMenuItem};

use crate::model::{AppState, FeedbackSeverity, KillFeedback, ProcessInfo};

const MAX_TOOLTIP_ENTRIES: usize = 5;
const MENU_ID_KILL_ALL: &str = "kill_all";
const MENU_ID_DOCKER_STOP_ALL: &str = "docker_stop_all";
const MENU_ID_BREW_STOP_ALL: &str = "brew_stop_all";
const MENU_ID_QUIT: &str = "quit";
const MENU_ID_EDIT_CONFIG: &str = "edit_config";
const MENU_ID_LAUNCH_AT_LOGIN: &str = "launch_at_login";
const MENU_ID_PROCESS_PREFIX: &str = "process_";
const MENU_ID_DOCKER_STOP_PREFIX: &str = "docker_stop_";
const MENU_ID_BREW_STOP_PREFIX: &str = "brew_stop_";
const MENU_ID_EMPTY: &str = "empty";

/// Maps common container names to friendly display names
fn friendly_container_name(raw_name: &str) -> String {
    // Strip common prefixes
    let name = raw_name
        .trim_start_matches("macport-")
        .trim_start_matches("test-")
        .trim_start_matches("dev-");

    // Map to friendly names
    match name {
        "postgres" | "postgresql" => "PostgreSQL".to_string(),
        "mongo" | "mongodb" => "MongoDB".to_string(),
        "redis" => "Redis".to_string(),
        "mysql" => "MySQL".to_string(),
        "nginx" => "Nginx".to_string(),
        "rabbitmq" => "RabbitMQ".to_string(),
        "elasticsearch" => "Elasticsearch".to_string(),
        "memcached" => "Memcached".to_string(),
        _ => {
            // Capitalize first letter of unknown containers
            let mut chars = name.chars();
            match chars.next() {
                None => name.to_string(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        }
    }
}

pub fn build_menu_with_context(state: &AppState) -> Result<Menu> {
    let menu = Menu::new();
    let processes = &state.processes;

    if processes.is_empty() {
        let item = MenuItem::with_id(MENU_ID_EMPTY, "No dev ports listening", false, None);
        menu.append(&item)?;
    } else {
        // Separate processes into Docker, Brew, and regular processes
        let mut docker_items: Vec<(&ProcessInfo, &crate::model::DockerContainerInfo)> = Vec::new();
        let mut brew_items: Vec<(&ProcessInfo, String)> = Vec::new();
        let mut regular_processes: Vec<&ProcessInfo> = Vec::new();

        for process in processes {
            if let Some(dc) = state.docker_port_map.get(&process.port) {
                docker_items.push((process, dc));
            } else if let Some(service) = crate::integrations::brew::get_brew_managed_service(
                &process.command,
                process.port,
                &state.brew_services_map,
            ) {
                brew_items.push((process, service));
            } else {
                regular_processes.push(process);
            }
        }

        let mut has_any_section = false;

        // === PROCESSES SECTION ===
        if !regular_processes.is_empty() {
            has_any_section = true;

            // Group by PID to count unique processes
            let mut by_pid: BTreeMap<i32, (String, Vec<u16>)> = BTreeMap::new();
            for p in &regular_processes {
                let entry = by_pid
                    .entry(p.pid)
                    .or_insert_with(|| (p.command.clone(), Vec::new()));
                if !entry.1.contains(&p.port) {
                    entry.1.push(p.port);
                }
            }

            let header = MenuItem::with_id(
                "header_processes",
                format!("Processes · {}", by_pid.len()),
                false,
                None,
            );
            menu.append(&header)?;

            // Create clickable menu item for each process (grouped by PID)
            for (pid, (command, ports)) in &mut by_pid {
                ports.sort();

                // Get project name for this PID
                let project_name = state.project_cache.get(pid).map(|pi| pi.name.clone());

                // Build main menu label: "ports · command · project"
                let ports_str = ports
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");

                let main_label = if let Some(ref project) = project_name {
                    format!("{} · {} · {}", ports_str, command, project)
                } else {
                    format!("{} · {}", ports_str, command)
                };

                // Create clickable menu item that kills the process when clicked
                let process_item = MenuItem::with_id(
                    MenuId::new(process_menu_id(*pid, ports[0])),
                    main_label,
                    true,
                    None,
                );
                menu.append(&process_item)?;
            }

            // Kill All only if multiple processes
            if by_pid.len() > 1 {
                let kill_all =
                    MenuItem::with_id(MENU_ID_KILL_ALL, "Kill All Processes", true, None);
                menu.append(&kill_all)?;
            }
        }

        // === DOCKER SECTION ===
        if !docker_items.is_empty() {
            if has_any_section {
                menu.append(&PredefinedMenuItem::separator())?;
            }
            has_any_section = true;

            // Group by container name
            let mut by_container: BTreeMap<String, Vec<u16>> = BTreeMap::new();
            for (process, dc) in &docker_items {
                by_container
                    .entry(dc.name.clone())
                    .or_default()
                    .push(process.port);
            }

            let header = MenuItem::with_id(
                "header_docker",
                format!("Docker Containers · {}", by_container.len()),
                false,
                None,
            );
            menu.append(&header)?;

            // Check if we need Stop All before consuming the map
            let needs_stop_all = by_container.len() > 1;

            // Create clickable menu item for each container
            for (container_name, mut ports) in by_container {
                ports.sort();
                let friendly = friendly_container_name(&container_name);

                // Build label: "ports · container_name"
                let ports_str = ports
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                let main_label = format!("{} · {}", ports_str, friendly);

                // Create clickable menu item that stops the container when clicked
                let container_item = MenuItem::with_id(
                    format!("{}{}", MENU_ID_DOCKER_STOP_PREFIX, container_name),
                    main_label,
                    true,
                    None,
                );
                menu.append(&container_item)?;
            }

            // Stop All only if multiple containers
            if needs_stop_all {
                let stop_all =
                    MenuItem::with_id(MENU_ID_DOCKER_STOP_ALL, "Stop All Containers", true, None);
                menu.append(&stop_all)?;
            }
        }

        // === BREW SECTION ===
        if !brew_items.is_empty() {
            if has_any_section {
                menu.append(&PredefinedMenuItem::separator())?;
            }

            // Group by service name
            let mut by_service: BTreeMap<String, Vec<u16>> = BTreeMap::new();
            for (process, service) in &brew_items {
                by_service
                    .entry(service.clone())
                    .or_default()
                    .push(process.port);
            }

            let header = MenuItem::with_id(
                "header_brew",
                format!("Brew Services · {}", by_service.len()),
                false,
                None,
            );
            menu.append(&header)?;

            // Check if we need Stop All before consuming the map
            let needs_stop_all = by_service.len() > 1;

            // Create clickable menu item for each service
            for (service_name, mut ports) in by_service {
                ports.sort();

                // Build label: "ports · service_name"
                let ports_str = ports
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                let main_label = format!("{} · {}", ports_str, service_name);

                // Create clickable menu item that stops the service when clicked
                let service_item = MenuItem::with_id(
                    format!("{}{}", MENU_ID_BREW_STOP_PREFIX, service_name),
                    main_label,
                    true,
                    None,
                );
                menu.append(&service_item)?;
            }

            // Stop All only if multiple services
            if needs_stop_all {
                let stop_all =
                    MenuItem::with_id(MENU_ID_BREW_STOP_ALL, "Stop All Services", true, None);
                menu.append(&stop_all)?;
            }
        }
    }

    menu.append(&PredefinedMenuItem::separator())?;
    let edit_config_item =
        MenuItem::with_id(MENU_ID_EDIT_CONFIG, "Edit Configuration...", true, None);
    menu.append(&edit_config_item)?;

    // Add checkable Launch at Login item
    let launch_enabled = state.config.system.launch_at_login;
    let launch_item = MenuItem::with_id(
        MENU_ID_LAUNCH_AT_LOGIN,
        if launch_enabled {
            "✓ Launch at Login"
        } else {
            "Launch at Login"
        },
        true,
        None,
    );
    menu.append(&launch_item)?;

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
    } else if raw == MENU_ID_DOCKER_STOP_ALL {
        Some(crate::model::MenuAction::DockerStopAll)
    } else if raw == MENU_ID_BREW_STOP_ALL {
        Some(crate::model::MenuAction::BrewStopAll)
    } else if raw == MENU_ID_QUIT {
        Some(crate::model::MenuAction::Quit)
    } else if raw == MENU_ID_EDIT_CONFIG {
        Some(crate::model::MenuAction::EditConfig)
    } else if raw == MENU_ID_LAUNCH_AT_LOGIN {
        Some(crate::model::MenuAction::LaunchAtLogin)
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
        assert!(matches!(
            parse_menu_action(&MenuId::new("docker_stop_all")),
            Some(MenuAction::DockerStopAll)
        ));
        assert!(matches!(
            parse_menu_action(&MenuId::new("brew_stop_all")),
            Some(MenuAction::BrewStopAll)
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
