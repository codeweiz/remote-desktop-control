use anyhow::{Context, Result};

use crate::daemon;
use crate::AgentAction;

/// Dispatch agent sub-commands to the running daemon via its REST API.
pub fn handle(action: &AgentAction) -> Result<()> {
    let (base_url, token) = daemon::api_base()?;

    match action {
        AgentAction::New {
            name,
            provider,
            model,
            cwd,
        } => new_agent(&base_url, &token, name, provider, model, cwd),
    }
}

fn new_agent(
    base_url: &str,
    token: &str,
    name: &Option<String>,
    provider: &Option<String>,
    model: &Option<String>,
    cwd: &Option<String>,
) -> Result<()> {
    let url = format!("{}/api/v1/sessions", base_url);

    let session_name = name.clone().unwrap_or_else(|| "agent".to_string());

    let mut body = serde_json::Map::new();
    body.insert("name".into(), serde_json::Value::String(session_name));
    body.insert(
        "type".into(),
        serde_json::Value::String("agent".to_string()),
    );
    if let Some(p) = provider {
        body.insert("provider".into(), serde_json::Value::String(p.clone()));
    }
    if let Some(m) = model {
        body.insert("model".into(), serde_json::Value::String(m.clone()));
    }
    if let Some(c) = cwd {
        body.insert("cwd".into(), serde_json::Value::String(c.clone()));
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
    println!("Agent session created: {}", id);

    Ok(())
}
