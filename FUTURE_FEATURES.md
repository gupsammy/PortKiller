# Future Feature Ideas for PortKiller

Ideas from website copy that aren't implemented yet:

## Keyboard Shortcuts (Not Implemented)
**Website claim:** "⌘+3 kills 3000. ⌘+8 obliterates 8080."
**Status:** FALSE - No global keyboard shortcuts exist in the app
**Implementation notes:** Would need to register global hotkeys, might require accessibility permissions

## Docker Advanced Operations (Partially Implemented)
**Website claim:** "Remove them. Kill their networks."
**Status:** PARTIAL - Can stop containers, but NOT remove them or kill networks
**Current:** `docker stop <container>` only
**Needed:** `docker rm <container>`, `docker network prune`, etc.

## Additional Ideas Worth Exploring

### 1. Quick Port Kill from Notification
- Click notification to instantly kill the process that just started
- Deep linking from notification center

### 2. Port History / Analytics
- Track how often ports are used
- Show "busiest" ports over time
- Help identify which processes keep respawning

### 3. Process Groups
- Kill all Node.js processes at once
- Bulk operations for similar process types

### 4. Whitelist / "Don't Kill" List
- Protect certain processes from accidental termination
- Useful for long-running databases

### 5. Custom Kill Scripts
- Run a shell script before/after killing a process
- e.g., clean up temp files, save state, etc.

### 6. Port Forwarding Management
- See SSH tunnels and port forwards
- Kill orphaned tunnels

### 7. Windows/Linux Support
- Cross-platform port management
- Different implementation for netstat/taskkill on Windows

### 8. Colima Integration
- Better Docker Desktop alternative detection
- Explicit Colima container management

### 9. Network Connection Viewer
- See established connections (not just LISTEN)
- Identify outbound connections per process

### 10. Export Port Activity
- Log port usage to file
- Export for debugging or auditing
