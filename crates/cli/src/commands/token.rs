use anyhow::{Context, Result};

use crate::daemon;
use crate::TokenAction;

/// Dispatch token sub-commands.
pub fn handle(action: &TokenAction) -> Result<()> {
    match action {
        TokenAction::Rotate => rotate_token(),
        TokenAction::Show => show_token(),
    }
}

/// Rotate the authentication token.
///
/// 1. Read the current (old) token from disk.
/// 2. Generate a new token and write it to ~/.rtb/session.token.
/// 3. If the daemon is running, call POST /api/v1/token/rotate with
///    the old token to inform the server of the change.
fn rotate_token() -> Result<()> {
    let config = rtb_core::config::Config::load().unwrap_or_default();

    // Read the old token (needed to authenticate the rotation request)
    let old_token = daemon::read_token(&config).ok();

    // Generate a new token and write it to disk
    let new_token = daemon::generate_token();
    let token_path = daemon::expand_token_path(&config);

    // Ensure parent dir exists
    if let Some(parent) = std::path::Path::new(&token_path).parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    std::fs::write(&token_path, &new_token)
        .with_context(|| format!("failed to write new token to {}", token_path))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&token_path, std::fs::Permissions::from_mode(0o600))?;
    }

    println!("New token: {}", new_token);

    // If the daemon is running, notify it of the new token
    if daemon::is_running() {
        if let Some(old) = old_token {
            let (base_url, _) = daemon::api_base()?;
            let url = format!("{}/api/v1/token/rotate", base_url);

            let body = serde_json::json!({ "new_token": new_token });

            match reqwest::blocking::Client::new()
                .post(&url)
                .bearer_auth(&old)
                .json(&body)
                .send()
            {
                Ok(resp) if resp.status().is_success() => {
                    println!("Daemon notified of token rotation.");
                }
                Ok(resp) => {
                    let status = resp.status();
                    let text = resp.text().unwrap_or_default();
                    println!(
                        "Warning: daemon returned HTTP {} when rotating token: {}",
                        status, text
                    );
                    println!("The token file has been updated. Restart the daemon to apply.");
                }
                Err(e) => {
                    println!(
                        "Warning: could not reach daemon to rotate token: {}",
                        e
                    );
                    println!("The token file has been updated. Restart the daemon to apply.");
                }
            }
        } else {
            println!("Warning: could not read old token. Restart the daemon to apply the new token.");
        }
    } else {
        println!("Daemon is not running. New token will take effect on next start.");
    }

    Ok(())
}

/// Show the current token.
fn show_token() -> Result<()> {
    let config = rtb_core::config::Config::load().unwrap_or_default();
    let token = daemon::read_token(&config)?;
    println!("{}", token);
    Ok(())
}
