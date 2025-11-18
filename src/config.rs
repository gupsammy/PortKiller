use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    pub port_ranges: Vec<(u16, u16)>,
    #[serde(default = "default_inactive_color")]
    pub inactive_color: (u8, u8, u8),
    #[serde(default = "default_active_color")]
    pub active_color: (u8, u8, u8),
    #[serde(default = "default_notifications_enabled")]
    pub notifications_enabled: bool,
}

fn default_inactive_color() -> (u8, u8, u8) {
    (255, 255, 255)
}

fn default_active_color() -> (u8, u8, u8) {
    (255, 69, 58)
}

fn default_notifications_enabled() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port_ranges: vec![
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
            ],
            inactive_color: default_inactive_color(),
            active_color: default_active_color(),
            notifications_enabled: default_notifications_enabled(),
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
        Ok(serde_json::from_str(&content).context("failed to parse config file")?)
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
