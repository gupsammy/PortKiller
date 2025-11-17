# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**macport** is a macOS system tray application that monitors common development server ports and allows killing processes listening on those ports. The application uses Rust with a native system tray interface.

Monitored ports include:
- 3000-3010: Node.js, React, Next.js, Vite
- 3306: MySQL
- 4000-4010: Alternative Node servers
- 5000-5010: Flask, general dev servers
- 5173: Vite default
- 5432: PostgreSQL
- 6379: Redis
- 8000-8100: Django, Python HTTP servers
- 8080-8090: Tomcat, alternative HTTP
- 9000-9010: Various dev tools
- 27017: MongoDB

## Build & Development Commands

```bash
# Build the project
cargo build

# Build optimized release version
cargo build --release

# Run the application
cargo run

# Run with logging enabled
RUST_LOG=debug cargo run

# Check code without building
cargo check

# Format code
cargo fmt

# Run linter
cargo clippy

# Install git hooks (recommended for contributors)
./scripts/install-hooks.sh
```

## Git Hooks

The project includes a pre-commit hook that automatically checks code formatting before each commit. To install:

```bash
./scripts/install-hooks.sh
```

The hook will:
- Check if code is properly formatted with `cargo fmt`
- Automatically run `cargo fmt` if formatting issues are found
- Abort the commit and prompt you to review and re-commit the formatted code

This ensures all commits pass CI formatting checks and maintains consistent code style.

## Architecture

### Single-File Design
The entire application is contained in `src/main.rs` (~730 lines). This is a deliberate architectural choice for a simple system utility.

### Threading Model
The application uses a multi-threaded event-driven architecture with four concurrent threads:

1. **Main Event Loop** (winit): Manages the UI event loop, tray icon updates, and orchestrates state changes
2. **Monitor Thread** (`spawn_monitor_thread`): Polls ports 3000-3020 every 2 seconds using `lsof`, detects changes, and sends `ProcessesUpdated` events
3. **Menu Listener Thread** (`spawn_menu_listener`): Listens for menu click events and converts them to `MenuAction` events
4. **Kill Worker Thread** (`spawn_kill_worker`): Receives kill commands via channel, executes graceful termination (SIGTERM → SIGKILL), and reports feedback

Communication between threads uses:
- `crossbeam_channel` for kill commands
- `EventLoopProxy<UserEvent>` for sending events to the main loop

### Process Termination Flow
The `terminate_pid` function implements a graceful shutdown strategy:
1. Check if process exists (`kill(pid, None)`)
2. Send SIGTERM and wait 2 seconds
3. If still alive, send SIGKILL and wait 1 second
4. Poll every 200ms during grace periods
5. Return detailed outcome: Success, AlreadyExited, PermissionDenied, TimedOut, or Failed

### State Management
`AppState` maintains:
- Current list of `ProcessInfo` (port, pid, command)
- Last feedback message for user visibility in tooltip

The tray icon displays:
- **Title**: Process count with ⚠️ emoji if active listeners exist
- **Tooltip**: Up to 5 active listeners + last action feedback with severity emoji
- **Menu**: Individual kill options, "Kill all", and "Quit"

### Key Dependencies
- **tray-icon**: Cross-platform system tray (macOS status bar)
- **winit**: Event loop foundation
- **nix**: Unix signal handling (SIGTERM, SIGKILL)
- **crossbeam-channel**: Thread-safe communication
- **anyhow**: Error handling

### Platform-Specific Notes
- Uses `lsof` command to detect port listeners (macOS/Unix)
- Uses `ps` command to resolve process names
- Icon is template-based (automatically adapts to light/dark mode on macOS)
- Rust edition is set to "2024" in Cargo.toml

## Key Constants
```rust
PORT_RANGES = [(3000, 3010), (3306, 3306), ...] // Port ranges to monitor
POLL_INTERVAL = 2s                              // How often to check ports
SIGTERM_GRACE = 2s, SIGKILL_GRACE = 1s          // Termination timeouts
MAX_TOOLTIP_ENTRIES = 5                         // Tooltip display limit
```
