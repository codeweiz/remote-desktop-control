//! Agent error classification.
//!
//! Classifies agent errors as permanent or transient based on pattern matching
//! against stderr output and error messages. Provides user-friendly guidance.

use crate::events::ErrorClass;

/// Classify an agent error and provide user guidance.
///
/// Returns (severity, guidance_message).
pub fn classify_error(stderr: &str, error_msg: &str) -> (ErrorClass, String) {
    let combined = format!("{}\n{}", stderr, error_msg).to_lowercase();

    if combined.contains("module_not_found")
        || combined.contains("eacces")
        || combined.contains("permission denied")
    {
        (ErrorClass::Permanent, "Agent binary not available or permission denied. Check installation.".into())
    } else if combined.contains("enoent") || combined.contains("not found") || combined.contains("no such file") {
        (ErrorClass::Permanent, "Agent command not found. Ensure it is installed and in PATH.".into())
    } else if combined.contains("syntax") || combined.contains("invalid option") || combined.contains("unrecognized") {
        (ErrorClass::Permanent, "Configuration error. Check agent settings and command-line flags.".into())
    } else if combined.contains("timeout") || combined.contains("econnrefused") || combined.contains("timed out") {
        (ErrorClass::Transient, "Network timeout. Will retry automatically.".into())
    } else if combined.contains("rate limit") || combined.contains("429") || combined.contains("too many requests") {
        (ErrorClass::Transient, "Rate limited. Will retry after backoff.".into())
    } else if combined.contains("killed") || combined.contains("signal") {
        (ErrorClass::Transient, "Process was killed. Will attempt restart.".into())
    } else {
        (ErrorClass::Transient, "Unknown error. Will attempt restart.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permanent_not_found() {
        let (class, _) = classify_error("", "ENOENT: claude not found");
        assert!(matches!(class, ErrorClass::Permanent));
    }

    #[test]
    fn test_permanent_permission() {
        let (class, _) = classify_error("Permission denied", "");
        assert!(matches!(class, ErrorClass::Permanent));
    }

    #[test]
    fn test_transient_timeout() {
        let (class, _) = classify_error("", "connection timed out");
        assert!(matches!(class, ErrorClass::Transient));
    }

    #[test]
    fn test_transient_rate_limit() {
        let (class, _) = classify_error("429 Too Many Requests", "");
        assert!(matches!(class, ErrorClass::Transient));
    }

    #[test]
    fn test_unknown_is_transient() {
        let (class, guidance) = classify_error("", "something weird happened");
        assert!(matches!(class, ErrorClass::Transient));
        assert!(guidance.contains("restart"));
    }
}
