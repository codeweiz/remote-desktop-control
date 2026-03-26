//! Auxiliary ACP sessions for background tasks.
//!
//! Used for title generation, follow-up suggestions, and summarization.
//! These reuse the same ACP process as the main session.
//!
//! NOTE: This is a stub -- full implementation deferred to a future iteration.
//! Auxiliary session failures are silently logged and never affect the main session.

/// The purpose of an auxiliary session.
#[derive(Debug)]
pub enum AuxPurpose {
    /// Generate a title for the conversation.
    TitleGen,
    /// Suggest follow-up prompts.
    FollowUp,
    /// Summarize the conversation so far.
    Summarize,
}

/// Placeholder for auxiliary session functionality.
/// Currently a no-op -- returns None for all operations.
pub async fn run_auxiliary(_purpose: AuxPurpose, _prompt: &str) -> Option<String> {
    tracing::debug!(purpose = ?_purpose, "auxiliary sessions not yet implemented");
    None
}
