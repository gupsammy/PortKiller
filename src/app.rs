use std::collections::{HashMap, HashSet};
use std::process::Command;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};
use log::{error, warn};
use nix::errno::Errno;
use tray_icon::menu::MenuEvent;
use tray_icon::{TrayIcon, TrayIconBuilder};
use winit::event::{Event, StartCause};
use winit::event_loop::{ControlFlow, EventLoop, EventLoopProxy};

use crate::config::{Config, get_config_path, load_or_create_config, save_config};
use crate::integrations::brew::{query_brew_services_map, run_brew_stop};
use crate::integrations::docker::{query_docker_port_map, run_docker_stop};
use crate::model::*;
use crate::notify::maybe_notify_changes;
use crate::process::kill::terminate_pid;
use crate::process::ports::scan_ports;
use crate::ui::icon::create_icon;
use crate::ui::menu::{
    build_menu_with_context, build_tooltip, collect_targets_for_all, format_command_label,
    parse_menu_action,
};

const POLL_INTERVAL: Duration = Duration::from_secs(2);
// menu constants moved under ui::menu

pub fn run() -> Result<()> {
    let config = load_or_create_config().context("failed to load configuration")?;

    let mut state = AppState {
        processes: Vec::new(),
        last_feedback: None,
        config: config.clone(),
        project_cache: HashMap::new(),
        docker_port_map: HashMap::new(),
        brew_services_map: HashMap::new(),
    };

    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .context("failed to create event loop")?;
    let proxy = event_loop.create_proxy();
    let (worker_tx, worker_rx) = crossbeam_channel::unbounded();

    let _monitor_thread = spawn_monitor_thread(proxy.clone(), config.clone());
    let _menu_thread = spawn_menu_listener(proxy.clone());
    let _worker = spawn_worker(worker_rx, proxy.clone());

    let icon = create_icon(config.ui.inactive_color).context("failed to create tray icon image")?;
    let initial_menu = build_menu_with_context(&state).context("failed to build initial menu")?;
    let tray_icon = TrayIconBuilder::new()
        .with_icon(icon)
        .with_menu(Box::new(initial_menu))
        .with_tooltip("No dev port listeners detected.")
        .build()
        .context("failed to create tray icon")?;
    tray_icon
        .set_visible(true)
        .context("failed to show tray icon")?;

    update_tray_display(&tray_icon, &state);
    let mut worker_sender: Option<Sender<WorkerCommand>> = Some(worker_tx);

    #[allow(deprecated)]
    let run_result = event_loop.run(move |event, event_loop| match event {
        Event::NewEvents(StartCause::Init) => {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
        Event::UserEvent(user_event) => match user_event {
            UserEvent::ProcessesUpdated(processes) => {
                let prev = std::mem::take(&mut state.processes);
                state.processes = processes;
                // Refresh docker port map when we have listeners (if enabled)
                if state.config.integrations.docker_enabled {
                    state.docker_port_map = query_docker_port_map().unwrap_or_default();
                } else {
                    state.docker_port_map.clear();
                }
                // Refresh brew services map when we have listeners (if enabled)
                if state.config.integrations.brew_enabled {
                    state.brew_services_map = query_brew_services_map().unwrap_or_default();
                } else {
                    state.brew_services_map.clear();
                }
                // Derive project info in best-effort mode
                refresh_projects_for(&mut state);
                // Clean up stale cache entries for terminated processes
                let active_pids: HashSet<i32> = state.processes.iter().map(|p| p.pid).collect();
                state
                    .project_cache
                    .retain(|pid, _| active_pids.contains(pid));
                // Notifications on change
                maybe_notify_changes(&state, &prev);
                sync_menu_with_context(&tray_icon, &state);
                update_tray_display(&tray_icon, &state);
            }
            UserEvent::MenuAction(action) => match action {
                MenuAction::EditConfig => {
                    let config_path = get_config_path();
                    let path_str = config_path.to_string_lossy().to_string();
                    let _ = Command::new("open").arg("-t").arg(&path_str).spawn();
                    state.last_feedback = Some(KillFeedback::info(format!(
                        "Opened config file: {}",
                        path_str
                    )));
                    update_tray_display(&tray_icon, &state);
                }
                MenuAction::LaunchAtLogin => {
                    use crate::launch::{
                        disable_launch_at_login, enable_launch_at_login, is_launch_at_login_enabled,
                    };

                    // Toggle the current state
                    let currently_enabled = state.config.system.launch_at_login;
                    let result = if currently_enabled {
                        disable_launch_at_login()
                    } else {
                        enable_launch_at_login()
                    };

                    match result {
                        Ok(()) => {
                            // Verify actual system state and update config based on reality
                            match is_launch_at_login_enabled() {
                                Ok(actual_state) => {
                                    state.config.system.launch_at_login = actual_state;
                                    if let Err(e) = save_config(&state.config) {
                                        state.last_feedback = Some(KillFeedback::error(format!(
                                            "Failed to save config: {}",
                                            e
                                        )));
                                    } else {
                                        state.last_feedback =
                                            Some(KillFeedback::info(if actual_state {
                                                "Launch at login enabled".to_string()
                                            } else {
                                                "Launch at login disabled".to_string()
                                            }));
                                    }
                                }
                                Err(e) => {
                                    state.last_feedback = Some(KillFeedback::error(format!(
                                        "Failed to verify launch-at-login state: {}",
                                        e
                                    )));
                                }
                            }
                        }
                        Err(e) => {
                            // Check if it's the "requires approval" case
                            let msg = e.to_string();
                            if msg.contains("approve") || msg.contains("Login Items") {
                                // Treat as partial success - verify actual state
                                match is_launch_at_login_enabled() {
                                    Ok(actual_state) => {
                                        state.config.system.launch_at_login = actual_state;
                                        let _ = save_config(&state.config);
                                        state.last_feedback = Some(KillFeedback::warning(
                                            "Please approve in System Settings > Login Items"
                                                .to_string(),
                                        ));
                                    }
                                    Err(_) => {
                                        state.last_feedback = Some(KillFeedback::warning(
                                            "Please approve in System Settings > Login Items"
                                                .to_string(),
                                        ));
                                    }
                                }
                            } else {
                                state.last_feedback = Some(KillFeedback::error(format!(
                                    "Failed to toggle launch-at-login: {}",
                                    e
                                )));
                            }
                        }
                    }

                    sync_menu_with_context(&tray_icon, &state);
                    update_tray_display(&tray_icon, &state);
                }
                MenuAction::KillPid { pid, .. } => {
                    if let Some(target) = describe_pid(pid, &state.processes) {
                        if let Some(sender) = worker_sender.as_ref() {
                            if let Err(err) = sender.send(WorkerCommand::KillPid(target)) {
                                let feedback = KillFeedback::error(format!(
                                    "Unable to dispatch kill command: {}",
                                    err
                                ));
                                worker_sender = None;
                                state.last_feedback = Some(feedback);
                                update_tray_display(&tray_icon, &state);
                            }
                        } else {
                            let feedback =
                                KillFeedback::error(format!("Worker unavailable for PID {}.", pid));
                            state.last_feedback = Some(feedback);
                            update_tray_display(&tray_icon, &state);
                        }
                    } else {
                        state.last_feedback = Some(KillFeedback::info(format!(
                            "PID {} is no longer active.",
                            pid
                        )));
                        update_tray_display(&tray_icon, &state);
                    }
                }
                MenuAction::KillAll => {
                    // Filter to only regular processes (exclude Docker and Brew)
                    let regular_processes: Vec<ProcessInfo> = state
                        .processes
                        .iter()
                        .filter(|p| {
                            // Exclude Docker containers
                            if state.docker_port_map.contains_key(&p.port) {
                                return false;
                            }
                            // Exclude Brew services
                            if crate::integrations::brew::get_brew_managed_service(
                                &p.command,
                                p.port,
                                &state.brew_services_map,
                            )
                            .is_some()
                            {
                                return false;
                            }
                            true
                        })
                        .cloned()
                        .collect();

                    let targets = collect_targets_for_all(&regular_processes);
                    if targets.is_empty() {
                        state.last_feedback = Some(KillFeedback::info(
                            "No dev port listeners to terminate.".to_string(),
                        ));
                        update_tray_display(&tray_icon, &state);
                    } else if let Some(sender) = worker_sender.as_ref() {
                        if let Err(err) = sender.send(WorkerCommand::KillAll(targets)) {
                            let feedback = KillFeedback::error(format!(
                                "Unable to dispatch kill-all command: {}",
                                err
                            ));
                            worker_sender = None;
                            state.last_feedback = Some(feedback);
                            update_tray_display(&tray_icon, &state);
                        }
                    } else {
                        let feedback = KillFeedback::error(
                            "Worker unavailable for batch request.".to_string(),
                        );
                        state.last_feedback = Some(feedback);
                        update_tray_display(&tray_icon, &state);
                    }
                }
                MenuAction::Quit => {
                    event_loop.exit();
                }
                MenuAction::DockerStop { container } => {
                    if let Some(sender) = worker_sender.as_ref() {
                        let _ = sender.send(WorkerCommand::DockerStop { container });
                    }
                }
                MenuAction::DockerStopAll => {
                    if let Some(sender) = worker_sender.as_ref() {
                        // Collect all unique Docker containers from current processes
                        let containers: Vec<String> = state
                            .docker_port_map
                            .values()
                            .map(|dc| dc.name.clone())
                            .collect::<HashSet<_>>()
                            .into_iter()
                            .collect();

                        for container in containers {
                            let _ = sender.send(WorkerCommand::DockerStop {
                                container: container.clone(),
                            });
                        }
                    }
                }
                MenuAction::BrewStop { service } => {
                    if let Some(sender) = worker_sender.as_ref() {
                        let _ = sender.send(WorkerCommand::BrewStop { service });
                    }
                }
                MenuAction::BrewStopAll => {
                    if let Some(sender) = worker_sender.as_ref() {
                        // Collect all unique brew services from current processes
                        let services: Vec<String> = state
                            .processes
                            .iter()
                            .filter_map(|p| {
                                crate::integrations::brew::get_brew_managed_service(
                                    &p.command,
                                    p.port,
                                    &state.brew_services_map,
                                )
                            })
                            .collect::<HashSet<_>>()
                            .into_iter()
                            .collect();

                        for service in services {
                            let _ = sender.send(WorkerCommand::BrewStop {
                                service: service.clone(),
                            });
                        }
                    }
                }
            },
            UserEvent::KillFeedback(feedback) => {
                state.last_feedback = Some(feedback);
                update_tray_display(&tray_icon, &state);
            }
            UserEvent::MonitorError(message) => {
                warn!("Monitor error: {}", message);
                state.last_feedback = Some(KillFeedback::error(message));
                update_tray_display(&tray_icon, &state);
            }
        },
        Event::LoopExiting => {
            worker_sender.take();
        }
        _ => {}
    });

    run_result.context("event loop terminated with error")?;
    Ok(())
}

fn spawn_monitor_thread(
    proxy: EventLoopProxy<UserEvent>,
    config: Config,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut previous: Vec<ProcessInfo> = Vec::new();
        loop {
            let scan_start = std::time::Instant::now();
            match scan_ports(&config.monitoring.port_ranges) {
                Ok(mut processes) => {
                    let scan_duration = scan_start.elapsed();
                    processes.sort();
                    if processes != previous {
                        log::debug!(
                            "Change detected (scan took {:?}). Polling immediately for rapid changes.",
                            scan_duration
                        );
                        previous = processes.clone();
                        if proxy
                            .send_event(UserEvent::ProcessesUpdated(processes))
                            .is_err()
                        {
                            break;
                        }
                        continue;
                    } else {
                        log::trace!(
                            "No change (scan took {:?}). Sleeping {}s.",
                            scan_duration,
                            POLL_INTERVAL.as_secs()
                        );
                        thread::sleep(POLL_INTERVAL);
                    }
                }
                Err(err) => {
                    let message = format!("{}", err);
                    if proxy.send_event(UserEvent::MonitorError(message)).is_err() {
                        break;
                    }
                    thread::sleep(POLL_INTERVAL);
                }
            }
        }
    })
}

fn spawn_menu_listener(proxy: EventLoopProxy<UserEvent>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let receiver = MenuEvent::receiver().clone();
        for event in receiver.iter() {
            let Some(action) = parse_menu_action(event.id()) else {
                continue;
            };
            if proxy.send_event(UserEvent::MenuAction(action)).is_err() {
                break;
            }
        }
    })
}

fn spawn_worker(
    rx: Receiver<WorkerCommand>,
    proxy: EventLoopProxy<UserEvent>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for command in rx.iter() {
            let should_continue = match command {
                WorkerCommand::KillPid(target) => handle_single_kill(&proxy, target),
                WorkerCommand::KillAll(targets) => handle_batch_kill(&proxy, targets),
                WorkerCommand::DockerStop { container } => {
                    let feedback = run_docker_stop(&container);
                    proxy.send_event(UserEvent::KillFeedback(feedback)).is_ok()
                }
                WorkerCommand::BrewStop { service } => {
                    let feedback = run_brew_stop(&service);
                    proxy.send_event(UserEvent::KillFeedback(feedback)).is_ok()
                }
            };
            if !should_continue {
                break;
            }
        }
    })
}

fn handle_single_kill(proxy: &EventLoopProxy<UserEvent>, target: KillTarget) -> bool {
    let outcome = terminate_pid(target.pid);
    let feedback = match outcome {
        KillOutcome::Success => {
            KillFeedback::info(format!("Terminated {} (PID {}).", target.label, target.pid))
        }
        KillOutcome::AlreadyExited => KillFeedback::warning(format!(
            "{} (PID {}) was already stopped.",
            target.label, target.pid
        )),
        KillOutcome::PermissionDenied => KillFeedback::error(format!(
            "Permission denied terminating {} (PID {}).",
            target.label, target.pid
        )),
        KillOutcome::TimedOut => KillFeedback::error(format!(
            "Timed out terminating {} (PID {}).",
            target.label, target.pid
        )),
        KillOutcome::Failed(err) => KillFeedback::error(format!(
            "Failed to terminate {} (PID {}): {}.",
            target.label, target.pid, err
        )),
    };

    proxy.send_event(UserEvent::KillFeedback(feedback)).is_ok()
}

fn handle_batch_kill(proxy: &EventLoopProxy<UserEvent>, targets: Vec<KillTarget>) -> bool {
    if targets.is_empty() {
        return proxy
            .send_event(UserEvent::KillFeedback(KillFeedback::info(
                "No dev port listeners to terminate.".to_string(),
            )))
            .is_ok();
    }

    let mut successes = 0usize;
    let mut already = 0usize;
    let mut denied = 0usize;
    let mut timed_out = 0usize;
    let mut failures: Vec<(KillTarget, Errno)> = Vec::new();

    for target in targets {
        match terminate_pid(target.pid) {
            KillOutcome::Success => successes += 1,
            KillOutcome::AlreadyExited => already += 1,
            KillOutcome::PermissionDenied => {
                denied += 1;
                failures.push((target, Errno::EPERM));
            }
            KillOutcome::TimedOut => {
                timed_out += 1;
                failures.push((target, Errno::ETIMEDOUT));
            }
            KillOutcome::Failed(err) => failures.push((target, err)),
        }
    }

    let failure_count = failures.len();
    let severity = if successes > 0 && failure_count == 0 && denied == 0 && timed_out == 0 {
        FeedbackSeverity::Info
    } else if successes > 0 {
        FeedbackSeverity::Warning
    } else {
        FeedbackSeverity::Error
    };

    let mut parts = Vec::new();
    if successes > 0 {
        parts.push(format!("terminated {}", successes));
    }
    if already > 0 {
        parts.push(format!("{} already stopped", already));
    }
    if denied > 0 {
        parts.push(format!("{} permission denied", denied));
    }
    if timed_out > 0 {
        parts.push(format!("{} timed out", timed_out));
    }
    if failure_count > 0 {
        parts.push(format!("{} failed", failure_count));
    }

    if parts.is_empty() {
        parts.push("no action taken".to_string());
    }

    let mut message = format!("Kill all: {}.", parts.join(", "));
    if let Some((failed_target, err)) = failures.first() {
        message.push_str(&format!(
            " First failure: {} (PID {}) — {}.",
            failed_target.label, failed_target.pid, err
        ));
    }

    let feedback = KillFeedback::new(message, severity);
    proxy.send_event(UserEvent::KillFeedback(feedback)).is_ok()
}

// removed: local terminate/scan helpers moved under process::{kill,ports}

// menu id parsing moved to ui::menu

// collect_targets_for_all now in ui::menu

fn describe_pid(pid: i32, processes: &[ProcessInfo]) -> Option<KillTarget> {
    let mut ports = Vec::new();
    let mut command: Option<String> = None;
    for process in processes.iter().filter(|p| p.pid == pid) {
        if !ports.contains(&process.port) {
            ports.push(process.port);
        }
        if command.is_none() || command.as_deref().unwrap().starts_with("pid ") {
            command = Some(process.command.clone());
        }
    }

    if ports.is_empty() {
        return None;
    }

    ports.sort();
    let label = format_command_label(command.as_deref().unwrap_or(""), &ports);
    Some(KillTarget { pid, label })
}

// format_command_label now in ui::menu

// build_menu_with_context moved to ui::menu

// process_menu_id moved to ui::menu

fn sync_menu_with_context(tray_icon: &TrayIcon, state: &AppState) {
    match build_menu_with_context(state) {
        Ok(menu) => tray_icon.set_menu(Some(Box::new(menu))),
        Err(err) => error!("Failed to rebuild menu: {}", err),
    }
}

fn update_tray_display(tray_icon: &TrayIcon, state: &AppState) {
    let color = if state.processes.is_empty() {
        state.config.ui.inactive_color
    } else {
        state.config.ui.active_color
    };

    if let Ok(icon) = create_icon(color) {
        let _ = tray_icon.set_icon(Some(icon));
    }

    let tooltip = build_tooltip(&state.processes, state.last_feedback.as_ref());
    if let Err(err) = tray_icon.set_tooltip(Some(tooltip.as_str())) {
        error!("Failed to update tooltip: {}", err);
    }
}

fn refresh_projects_for(state: &mut AppState) {
    let mut missing: HashSet<i32> = HashSet::new();
    for p in &state.processes {
        if !state.project_cache.contains_key(&p.pid) {
            missing.insert(p.pid);
        }
    }
    for pid in missing {
        if let Some(info) = resolve_project_info(pid) {
            state.project_cache.insert(pid, info);
        }
    }
}

fn resolve_project_info(pid: i32) -> Option<ProjectInfo> {
    let out = Command::new("lsof")
        .args(["-a", "-p", &pid.to_string(), "-d", "cwd", "-Fn"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut cwd: Option<String> = None;
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix('n') {
            cwd = Some(rest.to_string());
            break;
        }
    }
    let cwd = cwd?;
    let path = std::path::PathBuf::from(cwd.clone());
    let git = Command::new("git")
        .args(["-C", &cwd, "rev-parse", "--show-toplevel"])
        .output()
        .ok();
    let name = if let Some(gitout) = git {
        if gitout.status.success() {
            let root = String::from_utf8_lossy(&gitout.stdout).trim().to_string();
            std::path::PathBuf::from(root)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| {
                    path.file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "(unknown)".into())
                })
        } else {
            path.file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "(unknown)".into())
        }
    } else {
        path.file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "(unknown)".into())
    };
    Some(ProjectInfo { name, path })
}

// docker/brew integrations moved to crate::integrations::{docker,brew}

// notifications moved to crate::notify

// build_tooltip and create_icon moved under ui::{menu,icon}
