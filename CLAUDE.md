# portkiller

macOS menu bar app for monitoring and killing processes on development ports. Built with Rust and native system tray integration.

## Tech Stack

Rust 2024 edition, Cargo. Key deps: tray-icon, winit, nix, crossbeam-channel, anyhow, smappservice-rs (launch-at-login), notify (file watching).

## Build Optimizations

Release profile in `Cargo.toml`: opt-level "z", LTO, single codegen-unit, strip symbols, panic=abort.
Icon optimization via `scripts/create-icon.sh`: max 512x512, pngquant + optipng compression.
Final sizes: Binary ~1.1 MB, Icon ~512 KB, DMG ~1.1 MB.

## Development Commands

```bash
# Run with debug logging
RUST_LOG=debug cargo run

# Build release binary
cargo build --release

# Code quality
cargo fmt              # Format code
cargo clippy           # Lint
cargo check            # Quick compile check

# Install git hooks (auto-format on commit)
./scripts/install-hooks.sh
```

## Scripts

`scripts/` contains build and test utilities:
- `build-app.sh`, `create-dmg.sh`, `release.sh` — Build and packaging
- `create-icon.sh`, `create-menubar-icon.sh` — Icon generation
- `install-hooks.sh` — Git pre-commit hook for auto-formatting
- `start-test-ports.sh`, `test-notifications.sh` — Development testing

## Configuration

User config at `~/.portkiller.json` (auto-created). Edit via menu "Edit Configuration".
Sections: `monitoring` (poll_interval_secs, port_ranges, show_project_names), `integrations` (brew_enabled, docker_enabled), `notifications` (enabled), `system` (launch_at_login).

## Architecture

### Module Structure
```
src/
├── main.rs, lib.rs      # Entry point and module exports
├── app.rs               # Event loop orchestration, thread management
├── config.rs            # Configuration (~/.portkiller.json)
├── model.rs             # AppState, ProcessInfo data structures
├── notify.rs            # macOS notifications
├── launch.rs            # Launch-at-login (SMAppService on macOS 13+, LaunchAgent fallback)
├── process/
│   ├── ports.rs         # Port scanning via lsof
│   └── kill.rs          # SIGTERM → 2s → SIGKILL → 1s termination
├── ui/
│   ├── icon.rs          # Template tray icon
│   └── menu.rs          # Dynamic menu construction
└── integrations/
    ├── docker.rs        # Container name → port mapping
    └── brew.rs          # Homebrew service detection
```

### Threading Model
Five threads via channels: Main (winit event loop), Monitor (2s port polling), Menu Listener (click → action), Kill Worker (process termination), Config Watcher (hot-reload via FSEvents).

## Default Port Ranges

Defined in `config.rs`: 3000-3010 (Node/React), 3306 (MySQL), 4000-4010 (alt Node), 5001-5010 (Flask, 5000 excluded for AirPlay), 5173 (Vite), 5432 (Postgres), 6379 (Redis), 8000-8100 (Django/Python), 8080-8090 (Tomcat), 9000-9010 (dev tools), 27017 (MongoDB).

## Key Constants

Poll every 2s, SIGTERM grace 2s, SIGKILL grace 1s, poll step 200ms, max 5 tooltip entries.

## Common Patterns

Adding ports: Edit `~/.portkiller.json` (changes apply automatically via hot-reload, or use "Reload Configuration" menu). Defaults in `config.rs`.
New integrations: Add to `src/integrations/`, implement detection + port mapping + menu integration.
Debugging: `RUST_LOG=debug cargo run` for lsof parsing, Docker/Brew discovery, kill sequence logs.

## Development Notes

- Menu IDs use prefixes: `process_`, `docker_stop_`, `brew_stop_` for action routing
- All external command inputs sanitized to prevent injection
- Icon updates on state changes (not timer-based)
- Project cache in `model.rs` prevents repeated git lookups
- Brew detection verifies actual port binding, not just service status

## Coding Principles

- Module boundaries: Keep domain logic isolated (process/, ui/, integrations/)
- Error handling: `anyhow::Result` for fallible operations
- Logging: `log::debug!` for diagnostics, `log::error!` for failures
- Thread safety: Communicate via channels, minimize shared state
- Security: Sanitize all inputs to `lsof`, `docker`, `brew`, `osascript`
