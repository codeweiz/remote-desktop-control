use anyhow::{Context, Result};

use crate::daemon;
use crate::SessionAction;

/// Dispatch session sub-commands to the running daemon via its REST API.
pub fn handle(action: &SessionAction) -> Result<()> {
    let (base_url, token) = daemon::api_base()?;

    match action {
        SessionAction::List => list_sessions(&base_url, &token),
        SessionAction::New { name, cmd } => new_session(&base_url, &token, name, cmd),
        SessionAction::Kill { id } => kill_session(&base_url, &token, id),
    }
}

fn list_sessions(base_url: &str, token: &str) -> Result<()> {
    let url = format!("{}/api/v1/sessions", base_url);
    let resp = reqwest::blocking::Client::new()
        .get(&url)
        .bearer_auth(token)
        .send()
        .context("failed to connect to RTB daemon – is it running?")?;

    if !resp.status().is_success() {
        anyhow::bail!("daemon returned HTTP {}", resp.status());
    }

    let body: serde_json::Value = resp.json().context("invalid JSON from daemon")?;

    // Pretty-print sessions
    if let Some(sessions) = body.as_array() {
        if sessions.is_empty() {
            println!("No active sessions.");
        } else {
            println!("{:<14} {:<20} {:<10}", "ID", "NAME", "STATUS");
            println!("{}", "-".repeat(44));
            for s in sessions {
                let id = s.get("id").and_then(|v| v.as_str()).unwrap_or("-");
                let name = s.get("name").and_then(|v| v.as_str()).unwrap_or("-");
                let status = s.get("status").and_then(|v| v.as_str()).unwrap_or("-");
                println!("{:<14} {:<20} {:<10}", id, name, status);
            }
        }
    } else {
        // Fallback – just dump the JSON
        println!("{}", serde_json::to_string_pretty(&body)?);
    }

    Ok(())
}

fn new_session(
    base_url: &str,
    token: &str,
    name: &Option<String>,
    cmd: &Option<String>,
) -> Result<()> {
    let url = format!("{}/api/v1/sessions", base_url);

    let mut body = serde_json::Map::new();
    if let Some(n) = name {
        body.insert("name".into(), serde_json::Value::String(n.clone()));
    }
    if let Some(c) = cmd {
        body.insert("cmd".into(), serde_json::Value::String(c.clone()));
    }

    let resp = reqwest::blocking::Client::new()
        .post(&url)
        .bearer_auth(token)
        .json(&body)
        .send()
        .context("failed to connect to RTB daemon – is it running?")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        anyhow::bail!("daemon returned HTTP {}: {}", status, text);
    }

    let body: serde_json::Value = resp.json().context("invalid JSON from daemon")?;
    let id = body.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
    println!("Session created: {}", id);

    Ok(())
}

fn kill_session(base_url: &str, token: &str, id: &str) -> Result<()> {
    let url = format!("{}/api/v1/sessions/{}", base_url, id);

    let resp = reqwest::blocking::Client::new()
        .delete(&url)
        .bearer_auth(token)
        .send()
        .context("failed to connect to RTB daemon – is it running?")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        anyhow::bail!("daemon returned HTTP {}: {}", status, text);
    }

    println!("Session {} killed.", id);
    Ok(())
}
