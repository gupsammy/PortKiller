use std::collections::HashMap;
use std::path::PathBuf;

use nix::errno::Errno;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ProcessInfo {
    pub port: u16,
    pub pid: i32,
    pub command: String,
}

#[derive(Clone, Debug)]
pub enum UserEvent {
    ProcessesUpdated(Vec<ProcessInfo>),
    MenuAction(MenuAction),
    KillFeedback(KillFeedback),
    MonitorError(String),
    ConfigReloaded(crate::config::Config),
    ConfigReloadFailed(String),
}

#[derive(Clone, Debug)]
pub enum MenuAction {
    KillPid { pid: i32 },
    KillAll,
    DockerStop { container: String },
    DockerStopAll,
    BrewStop { service: String },
    BrewStopAll,
    EditConfig,
    ReloadConfig,
    LaunchAtLogin,
    Quit,
}

#[derive(Clone, Debug)]
pub enum WorkerCommand {
    KillPid(KillTarget),
    KillAll(Vec<KillTarget>),
    DockerStop { container: String },
    BrewStop { service: String },
}

#[derive(Clone, Debug)]
pub struct KillTarget {
    pub pid: i32,
    pub label: String,
}

#[derive(Clone, Debug)]
pub struct KillFeedback {
    pub message: String,
    pub severity: FeedbackSeverity,
}

impl KillFeedback {
    pub fn new(message: String, severity: FeedbackSeverity) -> Self {
        Self { message, severity }
    }

    pub fn info(message: String) -> Self {
        Self::new(message, FeedbackSeverity::Info)
    }

    pub fn warning(message: String) -> Self {
        Self::new(message, FeedbackSeverity::Warning)
    }

    pub fn error(message: String) -> Self {
        Self::new(message, FeedbackSeverity::Error)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum FeedbackSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Default)]
pub struct AppState {
    pub processes: Vec<ProcessInfo>,
    pub last_feedback: Option<KillFeedback>,
    pub config: crate::config::Config,
    pub project_cache: HashMap<i32, ProjectInfo>,
    pub docker_port_map: HashMap<u16, DockerContainerInfo>,
    pub brew_services_map: HashMap<String, String>, // service_name -> status
}

#[derive(Clone, Copy, Debug)]
pub enum KillOutcome {
    Success,
    AlreadyExited,
    PermissionDenied,
    TimedOut,
    Failed(Errno),
}

#[derive(Clone, Debug)]
pub struct ProjectInfo {
    pub name: String,
    #[allow(dead_code)]
    pub path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct DockerContainerInfo {
    pub name: String,
    #[allow(dead_code)]
    pub id: String,
}
