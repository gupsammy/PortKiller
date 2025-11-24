use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    #[serde(default)]
    pub monitoring: MonitoringConfig,
    #[serde(default)]
    pub integrations: IntegrationsConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub notifications: NotificationsConfig,
    #[serde(default)]
    pub system: SystemConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MonitoringConfig {
    #[serde(default = "default_poll_interval_secs")]
    pub poll_interval_secs: u64,
    #[serde(default = "default_port_ranges")]
    pub port_ranges: Vec<(u16, u16)>,
    #[serde(default = "default_show_project_names")]
    pub show_project_names: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IntegrationsConfig {
    #[serde(default = "default_brew_enabled")]
    pub brew_enabled: bool,
    #[serde(default = "default_docker_enabled")]
    pub docker_enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UiConfig {
    #[serde(default = "default_inactive_color")]
    pub inactive_color: (u8, u8, u8),
    #[serde(default = "default_active_color")]
    pub active_color: (u8, u8, u8),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NotificationsConfig {
    #[serde(default = "default_notifications_enabled")]
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SystemConfig {
    #[serde(default = "default_launch_at_login")]
    pub launch_at_login: bool,
}

// Defaults for MonitoringConfig
fn default_poll_interval_secs() -> u64 {
    2
}

fn default_port_ranges() -> Vec<(u16, u16)> {
    vec![
        (3000, 3010),   // Node.js, React, Next.js, Vite
        (3306, 3306),   // MySQL
        (4000, 4010),   // Alternative Node servers
        (5001, 5010),   // Flask, general dev servers (excluding 5000)
        (5173, 5173),   // Vite default
        (5432, 5432),   // PostgreSQL
        (6379, 6380),   // Redis (6379 default, 6380 for testing)
        (8000, 8100),   // Django, Python HTTP servers
        (8080, 8090),   // Tomcat, alternative HTTP
        (9000, 9010),   // Various dev tools
        (27017, 27017), // MongoDB
    ]
}

fn default_show_project_names() -> bool {
    true
}

// Defaults for IntegrationsConfig
fn default_brew_enabled() -> bool {
    true
}

fn default_docker_enabled() -> bool {
    true
}

// Defaults for UiConfig
fn default_inactive_color() -> (u8, u8, u8) {
    (255, 255, 255)
}

fn default_active_color() -> (u8, u8, u8) {
    (255, 69, 58)
}

// Defaults for NotificationsConfig
fn default_notifications_enabled() -> bool {
    true
}

// Defaults for SystemConfig
fn default_launch_at_login() -> bool {
    false
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: default_poll_interval_secs(),
            port_ranges: default_port_ranges(),
            show_project_names: default_show_project_names(),
        }
    }
}

impl Default for IntegrationsConfig {
    fn default() -> Self {
        Self {
            brew_enabled: default_brew_enabled(),
            docker_enabled: default_docker_enabled(),
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            inactive_color: default_inactive_color(),
            active_color: default_active_color(),
        }
    }
}

impl Default for NotificationsConfig {
    fn default() -> Self {
        Self {
            enabled: default_notifications_enabled(),
        }
    }
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            launch_at_login: default_launch_at_login(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            monitoring: MonitoringConfig::default(),
            integrations: IntegrationsConfig::default(),
            ui: UiConfig::default(),
            notifications: NotificationsConfig::default(),
            system: SystemConfig::default(),
        }
    }
}

// Legacy config format for backward compatibility
#[derive(Deserialize)]
struct LegacyConfig {
    port_ranges: Vec<(u16, u16)>,
    #[serde(default = "default_inactive_color")]
    inactive_color: (u8, u8, u8),
    #[serde(default = "default_active_color")]
    active_color: (u8, u8, u8),
    #[serde(default = "default_notifications_enabled")]
    notifications_enabled: bool,
}

impl From<LegacyConfig> for Config {
    fn from(legacy: LegacyConfig) -> Self {
        Self {
            monitoring: MonitoringConfig {
                poll_interval_secs: default_poll_interval_secs(),
                port_ranges: legacy.port_ranges,
                show_project_names: default_show_project_names(),
            },
            integrations: IntegrationsConfig {
                brew_enabled: default_brew_enabled(),
                docker_enabled: default_docker_enabled(),
            },
            ui: UiConfig {
                inactive_color: legacy.inactive_color,
                active_color: legacy.active_color,
            },
            notifications: NotificationsConfig {
                enabled: legacy.notifications_enabled,
            },
            system: SystemConfig {
                launch_at_login: default_launch_at_login(),
            },
        }
    }
}

pub fn get_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".macport.json")
}

pub fn load_or_create_config() -> Result<Config> {
    let path = get_config_path();

    if path.exists() {
        let content = fs::read_to_string(&path).context("failed to read config file")?;

        // Try parsing as new format first
        if let Ok(config) = serde_json::from_str::<Config>(&content) {
            return Ok(config);
        }

        // Fall back to legacy format and migrate
        if let Ok(legacy) = serde_json::from_str::<LegacyConfig>(&content) {
            let config = Config::from(legacy);
            // Save migrated config in new format
            save_config(&config)?;
            return Ok(config);
        }

        // If both fail, return parse error
        Err(anyhow::anyhow!(
            "failed to parse config file in either new or legacy format"
        ))
    } else {
        let config = Config::default();
        save_config(&config)?;
        Ok(config)
    }
}

pub fn save_config(config: &Config) -> Result<()> {
    let path = get_config_path();
    let content = serde_json::to_string_pretty(config).context("failed to serialize config")?;
    fs::write(&path, content).context("failed to write config file")?;
    Ok(())
}
