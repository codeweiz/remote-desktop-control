use anyhow::{Context, Result};

use crate::ConfigAction;

/// Dispatch config sub-commands.
pub fn handle(action: &ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Show => show_config(),
        ConfigAction::Edit => edit_config(),
        ConfigAction::Init => init_config(),
    }
}

/// Print the current configuration.
fn show_config() -> Result<()> {
    let config_path =
        rtb_core::config::Config::default_path().context("could not determine config path")?;

    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read {}", config_path.display()))?;
        println!("# {}", config_path.display());
        println!("{}", content);
    } else {
        println!("No config file found at {}", config_path.display());
        println!();
        println!("Using defaults. Run `rtb config init` to create a config file.");
    }

    Ok(())
}

/// Open the config file in the user's $EDITOR.
fn edit_config() -> Result<()> {
    let config_path =
        rtb_core::config::Config::default_path().context("could not determine config path")?;

    // Create a default config if none exists
    if !config_path.exists() {
        println!("No config file found. Creating default config...");
        let config = rtb_core::config::Config::default();
        config.save().context("failed to save default config")?;
        println!("Created {}", config_path.display());
    }

    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| {
            // Try common editors
            if which_exists("code") {
                "code --wait".to_string()
            } else if which_exists("vim") {
                "vim".to_string()
            } else if which_exists("nano") {
                "nano".to_string()
            } else {
                "vi".to_string()
            }
        });

    let path_str = config_path
        .to_str()
        .context("config path is not valid UTF-8")?;

    // Split editor command (e.g., "code --wait" -> ["code", "--wait"])
    let parts: Vec<&str> = editor.split_whitespace().collect();
    let (cmd, args) = parts.split_first().context("empty editor command")?;

    let status = std::process::Command::new(cmd)
        .args(args.iter())
        .arg(path_str)
        .status()
        .with_context(|| format!("failed to launch editor: {}", editor))?;

    if !status.success() {
        anyhow::bail!("editor exited with status: {}", status);
    }

    println!("Config saved. Restart the daemon for changes to take effect.");
    Ok(())
}

/// Interactive config initialization: create a default config file.
fn init_config() -> Result<()> {
    let config_path =
        rtb_core::config::Config::default_path().context("could not determine config path")?;

    if config_path.exists() {
        println!("Config file already exists at {}", config_path.display());
        println!("Use `rtb config edit` to modify it.");
        return Ok(());
    }

    // Create parent directory
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o700);
            std::fs::set_permissions(parent, perms).ok();
        }
    }

    let config = rtb_core::config::Config::default();
    config.save().context("failed to save config")?;

    println!("Config initialized at {}", config_path.display());
    println!();

    // Print the generated config
    let content =
        std::fs::read_to_string(&config_path).context("failed to read generated config")?;
    println!("{}", content);

    println!();
    println!("Edit with: rtb config edit");
    println!("Or directly: $EDITOR {}", config_path.display());

    Ok(())
}

/// Check if a command exists on PATH.
fn which_exists(name: &str) -> bool {
    std::process::Command::new("which")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
