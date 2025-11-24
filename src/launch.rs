use anyhow::Result;
use log::{debug, warn};

/// Determines the macOS version to decide which launch-at-login implementation to use
fn get_macos_version() -> Result<(u32, u32)> {
    let output = std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()?;

    let version_string = String::from_utf8(output.stdout)?;
    let parts: Vec<&str> = version_string.trim().split('.').collect();

    if parts.len() >= 2 {
        let major = parts[0].parse::<u32>()?;
        let minor = parts[1].parse::<u32>()?;
        Ok((major, minor))
    } else {
        Err(anyhow::anyhow!("Unable to parse macOS version"))
    }
}

/// Checks if we should use SMAppService (macOS 13.0+) or LaunchAgent fallback
fn should_use_smappservice() -> bool {
    match get_macos_version() {
        Ok((major, _minor)) => {
            debug!("Detected macOS version: {}.x", major);
            major >= 13
        }
        Err(e) => {
            warn!(
                "Failed to detect macOS version: {}, falling back to LaunchAgent",
                e
            );
            false
        }
    }
}

// ============================================================================
// SMAppService Implementation (macOS 13.0+)
// ============================================================================

#[cfg(target_os = "macos")]
mod smapp {
    use anyhow::Result;
    use log::{debug, info, warn};
    use smappservice_rs::{AppService, ServiceStatus, ServiceType};

    pub fn enable() -> Result<()> {
        debug!("Enabling launch-at-login via SMAppService");
        let app_service = AppService::new(ServiceType::MainApp);

        match app_service.register() {
            Ok(()) => {
                info!("Successfully registered with SMAppService");

                // Check if requires approval
                let status = app_service.status();
                if status == ServiceStatus::RequiresApproval {
                    warn!("Launch-at-login requires user approval in System Settings");
                    // Open System Settings to Login Items
                    AppService::open_system_settings_login_items();
                    return Err(anyhow::anyhow!(
                        "Please approve macport in System Settings > Login Items"
                    ));
                }

                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!(
                "Failed to register with SMAppService: {}",
                e
            )),
        }
    }

    pub fn disable() -> Result<()> {
        debug!("Disabling launch-at-login via SMAppService");
        let app_service = AppService::new(ServiceType::MainApp);
        app_service
            .unregister()
            .map_err(|e| anyhow::anyhow!("Failed to unregister from SMAppService: {}", e))
    }

    pub fn is_enabled() -> Result<bool> {
        let app_service = AppService::new(ServiceType::MainApp);
        let status = app_service.status();
        // Both Enabled and RequiresApproval mean the app is registered for launch-at-login
        // RequiresApproval just needs user to approve it in System Settings
        Ok(status == ServiceStatus::Enabled || status == ServiceStatus::RequiresApproval)
    }
}

// ============================================================================
// LaunchAgent Implementation (Fallback for macOS < 13.0)
// ============================================================================

#[cfg(target_os = "macos")]
mod launchagent {
    use anyhow::Result;
    use auto_launch::AutoLaunchBuilder;
    use log::{debug, info};

    fn get_auto_launch() -> Result<auto_launch::AutoLaunch> {
        // Get the current executable path
        let exe_path = std::env::current_exe()?;
        let app_path = exe_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid executable path"))?;

        AutoLaunchBuilder::new()
            .set_app_name("macport")
            .set_app_path(app_path)
            .set_use_launch_agent(true) // Use LaunchAgent instead of AppleScript
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create auto-launch config: {}", e))
    }

    pub fn enable() -> Result<()> {
        debug!("Enabling launch-at-login via LaunchAgent");
        let auto = get_auto_launch()?;
        auto.enable()
            .map_err(|e| anyhow::anyhow!("Failed to enable LaunchAgent: {}", e))?;
        info!("Successfully enabled launch-at-login via LaunchAgent");
        Ok(())
    }

    pub fn disable() -> Result<()> {
        debug!("Disabling launch-at-login via LaunchAgent");
        let auto = get_auto_launch()?;
        auto.disable()
            .map_err(|e| anyhow::anyhow!("Failed to disable LaunchAgent: {}", e))
    }

    pub fn is_enabled() -> Result<bool> {
        let auto = get_auto_launch()?;
        auto.is_enabled()
            .map_err(|e| anyhow::anyhow!("Failed to check LaunchAgent status: {}", e))
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Enables launch-at-login using the appropriate method for the current macOS version
pub fn enable_launch_at_login() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        if should_use_smappservice() {
            smapp::enable()
        } else {
            launchagent::enable()
        }
    }

    #[cfg(not(target_os = "macos"))]
    Err(anyhow::anyhow!(
        "Launch-at-login is only supported on macOS"
    ))
}

/// Disables launch-at-login using the appropriate method for the current macOS version
pub fn disable_launch_at_login() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        if should_use_smappservice() {
            smapp::disable()
        } else {
            launchagent::disable()
        }
    }

    #[cfg(not(target_os = "macos"))]
    Err(anyhow::anyhow!(
        "Launch-at-login is only supported on macOS"
    ))
}

/// Checks if launch-at-login is currently enabled
pub fn is_launch_at_login_enabled() -> Result<bool> {
    #[cfg(target_os = "macos")]
    {
        if should_use_smappservice() {
            smapp::is_enabled()
        } else {
            launchagent::is_enabled()
        }
    }

    #[cfg(not(target_os = "macos"))]
    Ok(false)
}
