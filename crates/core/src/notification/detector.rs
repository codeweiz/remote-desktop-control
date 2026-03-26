//! Three-layer signal fusion detector.
//!
//! Layer 1 (Process Monitor): Tracks foreground process group via tcgetpgrp.
//!   Detects ShellBecameForeground (command complete), ProcessExited.
//!
//! Layer 2 (Timing): Output rate tracking. Detects OutputStalled after
//!   configurable silence threshold.
//!
//! Layer 3 (Semantic): Pattern matching for known prompts and tool completions.
//!
//! Signal Fusion: Weighted scores from each layer, threshold 0.7 to trigger.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use tracing::debug;

use super::{
    DetectionLayer, DetectionSignal, NotifyTrigger, PromptType, FUSION_THRESHOLD,
};

// ---------------------------------------------------------------------------
// Layer 1: Process Monitor
// ---------------------------------------------------------------------------

/// Process monitor state.
pub struct ProcessMonitor {
    /// The PTY file descriptor to monitor (for tcgetpgrp).
    #[allow(dead_code)]
    pty_fd: Option<i32>,
    /// Whether we believe the shell is currently in the foreground.
    shell_is_foreground: bool,
    /// Last known foreground process group ID.
    #[allow(dead_code)]
    last_fg_pgid: Option<i32>,
}

impl ProcessMonitor {
    /// Create a new process monitor.
    pub fn new(pty_fd: Option<i32>) -> Self {
        Self {
            pty_fd,
            shell_is_foreground: true,
            last_fg_pgid: None,
        }
    }

    /// Check the foreground process group.
    ///
    /// Returns a signal if the foreground changed (e.g., command completed).
    pub fn check(&mut self) -> Option<DetectionSignal> {
        #[cfg(unix)]
        {
            if let Some(fd) = self.pty_fd {
                // Safety: tcgetpgrp is safe to call on a valid fd.
                let pgid = unsafe { libc::tcgetpgrp(fd) };
                if pgid < 0 {
                    return None;
                }

                let shell_pgid = unsafe { libc::getpgrp() };

                let was_foreground = self.shell_is_foreground;
                self.shell_is_foreground = pgid == shell_pgid;
                self.last_fg_pgid = Some(pgid);

                // If the shell just became foreground, a command completed.
                if !was_foreground && self.shell_is_foreground {
                    return Some(DetectionSignal {
                        layer: DetectionLayer::ProcessMonitor,
                        confidence: 0.8,
                        trigger: NotifyTrigger::ProcessExited {
                            exit_code: 0,
                            command: None,
                            duration_secs: 0.0,
                        },
                        weight: 0.4,
                    });
                }
            }
        }

        // On non-Unix platforms or when no PTY fd is available,
        // this layer produces no signals.
        None
    }

    /// Notify the monitor that a process exited with a specific code.
    pub fn process_exited(&mut self, exit_code: i32, command: Option<String>, duration_secs: f64) -> DetectionSignal {
        self.shell_is_foreground = true;
        DetectionSignal {
            layer: DetectionLayer::ProcessMonitor,
            confidence: 1.0,
            trigger: NotifyTrigger::ProcessExited {
                exit_code,
                command,
                duration_secs,
            },
            weight: 0.4,
        }
    }
}

// ---------------------------------------------------------------------------
// Layer 2: Timing Analysis
// ---------------------------------------------------------------------------

/// Output rate tracker for detecting stalled output.
pub struct TimingDetector {
    /// Timestamps of recent output chunks.
    output_timestamps: VecDeque<Instant>,
    /// Maximum entries to keep for rate calculation.
    max_entries: usize,
    /// Last time we saw any output.
    last_output: Option<Instant>,
    /// Silence threshold to consider output "stalled".
    silence_threshold: Duration,
    /// Whether we've already fired a "stalled" signal for the current silence.
    stalled_fired: bool,
    /// When the current command (approximately) started.
    command_start: Option<Instant>,
    /// Long-running threshold.
    long_running_threshold: Duration,
}

impl TimingDetector {
    /// Create a new timing detector.
    pub fn new(silence_threshold_secs: u64, long_running_threshold_secs: u64) -> Self {
        Self {
            output_timestamps: VecDeque::with_capacity(1000),
            max_entries: 1000,
            last_output: None,
            silence_threshold: Duration::from_secs(silence_threshold_secs),
            stalled_fired: false,
            command_start: None,
            long_running_threshold: Duration::from_secs(long_running_threshold_secs),
        }
    }

    /// Record that output was received.
    pub fn record_output(&mut self) {
        let now = Instant::now();
        self.last_output = Some(now);
        self.stalled_fired = false;

        if self.output_timestamps.len() >= self.max_entries {
            self.output_timestamps.pop_front();
        }
        self.output_timestamps.push_back(now);
    }

    /// Mark the start of a new command.
    pub fn command_started(&mut self) {
        self.command_start = Some(Instant::now());
        self.stalled_fired = false;
        self.output_timestamps.clear();
    }

    /// Check for timing-based signals.
    pub fn check(&mut self) -> Vec<DetectionSignal> {
        let mut signals = Vec::new();
        let now = Instant::now();

        // Check for output stall
        if let Some(last) = self.last_output {
            let silence = now.duration_since(last);
            if silence >= self.silence_threshold && !self.stalled_fired {
                self.stalled_fired = true;

                // Confidence based on how long the silence has been
                let ratio = silence.as_secs_f64() / self.silence_threshold.as_secs_f64();
                let confidence = (ratio.min(3.0) / 3.0).min(1.0);

                debug!(
                    silence_secs = silence.as_secs_f64(),
                    confidence = confidence,
                    "output stalled detected"
                );

                signals.push(DetectionSignal {
                    layer: DetectionLayer::Timing,
                    confidence,
                    trigger: NotifyTrigger::WaitingForInput {
                        prompt_type: PromptType::Input,
                        prompt_text: None,
                    },
                    weight: 0.3,
                });
            }
        }

        // Check for long-running command
        if let Some(start) = self.command_start {
            let elapsed = now.duration_since(start);
            if elapsed >= self.long_running_threshold {
                let output_rate = self.calculate_output_rate(Duration::from_secs(10));
                // If output rate has dropped significantly, the command might be done
                if output_rate < 0.1 && !self.stalled_fired {
                    signals.push(DetectionSignal {
                        layer: DetectionLayer::Timing,
                        confidence: 0.6,
                        trigger: NotifyTrigger::LongRunningDone {
                            command: None,
                            duration_secs: elapsed.as_secs_f64(),
                            success: true,
                        },
                        weight: 0.3,
                    });
                }
            }
        }

        signals
    }

    /// Calculate the output rate (events per second) over a recent window.
    fn calculate_output_rate(&self, window: Duration) -> f64 {
        let now = Instant::now();
        let cutoff = now - window;

        let count = self
            .output_timestamps
            .iter()
            .rev()
            .take_while(|ts| **ts >= cutoff)
            .count();

        count as f64 / window.as_secs_f64()
    }

    /// Get the time since last output, if any.
    pub fn silence_duration(&self) -> Option<Duration> {
        self.last_output.map(|last| Instant::now().duration_since(last))
    }
}

// ---------------------------------------------------------------------------
// Layer 3: Semantic Pattern Matching
// ---------------------------------------------------------------------------

/// Semantic patterns for detecting prompts and completions in terminal output.
struct SemanticPattern {
    /// The pattern to match (case-insensitive substring or suffix).
    pattern: &'static str,
    /// The trigger to emit when matched.
    trigger_type: SemanticTriggerType,
    /// Base confidence for this pattern.
    confidence: f64,
    /// Whether this is a suffix match (matches end of line).
    suffix_match: bool,
}

#[derive(Debug, Clone)]
enum SemanticTriggerType {
    Confirmation,
    Password,
    Input,
    Selection,
    ToolCompletion { success: bool },
    ErrorIndicator,
}

/// Semantic detector using pattern matching.
pub struct SemanticDetector {
    patterns: Vec<SemanticPattern>,
}

impl SemanticDetector {
    /// Create a new semantic detector with built-in patterns.
    pub fn new() -> Self {
        let patterns = vec![
            // Confirmation prompts
            SemanticPattern {
                pattern: "[y/n]",
                trigger_type: SemanticTriggerType::Confirmation,
                confidence: 0.95,
                suffix_match: false,
            },
            SemanticPattern {
                pattern: "[yes/no]",
                trigger_type: SemanticTriggerType::Confirmation,
                confidence: 0.95,
                suffix_match: false,
            },
            SemanticPattern {
                pattern: "(y/n)",
                trigger_type: SemanticTriggerType::Confirmation,
                confidence: 0.9,
                suffix_match: false,
            },
            SemanticPattern {
                pattern: "are you sure",
                trigger_type: SemanticTriggerType::Confirmation,
                confidence: 0.7,
                suffix_match: false,
            },
            SemanticPattern {
                pattern: "do you want to continue",
                trigger_type: SemanticTriggerType::Confirmation,
                confidence: 0.8,
                suffix_match: false,
            },
            SemanticPattern {
                pattern: "proceed?",
                trigger_type: SemanticTriggerType::Confirmation,
                confidence: 0.75,
                suffix_match: true,
            },
            // Password prompts
            SemanticPattern {
                pattern: "password:",
                trigger_type: SemanticTriggerType::Password,
                confidence: 0.95,
                suffix_match: true,
            },
            SemanticPattern {
                pattern: "passphrase:",
                trigger_type: SemanticTriggerType::Password,
                confidence: 0.95,
                suffix_match: true,
            },
            SemanticPattern {
                pattern: "enter password",
                trigger_type: SemanticTriggerType::Password,
                confidence: 0.9,
                suffix_match: false,
            },
            SemanticPattern {
                pattern: "sudo password",
                trigger_type: SemanticTriggerType::Password,
                confidence: 0.95,
                suffix_match: false,
            },
            // Selection prompts
            SemanticPattern {
                pattern: "select:",
                trigger_type: SemanticTriggerType::Selection,
                confidence: 0.7,
                suffix_match: true,
            },
            SemanticPattern {
                pattern: "choose:",
                trigger_type: SemanticTriggerType::Selection,
                confidence: 0.7,
                suffix_match: true,
            },
            SemanticPattern {
                pattern: "pick a number",
                trigger_type: SemanticTriggerType::Selection,
                confidence: 0.75,
                suffix_match: false,
            },
            // Generic input prompts
            SemanticPattern {
                pattern: "> ",
                trigger_type: SemanticTriggerType::Input,
                confidence: 0.3,
                suffix_match: true,
            },
            SemanticPattern {
                pattern: "press enter",
                trigger_type: SemanticTriggerType::Input,
                confidence: 0.85,
                suffix_match: false,
            },
            SemanticPattern {
                pattern: "press any key",
                trigger_type: SemanticTriggerType::Input,
                confidence: 0.85,
                suffix_match: false,
            },
            // Tool/build completions (success)
            SemanticPattern {
                pattern: "finished",
                trigger_type: SemanticTriggerType::ToolCompletion { success: true },
                confidence: 0.6,
                suffix_match: false,
            },
            SemanticPattern {
                pattern: "cargo finished",
                trigger_type: SemanticTriggerType::ToolCompletion { success: true },
                confidence: 0.9,
                suffix_match: false,
            },
            SemanticPattern {
                pattern: "compiled successfully",
                trigger_type: SemanticTriggerType::ToolCompletion { success: true },
                confidence: 0.9,
                suffix_match: false,
            },
            SemanticPattern {
                pattern: "npm added",
                trigger_type: SemanticTriggerType::ToolCompletion { success: true },
                confidence: 0.85,
                suffix_match: false,
            },
            SemanticPattern {
                pattern: "added .* packages",
                trigger_type: SemanticTriggerType::ToolCompletion { success: true },
                confidence: 0.8,
                suffix_match: false,
            },
            SemanticPattern {
                pattern: "build successful",
                trigger_type: SemanticTriggerType::ToolCompletion { success: true },
                confidence: 0.9,
                suffix_match: false,
            },
            SemanticPattern {
                pattern: "tests passed",
                trigger_type: SemanticTriggerType::ToolCompletion { success: true },
                confidence: 0.85,
                suffix_match: false,
            },
            SemanticPattern {
                pattern: "test result: ok",
                trigger_type: SemanticTriggerType::ToolCompletion { success: true },
                confidence: 0.95,
                suffix_match: false,
            },
            // Error indicators
            SemanticPattern {
                pattern: "error:",
                trigger_type: SemanticTriggerType::ErrorIndicator,
                confidence: 0.7,
                suffix_match: false,
            },
            SemanticPattern {
                pattern: "error[e",
                trigger_type: SemanticTriggerType::ErrorIndicator,
                confidence: 0.9,
                suffix_match: false,
            },
            SemanticPattern {
                pattern: "fatal:",
                trigger_type: SemanticTriggerType::ErrorIndicator,
                confidence: 0.85,
                suffix_match: false,
            },
            SemanticPattern {
                pattern: "panic:",
                trigger_type: SemanticTriggerType::ErrorIndicator,
                confidence: 0.9,
                suffix_match: false,
            },
            SemanticPattern {
                pattern: "segmentation fault",
                trigger_type: SemanticTriggerType::ErrorIndicator,
                confidence: 0.95,
                suffix_match: false,
            },
            SemanticPattern {
                pattern: "command not found",
                trigger_type: SemanticTriggerType::ErrorIndicator,
                confidence: 0.85,
                suffix_match: false,
            },
            SemanticPattern {
                pattern: "permission denied",
                trigger_type: SemanticTriggerType::ErrorIndicator,
                confidence: 0.8,
                suffix_match: false,
            },
            SemanticPattern {
                pattern: "test result: failed",
                trigger_type: SemanticTriggerType::ToolCompletion { success: false },
                confidence: 0.95,
                suffix_match: false,
            },
        ];

        Self { patterns }
    }

    /// Analyze a line of output for semantic patterns.
    pub fn analyze(&self, line: &str) -> Vec<DetectionSignal> {
        let lower = line.to_lowercase();
        let trimmed = lower.trim();
        let mut signals = Vec::new();

        for pattern in &self.patterns {
            let matched = if pattern.suffix_match {
                trimmed.ends_with(pattern.pattern)
            } else {
                trimmed.contains(pattern.pattern)
            };

            if matched {
                let trigger = match &pattern.trigger_type {
                    SemanticTriggerType::Confirmation => NotifyTrigger::WaitingForInput {
                        prompt_type: PromptType::Confirmation,
                        prompt_text: Some(line.to_string()),
                    },
                    SemanticTriggerType::Password => NotifyTrigger::WaitingForInput {
                        prompt_type: PromptType::Password,
                        prompt_text: Some(line.to_string()),
                    },
                    SemanticTriggerType::Input => NotifyTrigger::WaitingForInput {
                        prompt_type: PromptType::Input,
                        prompt_text: Some(line.to_string()),
                    },
                    SemanticTriggerType::Selection => NotifyTrigger::WaitingForInput {
                        prompt_type: PromptType::Selection,
                        prompt_text: Some(line.to_string()),
                    },
                    SemanticTriggerType::ToolCompletion { success } => {
                        NotifyTrigger::LongRunningDone {
                            command: None,
                            duration_secs: 0.0,
                            success: *success,
                        }
                    }
                    SemanticTriggerType::ErrorIndicator => NotifyTrigger::ErrorDetected {
                        error_text: Some(line.to_string()),
                    },
                };

                signals.push(DetectionSignal {
                    layer: DetectionLayer::Semantic,
                    confidence: pattern.confidence,
                    trigger,
                    weight: 0.3,
                });
            }
        }

        signals
    }
}

impl Default for SemanticDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Signal Fusion
// ---------------------------------------------------------------------------

/// Fuse detection signals from multiple layers and determine if the
/// combined confidence exceeds the threshold for triggering a notification.
pub fn fuse_signals(signals: &[DetectionSignal]) -> Vec<NotifyTrigger> {
    if signals.is_empty() {
        return Vec::new();
    }

    // Group signals by trigger category
    let mut prompt_score = 0.0;
    let mut exit_score = 0.0;
    let mut long_running_score = 0.0;
    let mut error_score = 0.0;
    let mut total_weight = 0.0;

    let mut best_prompt: Option<&DetectionSignal> = None;
    let mut best_exit: Option<&DetectionSignal> = None;
    let mut best_long_running: Option<&DetectionSignal> = None;
    let mut best_error: Option<&DetectionSignal> = None;

    for signal in signals {
        let weighted = signal.confidence * signal.weight;
        total_weight += signal.weight;

        match &signal.trigger {
            NotifyTrigger::WaitingForInput { .. } => {
                prompt_score += weighted;
                if best_prompt.is_none_or(|b| signal.confidence > b.confidence) {
                    best_prompt = Some(signal);
                }
            }
            NotifyTrigger::ProcessExited { .. } => {
                exit_score += weighted;
                if best_exit.is_none_or(|b| signal.confidence > b.confidence) {
                    best_exit = Some(signal);
                }
            }
            NotifyTrigger::LongRunningDone { .. } => {
                long_running_score += weighted;
                if best_long_running.is_none_or(|b| signal.confidence > b.confidence) {
                    best_long_running = Some(signal);
                }
            }
            NotifyTrigger::ErrorDetected { .. } => {
                error_score += weighted;
                if best_error.is_none_or(|b| signal.confidence > b.confidence) {
                    best_error = Some(signal);
                }
            }
            _ => {}
        }
    }

    let mut triggers = Vec::new();

    // Normalize scores
    if total_weight > 0.0 {
        let norm = 1.0 / total_weight.min(1.0);
        if prompt_score * norm >= FUSION_THRESHOLD {
            if let Some(signal) = best_prompt {
                triggers.push(signal.trigger.clone());
            }
        }
        if exit_score * norm >= FUSION_THRESHOLD {
            if let Some(signal) = best_exit {
                triggers.push(signal.trigger.clone());
            }
        }
        if long_running_score * norm >= FUSION_THRESHOLD {
            if let Some(signal) = best_long_running {
                triggers.push(signal.trigger.clone());
            }
        }
        if error_score * norm >= FUSION_THRESHOLD {
            if let Some(signal) = best_error {
                triggers.push(signal.trigger.clone());
            }
        }
    }

    triggers
}

// ---------------------------------------------------------------------------
// Composite Detector
// ---------------------------------------------------------------------------

/// Composite detector that runs all three layers and performs signal fusion.
pub struct Detector {
    pub process_monitor: ProcessMonitor,
    pub timing: TimingDetector,
    pub semantic: SemanticDetector,
}

impl Detector {
    /// Create a new composite detector.
    pub fn new(
        pty_fd: Option<i32>,
        silence_threshold_secs: u64,
        long_running_threshold_secs: u64,
    ) -> Self {
        Self {
            process_monitor: ProcessMonitor::new(pty_fd),
            timing: TimingDetector::new(silence_threshold_secs, long_running_threshold_secs),
            semantic: SemanticDetector::new(),
        }
    }

    /// Process new output and return any triggered notifications.
    pub fn process_output(&mut self, output: &str) -> Vec<NotifyTrigger> {
        let mut all_signals = Vec::new();

        // Record output in timing detector
        self.timing.record_output();

        // Layer 1: Check process state
        if let Some(signal) = self.process_monitor.check() {
            all_signals.push(signal);
        }

        // Layer 2: Check timing
        all_signals.extend(self.timing.check());

        // Layer 3: Analyze each line semantically
        for line in output.lines() {
            let line = line.trim();
            if !line.is_empty() {
                all_signals.extend(self.semantic.analyze(line));
            }
        }

        // Fuse signals
        fuse_signals(&all_signals)
    }

    /// Periodic check (no new output). Useful for detecting stalls.
    pub fn periodic_check(&mut self) -> Vec<NotifyTrigger> {
        let mut all_signals = Vec::new();

        if let Some(signal) = self.process_monitor.check() {
            all_signals.push(signal);
        }

        all_signals.extend(self.timing.check());

        fuse_signals(&all_signals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_confirmation_prompt() {
        let detector = SemanticDetector::new();
        let signals = detector.analyze("Do you want to continue? [Y/n]");
        assert!(!signals.is_empty());

        let has_confirmation = signals.iter().any(|s| {
            matches!(
                &s.trigger,
                NotifyTrigger::WaitingForInput {
                    prompt_type: PromptType::Confirmation,
                    ..
                }
            )
        });
        assert!(has_confirmation);
    }

    #[test]
    fn test_semantic_password_prompt() {
        let detector = SemanticDetector::new();
        let signals = detector.analyze("Enter your password:");
        assert!(!signals.is_empty());

        let has_password = signals.iter().any(|s| {
            matches!(
                &s.trigger,
                NotifyTrigger::WaitingForInput {
                    prompt_type: PromptType::Password,
                    ..
                }
            )
        });
        assert!(has_password);
    }

    #[test]
    fn test_semantic_cargo_finished() {
        let detector = SemanticDetector::new();
        let signals = detector.analyze("   Cargo Finished `dev` profile [unoptimized + debuginfo]");
        assert!(!signals.is_empty());

        let has_completion = signals.iter().any(|s| {
            matches!(
                &s.trigger,
                NotifyTrigger::LongRunningDone { success: true, .. }
            )
        });
        assert!(has_completion);
    }

    #[test]
    fn test_semantic_error_detection() {
        let detector = SemanticDetector::new();
        let signals = detector.analyze("error[E0308]: mismatched types");
        assert!(!signals.is_empty());

        let has_error = signals
            .iter()
            .any(|s| matches!(&s.trigger, NotifyTrigger::ErrorDetected { .. }));
        assert!(has_error);
    }

    #[test]
    fn test_semantic_no_match() {
        let detector = SemanticDetector::new();
        let signals = detector.analyze("Hello world, this is normal output.");
        // Should not match any high-confidence patterns
        let high_confidence: Vec<_> = signals.iter().filter(|s| s.confidence > 0.5).collect();
        assert!(high_confidence.is_empty());
    }

    #[test]
    fn test_timing_stall_detection() {
        let mut timing = TimingDetector::new(1, 30); // 1 second silence threshold

        timing.record_output();

        // Immediately after output, should not detect stall
        let signals = timing.check();
        assert!(signals.is_empty());
    }

    #[test]
    fn test_fusion_threshold() {
        // High confidence prompt signal should trigger
        let signals = vec![DetectionSignal {
            layer: DetectionLayer::Semantic,
            confidence: 0.95,
            trigger: NotifyTrigger::WaitingForInput {
                prompt_type: PromptType::Confirmation,
                prompt_text: Some("[Y/n]".to_string()),
            },
            weight: 0.8,
        }];

        let triggers = fuse_signals(&signals);
        assert!(!triggers.is_empty());
    }

    #[test]
    fn test_fusion_below_threshold() {
        // Low confidence signal should not trigger
        let signals = vec![DetectionSignal {
            layer: DetectionLayer::Semantic,
            confidence: 0.3,
            trigger: NotifyTrigger::WaitingForInput {
                prompt_type: PromptType::Input,
                prompt_text: Some("> ".to_string()),
            },
            weight: 0.3,
        }];

        let triggers = fuse_signals(&signals);
        // 0.3 * 0.3 / 0.3 = 0.3 which is below 0.7
        assert!(triggers.is_empty());
    }
}
