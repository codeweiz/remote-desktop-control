//! IP blocklist with progressive ban durations.
//!
//! Tracks per-IP authentication failure counts and automatically bans IPs
//! after 10 consecutive failures. Ban durations escalate:
//!
//! | Ban # | Duration |
//! |-------|----------|
//! | 1st   | 15 min   |
//! | 2nd   | 30 min   |
//! | 3rd+  | 60 min   |
//!
//! Localhost (`127.0.0.1`, `::1`) is always whitelisted.

use std::net::IpAddr;
use std::time::{Duration, Instant};

use dashmap::DashMap;

/// Maximum consecutive auth failures before auto-ban kicks in.
const MAX_FAILURES: u32 = 10;

/// Ban durations indexed by ban count (0-based after first ban).
const BAN_DURATIONS: [Duration; 3] = [
    Duration::from_secs(15 * 60), // 1st ban: 15 min
    Duration::from_secs(30 * 60), // 2nd ban: 30 min
    Duration::from_secs(60 * 60), // 3rd+ ban: 60 min
];

fn ban_duration(ban_count: u32) -> Duration {
    let idx = (ban_count.saturating_sub(1) as usize).min(BAN_DURATIONS.len() - 1);
    BAN_DURATIONS[idx]
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

struct BanEntry {
    /// When the current ban expires (if active).
    banned_until: Option<Instant>,
    /// Consecutive auth failure count.
    failure_count: u32,
    /// How many times this IP has been banned.
    ban_count: u32,
}

impl BanEntry {
    fn new() -> Self {
        Self {
            banned_until: None,
            failure_count: 0,
            ban_count: 0,
        }
    }
}

/// Thread-safe IP blocklist.
pub struct IpBlocklist {
    entries: DashMap<IpAddr, BanEntry>,
    whitelist: Vec<IpAddr>,
}

impl IpBlocklist {
    /// Create a new blocklist.
    ///
    /// `whitelist` should contain IP address strings (e.g. `"192.168.1.0"`).
    /// `127.0.0.1` and `::1` are always implicitly whitelisted.
    pub fn new(whitelist: Vec<String>) -> Self {
        let mut wl: Vec<IpAddr> = vec![
            IpAddr::from([127, 0, 0, 1]),
            IpAddr::from([0, 0, 0, 0, 0, 0, 0, 1]),
        ];
        for s in &whitelist {
            if let Ok(ip) = s.parse::<IpAddr>() {
                wl.push(ip);
            } else {
                tracing::warn!(entry = %s, "ignoring invalid whitelist entry");
            }
        }
        Self {
            entries: DashMap::new(),
            whitelist: wl,
        }
    }

    /// Returns `true` if the IP is currently banned (and not whitelisted).
    pub fn is_banned(&self, ip: &IpAddr) -> bool {
        if self.whitelist.contains(ip) {
            return false;
        }
        if let Some(entry) = self.entries.get(ip) {
            if let Some(until) = entry.banned_until {
                return Instant::now() < until;
            }
        }
        false
    }

    /// Record an authentication failure for `ip`.
    ///
    /// After [`MAX_FAILURES`] consecutive failures the IP is automatically
    /// banned with an escalating duration.
    pub fn record_failure(&self, ip: &IpAddr) {
        if self.whitelist.contains(ip) {
            return;
        }

        let mut entry = self.entries.entry(*ip).or_insert_with(BanEntry::new);
        entry.failure_count += 1;

        if entry.failure_count >= MAX_FAILURES {
            entry.ban_count += 1;
            let duration = ban_duration(entry.ban_count);
            entry.banned_until = Some(Instant::now() + duration);
            entry.failure_count = 0; // reset for next cycle

            tracing::warn!(
                ip = %ip,
                ban_count = entry.ban_count,
                duration_secs = duration.as_secs(),
                "IP auto-banned after {} consecutive auth failures",
                MAX_FAILURES,
            );
        }
    }

    /// Record a successful authentication for `ip`, resetting its failure count.
    pub fn record_success(&self, ip: &IpAddr) {
        if let Some(mut entry) = self.entries.get_mut(ip) {
            entry.failure_count = 0;
        }
    }

    /// Remove expired ban entries to reclaim memory.
    ///
    /// Should be called periodically from a background task.
    pub fn cleanup_expired(&self) {
        let now = Instant::now();
        self.entries.retain(|_ip, entry| {
            // Keep if there is no ban, or if the ban hasn't expired yet,
            // or if there is a non-zero failure count worth tracking.
            match entry.banned_until {
                Some(until) if now >= until => {
                    // Ban expired — drop if no recent failures
                    entry.banned_until = None;
                    entry.failure_count > 0 || entry.ban_count > 0
                }
                _ => true,
            }
        });
    }
}

impl Default for IpBlocklist {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ip() -> IpAddr {
        "10.0.0.1".parse().unwrap()
    }

    #[test]
    fn not_banned_initially() {
        let bl = IpBlocklist::default();
        assert!(!bl.is_banned(&test_ip()));
    }

    #[test]
    fn ban_after_max_failures() {
        let bl = IpBlocklist::default();
        let ip = test_ip();
        for _ in 0..MAX_FAILURES {
            bl.record_failure(&ip);
        }
        assert!(bl.is_banned(&ip));
    }

    #[test]
    fn success_resets_failure_count() {
        let bl = IpBlocklist::default();
        let ip = test_ip();
        for _ in 0..(MAX_FAILURES - 1) {
            bl.record_failure(&ip);
        }
        bl.record_success(&ip);
        // One more failure should not trigger ban (counter was reset)
        bl.record_failure(&ip);
        assert!(!bl.is_banned(&ip));
    }

    #[test]
    fn localhost_whitelisted() {
        let bl = IpBlocklist::default();
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        for _ in 0..MAX_FAILURES {
            bl.record_failure(&ip);
        }
        assert!(!bl.is_banned(&ip));
    }

    #[test]
    fn escalating_ban_durations() {
        assert_eq!(ban_duration(1), Duration::from_secs(15 * 60));
        assert_eq!(ban_duration(2), Duration::from_secs(30 * 60));
        assert_eq!(ban_duration(3), Duration::from_secs(60 * 60));
        assert_eq!(ban_duration(10), Duration::from_secs(60 * 60)); // capped
    }
}
