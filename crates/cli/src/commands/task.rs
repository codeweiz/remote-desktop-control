use anyhow::{Context, Result};

use crate::daemon;
use crate::TaskAction;

/// Dispatch task sub-commands to the running daemon via its REST API.
pub fn handle(action: &TaskAction) -> Result<()> {
    let (base_url, token) = daemon::api_base()?;

    match action {
        TaskAction::Add {
            title,
            priority,
            cwd,
            depends_on,
        } => add_task(&base_url, &token, title, priority, cwd, depends_on),
        TaskAction::List => list_tasks(&base_url, &token),
        TaskAction::Cancel { id } => cancel_task(&base_url, &token, id),
        TaskAction::Pause => pause_scheduler(&base_url, &token),
        TaskAction::Resume => resume_scheduler(&base_url, &token),
    }
}

fn add_task(
    base_url: &str,
    token: &str,
    title: &str,
    priority: &Option<String>,
    cwd: &Option<String>,
    depends_on: &Option<String>,
) -> Result<()> {
    let url = format!("{}/api/v1/tasks", base_url);

    let mut body = serde_json::Map::new();
    body.insert("title".into(), serde_json::Value::String(title.to_string()));
    if let Some(p) = priority {
        body.insert("priority".into(), serde_json::Value::String(p.clone()));
    }
    if let Some(c) = cwd {
        body.insert("cwd".into(), serde_json::Value::String(c.clone()));
    }
    if let Some(dep) = depends_on {
        let deps: Vec<serde_json::Value> = dep
            .split(',')
            .map(|s| serde_json::Value::String(s.trim().to_string()))
            .collect();
        body.insert("depends_on".into(), serde_json::Value::Array(deps));
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
    let status = body
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("queued");
    println!("Task added: {} (status: {})", id, status);

    Ok(())
}

fn list_tasks(base_url: &str, token: &str) -> Result<()> {
    let url = format!("{}/api/v1/tasks", base_url);
    let resp = reqwest::blocking::Client::new()
        .get(&url)
        .bearer_auth(token)
        .send()
        .context("failed to connect to RTB daemon – is it running?")?;

    if !resp.status().is_success() {
        anyhow::bail!("daemon returned HTTP {}", resp.status());
    }

    let body: serde_json::Value = resp.json().context("invalid JSON from daemon")?;

    if let Some(tasks) = body.as_array() {
        if tasks.is_empty() {
            println!("No tasks.");
        } else {
            println!("{:<14} {:<30} {:<4} {:<12}", "ID", "NAME", "PRI", "STATUS");
            println!("{}", "-".repeat(64));
            for t in tasks {
                let id = t.get("id").and_then(|v| v.as_str()).unwrap_or("-");
                let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("-");
                let priority = t.get("priority").and_then(|v| v.as_str()).unwrap_or("-");
                let status = t.get("status").and_then(|v| v.as_str()).unwrap_or("-");
                println!("{:<14} {:<30} {:<4} {:<12}", id, name, priority, status);
            }
        }
    } else {
        println!("{}", serde_json::to_string_pretty(&body)?);
    }

    Ok(())
}

fn cancel_task(base_url: &str, token: &str, id: &str) -> Result<()> {
    let url = format!("{}/api/v1/tasks/{}", base_url, id);

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

    let body: serde_json::Value = resp.json().context("invalid JSON from daemon")?;
    let msg = body
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("Task cancelled");
    println!("{}", msg);

    Ok(())
}

fn pause_scheduler(base_url: &str, token: &str) -> Result<()> {
    let url = format!("{}/api/v1/tasks/scheduler/pause", base_url);

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

    println!("Task scheduler paused.");
    Ok(())
}

fn resume_scheduler(base_url: &str, token: &str) -> Result<()> {
    let url = format!("{}/api/v1/tasks/scheduler/resume", base_url);

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

    println!("Task scheduler resumed.");
    Ok(())
}
