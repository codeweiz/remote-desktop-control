use anyhow::{Context, Result};

use crate::daemon;
use crate::PluginAction;

/// Dispatch plugin sub-commands to the running daemon via its REST API.
pub fn handle(action: &PluginAction) -> Result<()> {
    let (base_url, token) = daemon::api_base()?;

    match action {
        PluginAction::List => list_plugins(&base_url, &token),
        PluginAction::Enable { name } => enable_plugin(&base_url, &token, name),
        PluginAction::Disable { name } => disable_plugin(&base_url, &token, name),
    }
}

fn list_plugins(base_url: &str, token: &str) -> Result<()> {
    let url = format!("{}/api/v1/plugins", base_url);
    let resp = reqwest::blocking::Client::new()
        .get(&url)
        .bearer_auth(token)
        .send()
        .context("failed to connect to RTB daemon – is it running?")?;

    if !resp.status().is_success() {
        anyhow::bail!("daemon returned HTTP {}", resp.status());
    }

    let body: serde_json::Value = resp.json().context("invalid JSON from daemon")?;

    if let Some(plugins) = body.as_array() {
        if plugins.is_empty() {
            println!("No plugins installed.");
        } else {
            println!("{:<20} {:<10} {:<15}", "NAME", "TYPE", "STATUS");
            println!("{}", "-".repeat(45));
            for p in plugins {
                let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("-");
                let ptype = p.get("type").and_then(|v| v.as_str()).unwrap_or("-");
                let status = p.get("status").and_then(|v| v.as_str()).unwrap_or("-");
                println!("{:<20} {:<10} {:<15}", name, ptype, status);
            }
        }
    } else {
        println!("{}", serde_json::to_string_pretty(&body)?);
    }

    Ok(())
}

fn enable_plugin(base_url: &str, token: &str, name: &str) -> Result<()> {
    let url = format!("{}/api/v1/plugins/{}/enable", base_url, name);

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

    let body: serde_json::Value = resp.json().context("invalid JSON from daemon")?;
    let msg = body
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("Plugin enabled");
    println!("{}", msg);

    Ok(())
}

fn disable_plugin(base_url: &str, token: &str, name: &str) -> Result<()> {
    let url = format!("{}/api/v1/plugins/{}/disable", base_url, name);

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

    let body: serde_json::Value = resp.json().context("invalid JSON from daemon")?;
    let msg = body
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("Plugin disabled");
    println!("{}", msg);

    Ok(())
}
