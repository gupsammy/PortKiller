use std::path::Path;
use std::sync::OnceLock;

/// Find an executable in common Homebrew locations, falling back to PATH.
/// Results are cached for efficiency.
pub fn find_command(name: &str) -> &'static str {
    // Use static caches for common commands
    match name {
        "docker" => {
            static DOCKER: OnceLock<&'static str> = OnceLock::new();
            DOCKER.get_or_init(|| find_in_paths(name, HOMEBREW_PATHS))
        }
        "brew" => {
            static BREW: OnceLock<&'static str> = OnceLock::new();
            BREW.get_or_init(|| find_in_paths(name, HOMEBREW_PATHS))
        }
        "terminal-notifier" => {
            static NOTIFIER: OnceLock<&'static str> = OnceLock::new();
            NOTIFIER.get_or_init(|| find_in_paths(name, HOMEBREW_PATHS))
        }
        _ => find_in_paths(name, HOMEBREW_PATHS),
    }
}

const HOMEBREW_PATHS: &[&str] = &[
    "/opt/homebrew/bin", // Apple Silicon
    "/usr/local/bin",    // Intel Mac
];

fn find_in_paths(name: &str, prefix_paths: &[&str]) -> &'static str {
    for prefix in prefix_paths {
        let full_path = format!("{}/{}", prefix, name);
        if Path::new(&full_path).exists() {
            // Leak the string to get a 'static lifetime (acceptable for small, cached strings)
            return Box::leak(full_path.into_boxed_str());
        }
    }
    // Fallback to PATH lookup
    Box::leak(name.to_string().into_boxed_str())
}
