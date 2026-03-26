use anyhow::{Context, Result};

use crate::daemon;
use crate::TunnelAction;

/// Dispatch tunnel sub-commands to the running daemon via its REST API.
pub fn handle(action: &TunnelAction) -> Result<()> {
    let (base_url, token) = daemon::api_base()?;

    match action {
        TunnelAction::Start { provider, domain } => {
            start_tunnel(&base_url, &token, provider, domain)
        }
        TunnelAction::Stop => stop_tunnel(&base_url, &token),
        TunnelAction::Status => tunnel_status(&base_url, &token),
    }
}

fn start_tunnel(
    base_url: &str,
    token: &str,
    provider: &Option<String>,
    domain: &Option<String>,
) -> Result<()> {
    let url = format!("{}/api/v1/tunnel/start", base_url);

    let mut body = serde_json::Map::new();
    if let Some(p) = provider {
        body.insert("provider".into(), serde_json::Value::String(p.clone()));
    }
    if let Some(d) = domain {
        body.insert("domain".into(), serde_json::Value::String(d.clone()));
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
    if let Some(url) = body.get("url").and_then(|v| v.as_str()) {
        println!("Tunnel started: {}", url);
    } else {
        println!("{}", serde_json::to_string_pretty(&body)?);
    }

    Ok(())
}

fn stop_tunnel(base_url: &str, token: &str) -> Result<()> {
    let url = format!("{}/api/v1/tunnel/stop", base_url);

    let resp = reqwest::blocking::Client::new()
        .post(&url)
        .bearer_auth(token)
        .send()
        .context("failed to connect to RTB daemon – is it running?")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        anyhow::bail!("daemon returned HTTP {}: {}", status, text);
    }

    println!("Tunnel stopped.");
    Ok(())
}

fn tunnel_status(base_url: &str, token: &str) -> Result<()> {
    let url = format!("{}/api/v1/tunnel/status", base_url);

    let resp = reqwest::blocking::Client::new()
        .get(&url)
        .bearer_auth(token)
        .send()
        .context("failed to connect to RTB daemon – is it running?")?;

    if !resp.status().is_success() {
        anyhow::bail!("daemon returned HTTP {}", resp.status());
    }

    let body: serde_json::Value = resp.json().context("invalid JSON from daemon")?;
    let active = body
        .get("active")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let message = body
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let tunnel_url = body.get("url").and_then(|v| v.as_str());
    let provider = body.get("provider").and_then(|v| v.as_str());

    if active {
        println!("Tunnel: ACTIVE");
        if let Some(p) = provider {
            println!("Provider: {}", p);
        }
        if let Some(u) = tunnel_url {
            println!("URL: {}", u);
        }
    } else {
        println!("Tunnel: INACTIVE");
    }
    if !message.is_empty() {
        println!("{}", message);
    }

    Ok(())
}
